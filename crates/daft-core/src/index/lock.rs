use super::entry::{DIRC_MAGIC, DIRC_VERSION, FIXED_ENTRY_SIZE};
use super::Index;
use crate::error::IndexError;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

pub struct IndexLock {
    target_path: PathBuf,
    lock_path: PathBuf,
    writer: Option<BufWriter<File>>,
    committed: bool,
}

impl IndexLock {
    pub fn acquire(index_path: &Path) -> Result<Self, IndexError> {
        let lock_path = PathBuf::from(format!("{}.lock", index_path.display()));

        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    IndexError::IndexLocked(lock_path.clone())
                } else {
                    IndexError::Io(e)
                }
            })?;

        Ok(Self {
            target_path: index_path.to_path_buf(),
            lock_path,
            writer: Some(BufWriter::new(file)),
            committed: false,
        })
    }

    pub fn write_index(&mut self, index: &Index) -> Result<(), IndexError> {
        let writer = self
            .writer
            .as_mut()
            .ok_or(IndexError::LockAlreadyCommitted)?;
        let mut hasher = Sha256::new();

        // 1. Write Header (12 bytes)
        writer.write_all(DIRC_MAGIC)?;
        hasher.update(DIRC_MAGIC);

        let version_bytes = DIRC_VERSION.to_be_bytes();
        writer.write_all(&version_bytes)?;
        hasher.update(version_bytes);

        let count_bytes = (index.len() as u32).to_be_bytes();
        writer.write_all(&count_bytes)?;
        hasher.update(count_bytes);

        // 2. Write Sorted Entries
        for entry in index.entries() {
            super::IndexEntry::validate_path(&entry.path)?;

            let mut buf = [0u8; FIXED_ENTRY_SIZE];
            buf[0..4].copy_from_slice(&entry.ctime.sec.to_be_bytes());
            buf[4..8].copy_from_slice(&entry.ctime.nsec.to_be_bytes());
            buf[8..12].copy_from_slice(&entry.mtime.sec.to_be_bytes());
            buf[12..16].copy_from_slice(&entry.mtime.nsec.to_be_bytes());
            buf[16..20].copy_from_slice(&entry.dev.to_be_bytes());
            buf[20..24].copy_from_slice(&entry.ino.to_be_bytes());
            buf[24..28].copy_from_slice(&entry.mode.to_be_bytes());
            buf[28..32].copy_from_slice(&entry.uid.to_be_bytes());
            buf[32..36].copy_from_slice(&entry.gid.to_be_bytes());
            buf[36..40].copy_from_slice(&entry.file_size.to_be_bytes());
            buf[40..72].copy_from_slice(entry.oid.as_bytes());

            // Flags: stage (bits 12..13), assume-valid (bit 15), clamped length (bits 0..11)
            let path_len_clamped = (entry.path.len().min(4095) as u16) & 0x0FFF;
            let flags = (entry.flags & !0x0FFF) | path_len_clamped;
            buf[72..74].copy_from_slice(&flags.to_be_bytes());

            writer.write_all(&buf)?;
            hasher.update(buf);

            // Write path bytes
            writer.write_all(entry.path.as_bytes())?;
            hasher.update(entry.path.as_bytes());

            // Write 1..8 null padding bytes
            let pad_len = entry.padding_len();
            let padding = [0u8; 8];
            writer.write_all(&padding[..pad_len])?;
            hasher.update(&padding[..pad_len]);
        }

        // 3. Write Checksum Footer (32 bytes)
        let checksum: [u8; 32] = hasher.finalize().into();
        writer.write_all(&checksum)?;
        writer.flush()?;

        // 4. fsync lockfile
        let file = writer.get_ref();
        file.sync_all()?;

        Ok(())
    }

    pub fn commit(mut self) -> Result<(), IndexError> {
        // Drop writer to flush & close file handle before renaming
        drop(self.writer.take());

        std::fs::rename(&self.lock_path, &self.target_path)?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for IndexLock {
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::fs::remove_file(&self.lock_path);
        }
    }
}
