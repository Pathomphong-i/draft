use crate::error::SyncError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonInfo {
    pub pid: u32,
    pub status: String,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopResult {
    pub pid: Option<u32>,
    pub success: bool,
    pub message: String,
}

pub struct DaemonManager {
    dft_dir: PathBuf,
    cronos_dir: PathBuf,
    lock_file: Arc<Mutex<Option<fs::File>>>,
}

impl DaemonManager {
    pub fn new(dft_dir: &Path) -> Self {
        Self {
            dft_dir: dft_dir.to_path_buf(),
            cronos_dir: dft_dir.join("cronos"),
            lock_file: Arc::new(Mutex::new(None)),
        }
    }

    fn pid_file(&self) -> PathBuf {
        self.cronos_dir.join("daemon.pid")
    }

    fn legacy_pid_file(&self) -> PathBuf {
        self.dft_dir.join("cronos.pid")
    }

    fn state_file(&self) -> PathBuf {
        self.cronos_dir.join("state.json")
    }

    pub fn lock_file(&self) -> PathBuf {
        self.cronos_dir.join("daemon.lock")
    }

    pub fn is_pid_alive(pid: u32) -> bool {
        #[cfg(unix)]
        {
            unsafe { libc::kill(pid as i32, 0) == 0 }
        }
        #[cfg(not(unix))]
        {
            false
        }
    }

    pub fn get_running_pid(&self) -> Option<u32> {
        let pids = [self.pid_file(), self.legacy_pid_file()];
        for p in &pids {
            if p.exists() {
                if let Ok(content) = fs::read_to_string(p) {
                    if let Ok(pid) = content.trim().parse::<u32>() {
                        if Self::is_pid_alive(pid) {
                            return Some(pid);
                        }
                    }
                }
            }
        }
        None
    }

    pub fn is_running(&self) -> bool {
        self.get_running_pid().is_some()
    }

    pub fn start(&self) -> Result<DaemonInfo, SyncError> {
        fs::create_dir_all(&self.cronos_dir)?;

        if let Some(pid) = self.get_running_pid() {
            return Err(SyncError::DaemonAlreadyRunning { pid });
        }

        let lock_path = self.lock_file();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&lock_path)?;

        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let ret = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if ret != 0 {
                let pid = self.get_running_pid().unwrap_or(0);
                return Err(SyncError::DaemonAlreadyRunning { pid });
            }
        }

        let current_pid = std::process::id();

        // Inscribe lock metadata
        {
            use std::io::{Seek, SeekFrom, Write};
            let mut f = &file;
            let _ = f.set_len(0);
            let _ = f.seek(SeekFrom::Start(0));
            let lock_data = serde_json::json!({
                "pid": current_pid,
                "acquired_at": Utc::now(),
            });
            let _ = f.write_all(lock_data.to_string().as_bytes());
            let _ = f.flush();
        }

        // Retain lock file descriptor
        {
            let mut guard = self.lock_file.lock().unwrap();
            *guard = Some(file);
        }

        // Write PID files
        let pid_str = format!("{}\n", current_pid);
        let _ = fs::write(self.pid_file(), &pid_str);
        let _ = fs::write(self.legacy_pid_file(), &pid_str);

        // Update state file
        let state = serde_json::json!({
            "status": "running",
            "pid": current_pid,
            "started_at": Utc::now(),
        });
        fs::write(self.state_file(), serde_json::to_string_pretty(&state)?)?;

        Ok(DaemonInfo {
            pid: current_pid,
            status: "running".to_string(),
            started_at: Utc::now(),
        })
    }

    pub fn stop(&self) -> Result<StopResult, SyncError> {
        let pid_opt = self.get_running_pid();

        #[cfg(unix)]
        if let Some(pid) = pid_opt {
            if pid != std::process::id() {
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
                if Self::is_pid_alive(pid) {
                    unsafe {
                        libc::kill(pid as i32, libc::SIGKILL);
                    }
                }
            }
        }

        // Release kernel advisory lock if held locally
        {
            let mut guard = self.lock_file.lock().unwrap();
            if let Some(file) = guard.take() {
                #[cfg(unix)]
                {
                    use std::os::unix::io::AsRawFd;
                    unsafe {
                        libc::flock(file.as_raw_fd(), libc::LOCK_UN);
                    }
                }
            }
        }

        // Clean up PID files and lock file
        let _ = fs::remove_file(self.pid_file());
        let _ = fs::remove_file(self.legacy_pid_file());
        let _ = fs::remove_file(self.lock_file());

        // Update state file
        let state = serde_json::json!({
            "status": "stopped",
            "stopped_at": Utc::now(),
        });
        let _ = fs::write(self.state_file(), serde_json::to_string_pretty(&state)?);

        Ok(StopResult {
            pid: pid_opt,
            success: true,
            message: "Cronos background daemon stopped".to_string(),
        })
    }
}

impl Drop for DaemonManager {
    fn drop(&mut self) {
        let mut guard = self.lock_file.lock().unwrap();
        if let Some(file) = guard.take() {
            #[cfg(unix)]
            {
                use std::os::unix::io::AsRawFd;
                unsafe {
                    libc::flock(file.as_raw_fd(), libc::LOCK_UN);
                }
            }
        }
    }
}
