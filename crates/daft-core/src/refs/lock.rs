use super::ReferenceTarget;
use crate::error::RefError;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct RefLock {
    ref_path: PathBuf,
    lock_path: PathBuf,
    file: Option<File>,
    committed: bool,
}

impl RefLock {
    pub fn acquire(
        ref_path: &Path,
        expected_old: Option<&ReferenceTarget>,
    ) -> Result<Self, RefError> {
        let lock_path = PathBuf::from(format!("{}.lock", ref_path.display()));

        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    RefError::RefLocked(ref_path.display().to_string())
                } else {
                    RefError::Io(e)
                }
            })?;

        let lock = Self {
            ref_path: ref_path.to_path_buf(),
            lock_path,
            file: Some(file),
            committed: false,
        };

        // CAS check under mutual exclusion
        if let Some(expected) = expected_old {
            let actual = if ref_path.exists() {
                let content = fs::read_to_string(ref_path)?;
                Some(ReferenceTarget::parse(&content)?)
            } else {
                None
            };

            if actual.as_ref() != Some(expected) {
                return Err(RefError::CasMismatch {
                    name: ref_path.display().to_string(),
                    expected: Some(format!("{:?}", expected)),
                    actual: actual.map(|a| format!("{:?}", a)),
                });
            }
        }

        Ok(lock)
    }

    pub fn write_target(&mut self, target: &ReferenceTarget) -> Result<(), RefError> {
        let file = self.file.as_mut().ok_or_else(|| {
            RefError::Io(std::io::Error::other(
                "lockfile already committed or closed",
            ))
        })?;

        let content = match target {
            ReferenceTarget::Symbolic(target_ref) => format!("ref: {}\n", target_ref),
            ReferenceTarget::Direct(oid) => format!("{}\n", oid.to_hex()),
        };

        file.write_all(content.as_bytes())?;
        file.flush()?;
        file.sync_all()?;
        Ok(())
    }

    pub fn commit(mut self) -> Result<(), RefError> {
        // Drop file handle first
        drop(self.file.take());

        fs::rename(&self.lock_path, &self.ref_path)?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for RefLock {
    fn drop(&mut self) {
        if !self.committed {
            drop(self.file.take());
            let _ = fs::remove_file(&self.lock_path);
        }
    }
}
