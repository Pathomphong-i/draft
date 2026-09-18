use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tempfile::NamedTempFile;

use super::error::CasError;
use super::id::ObjectId;
use super::object::{ObjectType, RawObject};

/// The Content-Addressable Storage (CAS) engine.
/// Thread-safe, multi-process safe, with atomic writes and lock-free reads.
#[derive(Debug, Clone)]
pub struct ObjectStore {
    root: PathBuf,
    tmp_dir: PathBuf,
}

impl ObjectStore {
    /// Initialize or open an object store at `.dft/objects`.
    /// Creates root and tmp directories if they do not already exist.
    pub fn init(objects_dir: impl AsRef<Path>) -> Result<Self, CasError> {
        let root = objects_dir.as_ref().to_path_buf();
        let tmp_dir = root.join("tmp");

        fs::create_dir_all(&tmp_dir).map_err(|e| CasError::Io {
            path: tmp_dir.clone(),
            source: e,
        })?;

        Ok(Self { root, tmp_dir })
    }

    /// Access the root objects path.
    pub fn root_path(&self) -> &Path {
        &self.root
    }

    /// Access the staging temporary directory path.
    pub fn tmp_path(&self) -> &Path {
        &self.tmp_dir
    }

    /// Calculate the canonical on-disk path for an ObjectId: `.dft/objects/xx/yyyy...`
    pub fn object_path(&self, id: &ObjectId) -> PathBuf {
        let (prefix, remainder) = id.fanout_parts();
        self.root.join(prefix).join(remainder)
    }

    /// Check if an object exists in the store without decompressing it.
    /// Fast lock-free existence check.
    pub fn has_object(&self, id: &ObjectId) -> bool {
        self.object_path(id).exists()
    }

    /// Write an uncompressed RawObject into the CAS store.
    ///
    /// Protocol:
    /// 1. Calculate uncompressed framed envelope and SHA-256 ObjectId.
    /// 2. If object file already exists, return Ok(id) immediately (idempotent write).
    /// 3. Create a unique temporary file inside `.dft/objects/tmp/` (ensures same filesystem).
    /// 4. Compress framed envelope via zlib deflate (level 6) into temporary file.
    /// 5. Call fsync (sync_all) to guarantee persistence.
    /// 6. Ensure target directory `.dft/objects/xx/` exists.
    /// 7. Atomically rename the temporary file to target path using `persist`.
    pub fn write_raw(&self, obj: &RawObject) -> Result<ObjectId, CasError> {
        let framed = obj.framed_bytes();
        let id = obj.compute_id();
        let target_path = self.object_path(&id);

        // Idempotency: if object already exists, avoid duplicate compression and I/O.
        if target_path.exists() {
            return Ok(id);
        }

        // Step 1: Create unique temporary file on the same filesystem
        let mut temp_file = NamedTempFile::new_in(&self.tmp_dir).map_err(|e| CasError::Io {
            path: self.tmp_dir.clone(),
            source: e,
        })?;

        // Step 2: Compress framed envelope into temp file
        {
            let mut encoder = ZlibEncoder::new(&mut temp_file, Compression::default());
            encoder.write_all(&framed).map_err(CasError::Compression)?;
            encoder.finish().map_err(CasError::Compression)?;
        }

        // Step 3: Flush OS buffer to disk
        temp_file.as_file().sync_all().map_err(|e| CasError::Io {
            path: temp_file.path().to_path_buf(),
            source: e,
        })?;

        // Step 4: Ensure target fanout directory exists
        if let Some(parent) = target_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| CasError::Io {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
        }

        // Step 5: Atomically rename into final location
        match temp_file.persist(&target_path) {
            Ok(_) => {}
            Err(e) => {
                // If another thread or process raced and created it identically, it's valid
                if !target_path.exists() {
                    return Err(CasError::PersistFailed {
                        target: target_path,
                        error: e.to_string(),
                    });
                }
            }
        }

        // Step 6: Hardening: mark object as read-only (0o444 on Unix)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&target_path, fs::Permissions::from_mode(0o444));
        }

