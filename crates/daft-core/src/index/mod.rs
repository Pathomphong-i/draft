pub mod entry;
pub mod lock;

pub use entry::{IndexEntry, IndexTime, Stage, DIRC_MAGIC, DIRC_VERSION, FIXED_ENTRY_SIZE};
pub use lock::IndexLock;

use crate::cas::ObjectId;
use crate::error::IndexError;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct Index {
    entries: Vec<IndexEntry>,
}

impl Index {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[IndexEntry] {
        &self.entries
    }

    pub fn entries_mut(&mut self) -> &mut [IndexEntry] {
        &mut self.entries
    }

    /// Locate entry by (path, stage) using binary search.
    pub fn find_entry(&self, path: &str, stage: Stage) -> Option<&IndexEntry> {
        self.entries
            .binary_search_by(|e| {
                e.path
                    .as_str()
                    .cmp(path)
                    .then_with(|| e.stage().cmp(&stage))
            })
            .ok()
            .map(|idx| &self.entries[idx])
    }

    /// Add or update an entry. If adding Stage::Normal, removes any conflicting
    /// Stage 1, 2, 3 entries for this path.
    pub fn add_entry(&mut self, entry: IndexEntry) {
        if entry.stage() == Stage::Normal {
            self.remove_path(&entry.path);
        }

        match self.entries.binary_search_by(|e| {
            e.path
                .cmp(&entry.path)
                .then_with(|| e.stage().cmp(&entry.stage()))
        }) {
            Ok(idx) => {
                self.entries[idx] = entry;
            }
            Err(idx) => {
                self.entries.insert(idx, entry);
            }
        }
    }

    /// Remove a specific (path, stage) entry.
    pub fn remove_entry(&mut self, path: &str, stage: Stage) -> bool {
        if let Ok(idx) = self.entries.binary_search_by(|e| {
            e.path
                .as_str()
                .cmp(path)
                .then_with(|| e.stage().cmp(&stage))
        }) {
            self.entries.remove(idx);
            true
        } else {
            false
        }
    }

    /// Remove all entries matching path across all stages.
    pub fn remove_path(&mut self, path: &str) -> bool {
        let initial_len = self.entries.len();
        self.entries.retain(|e| e.path != path);
        self.entries.len() < initial_len
    }

    /// Check if index has any unmerged conflict stages (1, 2, or 3).
    pub fn has_conflicts(&self) -> bool {
        self.entries.iter().any(|e| e.stage() != Stage::Normal)
    }

    /// Collect all conflicting paths with their associated entries.
    pub fn conflicts(&self) -> BTreeMap<String, Vec<&IndexEntry>> {
        let mut map = BTreeMap::new();
        for entry in &self.entries {
            if entry.stage() != Stage::Normal {
                map.entry(entry.path.clone())
                    .or_insert_with(Vec::new)
                    .push(entry);
            }
        }
        map
    }

