//! Fine-Grained Per-Dimension Concurrency & Locking for Daft.
//!
//! Provides advisory kernel-level file locking (`flock`) paired with JSON process metadata
//! for automated crash recovery and full agent observability.
//! Cross-dimension operations never collide: Dimension A's lock is completely independent
//! of Dimension B's lock.

use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DimensionLockError {
    #[error("Dimension '{0}' not found")]
    DimensionNotFound(String),

    #[error("Dimension '{dimension}' lock is held by another process: {holder:?}")]
    LockContention {
        dimension: String,
        holder: Option<LockInfo>,
    },

    #[error("I/O error during lock operation: {0}")]
    Io(#[from] io::Error),

    #[error("Lock metadata JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Metadata recorded inside the dimension `.lock` file while held.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockInfo {
    pub pid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    pub operation: String,
    pub acquired_at: String,
}

/// RAII lock guard for an exclusive dimension lock.
/// Automatically unlocks the advisory kernel `flock` and truncates metadata on `drop`.
#[derive(Debug)]
pub struct DimensionLockGuard {
    dimension_id: String,
    lock_path: PathBuf,
    file: Option<File>,
}

impl DimensionLockGuard {
    /// Attempts non-blocking acquisition of the dimension lock.
    pub fn try_acquire(
        dimensions_dir: &Path,
        dimension_id: &str,
        agent_id: Option<&str>,
        operation: &str,
    ) -> Result<Self, DimensionLockError> {
        Self::acquire_timeout(
            dimensions_dir,
            dimension_id,
            agent_id,
            operation,
            Duration::ZERO,
        )
    }

    /// Acquires the dimension lock, waiting up to `timeout` before failing.
    pub fn acquire_timeout(
        dimensions_dir: &Path,
        dimension_id: &str,
        agent_id: Option<&str>,
        operation: &str,
        timeout: Duration,
    ) -> Result<Self, DimensionLockError> {
        let dim_dir = dimensions_dir.join(dimension_id);
        if !dim_dir.exists() {
            return Err(DimensionLockError::DimensionNotFound(
                dimension_id.to_string(),
            ));
        }

        let lock_path = dim_dir.join(".lock");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)?;

        let start = Instant::now();

        loop {
            #[cfg(unix)]
            {
                use std::os::unix::io::AsRawFd;
                let ret = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };

                if ret == 0 {
                    // Lock acquired! Inscribe process metadata
                    let mut file = file;
                    let info = LockInfo {
                        pid: std::process::id(),
                        agent_id: agent_id.map(|s| s.to_string()),
                        operation: operation.to_string(),
                        acquired_at: chrono::Utc::now().to_rfc3339(),
                    };
                    let json = serde_json::to_string_pretty(&info).unwrap_or_default();
                    let _ = file.set_len(0);
                    let _ = file.seek(SeekFrom::Start(0));
                    let _ = file.write_all(json.as_bytes());
                    let _ = file.flush();

                    return Ok(Self {
                        dimension_id: dimension_id.to_string(),
                        lock_path,
                        file: Some(file),
                    });
                }
            }

            #[cfg(not(unix))]
            {
                return Ok(Self {
                    dimension_id: dimension_id.to_string(),
                    lock_path,
                    file: Some(file),
                });
            }

            if start.elapsed() >= timeout {
                // Read current holder info if available for diagnostic messaging
                let mut content = String::new();
                if let Ok(mut reader) = File::open(&lock_path) {
                    let _ = reader.read_to_string(&mut content);
                }
                let holder: Option<LockInfo> = serde_json::from_str(&content).ok();

                return Err(DimensionLockError::LockContention {
                    dimension: dimension_id.to_string(),
                    holder,
                });
            }

            std::thread::sleep(Duration::from_millis(15));
        }
    }

    /// Explicitly releases the lock ahead of RAII drop.
    pub fn release(mut self) {
        self.unlock_internal();
    }

    fn unlock_internal(&mut self) {
        if let Some(file) = self.file.take() {
            #[cfg(unix)]
            {
                use std::os::unix::io::AsRawFd;
                let _ = file.set_len(0);
                unsafe {
                    libc::flock(file.as_raw_fd(), libc::LOCK_UN);
                }
            }
        }
    }

    pub fn dimension_id(&self) -> &str {
        &self.dimension_id
    }

    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }
}

impl Drop for DimensionLockGuard {
    fn drop(&mut self) {
        self.unlock_internal();
    }
}