        Ok(id)
    }

    /// Read and decompress an object by its ObjectId without explicit re-hashing.
    /// Reads are completely lock-free.
    pub fn read_raw(&self, id: &ObjectId) -> Result<RawObject, CasError> {
        let path = self.object_path(id);
        if !path.exists() {
            return Err(CasError::ObjectNotFound(*id));
        }

        let file = File::open(&path).map_err(|e| CasError::Io {
            path: path.clone(),
            source: e,
        })?;

        let mut decoder = ZlibDecoder::new(file);
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .map_err(CasError::Decompression)?;

        RawObject::parse_framed(&decompressed)
    }

    /// Read an object and verify its SHA-256 hash against the requested ObjectId.
    /// Guarantees data integrity against bit rot or disk tampering.
    pub fn read_raw_verified(&self, id: &ObjectId) -> Result<RawObject, CasError> {
        let obj = self.read_raw(id)?;
        let computed_id = obj.compute_id();
        if computed_id != *id {
            return Err(CasError::HashMismatch {
                id: *id,
                computed: computed_id,
            });
        }
        Ok(obj)
    }

    /// High-performance zero-copy memory-mapped read for large objects.
    pub fn read_raw_mmap(&self, id: &ObjectId) -> Result<RawObject, CasError> {
        let path = self.object_path(id);
        if !path.exists() {
            return Err(CasError::ObjectNotFound(*id));
        }

        let file = File::open(&path).map_err(|e| CasError::Io {
            path: path.clone(),
            source: e,
        })?;

        let mmap = unsafe {
            memmap2::Mmap::map(&file).map_err(|e| CasError::Io {
                path: path.clone(),
                source: e,
            })?
        };

        let mut decoder = ZlibDecoder::new(&mmap[..]);
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .map_err(CasError::Decompression)?;

        RawObject::parse_framed(&decompressed)
    }

    /// Convenience helper: Write a byte slice as a blob object.
    pub fn write_blob(&self, data: &[u8]) -> Result<ObjectId, CasError> {
        let raw = RawObject::blob(data.to_vec());
        self.write_raw(&raw)
    }

    /// Convenience helper: Read raw payload bytes of a blob object.
    pub fn read_blob(&self, id: &ObjectId) -> Result<Vec<u8>, CasError> {
        let raw = self.read_raw(id)?;
        if raw.object_type != ObjectType::Blob {
            return Err(CasError::CorruptObject(
                *id,
                format!("expected blob object, found {}", raw.object_type),
            ));
        }
        Ok(raw.data)
    }

    /// Iterate over all loose ObjectIds currently in the CAS store.
    pub fn list_objects(&self) -> Result<Vec<ObjectId>, CasError> {
        let mut objects = Vec::new();
        if !self.root.exists() {
            return Ok(objects);
        }

        for entry in fs::read_dir(&self.root).map_err(|e| CasError::Io {
            path: self.root.clone(),
            source: e,
        })? {
            let entry = entry.map_err(|e| CasError::Io {
                path: self.root.clone(),
                source: e,
            })?;
            let file_name = entry.file_name();
            let prefix = file_name.to_string_lossy();

            // Only inspect 2-character hex subdirectories (skip 'tmp', 'pack', 'info')
            if prefix.len() == 2 && prefix.chars().all(|c| c.is_ascii_hexdigit()) {
                let sub_dir = entry.path();
                for sub_entry in fs::read_dir(&sub_dir).map_err(|e| CasError::Io {
                    path: sub_dir.clone(),
                    source: e,
                })? {
                    let sub_entry = sub_entry.map_err(|e| CasError::Io {
                        path: sub_dir.clone(),
                        source: e,
                    })?;
                    let remainder = sub_entry.file_name().to_string_lossy().to_string();
                    if remainder.len() == 62 && remainder.chars().all(|c| c.is_ascii_hexdigit()) {
                        let full_hex = format!("{}{}", prefix, remainder);
                        if let Ok(oid) = ObjectId::from_hex(&full_hex) {
                            objects.push(oid);
                        }
                    }
                }
            }
        }
        Ok(objects)
    }

    /// Clean up any orphaned temporary files in `.dft/objects/tmp/` older than `max_age`.
    pub fn clean_stale_tmp(&self, max_age: std::time::Duration) -> Result<usize, CasError> {
        let mut cleaned = 0;
        if !self.tmp_dir.exists() {
            return Ok(0);
        }

        let now = SystemTime::now();
        for entry in fs::read_dir(&self.tmp_dir).map_err(|e| CasError::Io {
            path: self.tmp_dir.clone(),
            source: e,
        })? {
            let entry = entry.map_err(|e| CasError::Io {
                path: self.tmp_dir.clone(),
                source: e,
            })?;
            if let Ok(meta) = entry.metadata() {
                if let Ok(modified) = meta.modified() {
                    if let Ok(age) = now.duration_since(modified) {
                        if age > max_age {
                            let _ = fs::remove_file(entry.path());
                            cleaned += 1;
                        }
                    }
                }
            }
        }
        Ok(cleaned)
    }
}