    /// Read an index from a file on disk, validating its DIRC header and SHA-256 checksum.
    pub fn read_from(path: &Path) -> Result<Self, IndexError> {
        let mut file = File::open(path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;

        if data.len() < 44 {
            return Err(IndexError::TruncatedEntry(data.len()));
        }

        // 1. Verify Checksum
        let content_len = data.len() - 32;
        let expected_checksum = &data[content_len..];
        let mut hasher = Sha256::new();
        hasher.update(&data[..content_len]);
        let computed_checksum: [u8; 32] = hasher.finalize().into();

        if computed_checksum != expected_checksum {
            return Err(IndexError::CorruptChecksum {
                expected: hex::encode(expected_checksum),
                actual: hex::encode(computed_checksum),
            });
        }

        // 2. Parse Header
        let magic = &data[0..4];
        if magic != DIRC_MAGIC {
            let mut arr = [0u8; 4];
            arr.copy_from_slice(magic);
            return Err(IndexError::BadMagic(arr));
        }

        let version = u32::from_be_bytes(data[4..8].try_into().unwrap());
        if version != DIRC_VERSION {
            return Err(IndexError::UnsupportedVersion(version));
        }

        let entry_count = u32::from_be_bytes(data[8..12].try_into().unwrap()) as usize;

        // 3. Parse Entries
        let mut entries = Vec::with_capacity(entry_count);
        let mut offset = 12;

        for _ in 0..entry_count {
            if offset + FIXED_ENTRY_SIZE > content_len {
                return Err(IndexError::TruncatedEntry(offset));
            }

            let ctime_sec = u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap());
            let ctime_nsec = u32::from_be_bytes(data[offset + 4..offset + 8].try_into().unwrap());
            let mtime_sec = u32::from_be_bytes(data[offset + 8..offset + 12].try_into().unwrap());
            let mtime_nsec = u32::from_be_bytes(data[offset + 12..offset + 16].try_into().unwrap());
            let dev = u32::from_be_bytes(data[offset + 16..offset + 20].try_into().unwrap());
            let ino = u32::from_be_bytes(data[offset + 20..offset + 24].try_into().unwrap());
            let mode = u32::from_be_bytes(data[offset + 24..offset + 28].try_into().unwrap());
            let uid = u32::from_be_bytes(data[offset + 28..offset + 32].try_into().unwrap());
            let gid = u32::from_be_bytes(data[offset + 32..offset + 36].try_into().unwrap());
            let file_size = u32::from_be_bytes(data[offset + 36..offset + 40].try_into().unwrap());

            let mut oid_bytes = [0u8; 32];
            oid_bytes.copy_from_slice(&data[offset + 40..offset + 72]);
            let oid = ObjectId::from_bytes(oid_bytes);

            let flags = u16::from_be_bytes(data[offset + 72..offset + 74].try_into().unwrap());
            let path_start = offset + FIXED_ENTRY_SIZE;
            let declared_len = (flags & 0x0FFF) as usize;

            let (path_bytes, path_len) = if declared_len < 0x0FFF {
                if path_start + declared_len > content_len {
                    return Err(IndexError::TruncatedEntry(path_start));
                }
                let slice = &data[path_start..path_start + declared_len];
                if slice.contains(&0) {
                    return Err(IndexError::InvalidPath(
                        "path contains null byte".to_string(),
                    ));
                }
                (slice, declared_len)
            } else {
                if path_start + 4095 > content_len {
                    return Err(IndexError::TruncatedEntry(path_start));
                }
                if data[path_start..path_start + 4095].contains(&0) {
                    return Err(IndexError::InvalidPath(
                        "path contains null byte before clamped length".to_string(),
                    ));
                }
                let null_offset = data[path_start + 4095..content_len]
                    .iter()
                    .position(|&b| b == 0)
                    .ok_or(IndexError::TruncatedEntry(path_start + 4095))?;
                let len = 4095 + null_offset;
                (&data[path_start..path_start + len], len)
            };

            let path_str = std::str::from_utf8(path_bytes)
                .map_err(|e| IndexError::TruncatedEntry(path_start + e.valid_up_to()))?;
            IndexEntry::validate_path(path_str)?;

            let pad_len = 8 - ((FIXED_ENTRY_SIZE + path_len) % 8);
            let entry_len = FIXED_ENTRY_SIZE + path_len + pad_len;

            if offset + entry_len > content_len {
                return Err(IndexError::TruncatedEntry(offset));
            }

            let padding_slice = &data[path_start + path_len..offset + entry_len];
            if padding_slice.iter().any(|&b| b != 0) {
                return Err(IndexError::TruncatedEntry(path_start + path_len));
            }

            let path = path_str.to_string();

            entries.push(IndexEntry {
                ctime: IndexTime {
                    sec: ctime_sec,
                    nsec: ctime_nsec,
                },
                mtime: IndexTime {
                    sec: mtime_sec,
                    nsec: mtime_nsec,
                },
                dev,
                ino,
                mode,
                uid,
                gid,
                file_size,
                oid,
                flags,
                path,
            });

            offset += entry_len;
        }

        // Sort entries to maintain the invariant
        entries.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.stage().cmp(&b.stage())));

        Ok(Self { entries })
    }

    /// Write this index to a file using atomic locking.
    pub fn write_to(&self, path: &Path) -> Result<(), IndexError> {
        let mut lock = IndexLock::acquire(path)?;
        lock.write_index(self)?;
        lock.commit()?;
        Ok(())
    }
}
