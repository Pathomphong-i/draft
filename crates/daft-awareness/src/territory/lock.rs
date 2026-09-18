use crate::error::AwarenessError;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerritoryLockInfo {
    pub pid: u32,
    pub agent_id: Option<String>,
    pub operation: String,
    pub acquired_at: String,
}

pub struct TerritoryLockGuard {
    _lock_path: PathBuf,
    file: Option<File>,
}

impl TerritoryLockGuard {
    pub fn acquire(
        territory_dir: &Path,
        agent_id: Option<&str>,
        operation: &str,
        timeout: Duration,
    ) -> Result<Self, AwarenessError> {
        let _ = std::fs::create_dir_all(territory_dir);
        let lock_path = territory_dir.join(".lock");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&lock_path)
            .map_err(|e| AwarenessError::TerritoryLock(format!("Failed to open .lock: {}", e)))?;

        let start = Instant::now();
        loop {
            #[cfg(unix)]
            {
                use std::os::unix::io::AsRawFd;
                let ret = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
                if ret == 0 {
                    let mut file = file;
                    let info = TerritoryLockInfo {
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
                        _lock_path: lock_path,
                        file: Some(file),
                    });
                }
            }

            #[cfg(not(unix))]
            {
                return Ok(Self {
                    _lock_path: lock_path,
                    file: Some(file),
                });
            }

            if start.elapsed() >= timeout {
                let mut content = String::new();
                if let Ok(mut reader) = File::open(&lock_path) {
                    let _ = reader.read_to_string(&mut content);
                }
                let info: Option<TerritoryLockInfo> = serde_json::from_str(&content).ok();
                return Err(AwarenessError::TerritoryLock(format!(
                    "Territory lock contention held by PID {:?} (agent: {:?}, op: {:?}): timed out after {}s",
                    info.as_ref().map(|i| i.pid),
                    info.as_ref().and_then(|i| i.agent_id.clone()),
                    info.as_ref().map(|i| i.operation.clone()),
                    timeout.as_secs()
                )));
            }

            std::thread::sleep(Duration::from_millis(15));
        }
    }
}

impl Drop for TerritoryLockGuard {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            #[cfg(unix)]
            {
                use std::os::unix::io::AsRawFd;
                let mut file = file;
                let _ = file.set_len(0);
                let _ = file.flush();
                let _ = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
            }
        }
    }
}
