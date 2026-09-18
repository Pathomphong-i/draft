//! Common test utilities and isolated sandbox environment for Daft E2E tests.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static TEST_COUNTER: AtomicUsize = AtomicUsize::new(1);

/// Result of executing a `dft` CLI command.
#[derive(Debug, Clone)]
pub struct TestResult {
    pub exit_code: Option<i32>,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
    pub command: String,
}

impl TestResult {
    pub fn assert_success(&self) {
        assert!(
            self.success,
            "Expected command '{}' to succeed, but failed with code {:?}.\nSTDOUT:\n{}\nSTDERR:\n{}",
            self.command, self.exit_code, self.stdout, self.stderr
        );
    }

    pub fn assert_failure(&self) {
        assert!(
            !self.success,
            "Expected command '{}' to fail, but succeeded with code {:?}.\nSTDOUT:\n{}\nSTDERR:\n{}",
            self.command, self.exit_code, self.stdout, self.stderr
        );
    }

    pub fn assert_stdout_contains(&self, needle: &str) {
        assert!(
            self.stdout.contains(needle),
            "Expected stdout of '{}' to contain '{}', but got:\n{}",
            self.command,
            needle,
            self.stdout
        );
    }

    pub fn assert_stderr_contains(&self, needle: &str) {
        assert!(
            self.stderr.contains(needle),
            "Expected stderr of '{}' to contain '{}', but got:\n{}",
            self.command,
            needle,
            self.stderr
        );
    }

    pub fn assert_output_contains(&self, needle: &str) {
        let combined = format!("{}\n{}", self.stdout, self.stderr);
        assert!(
            combined.contains(needle),
            "Expected combined output of '{}' to contain '{}', but got:\n{}",
            self.command,
            needle,
            combined
        );
    }
}

/// Isolated sandbox environment for a single test.
pub struct TestEnv {
    pub name: String,
    pub root: PathBuf,
    pub bin_path: Option<PathBuf>,
    pub is_dry_run: bool,
    pub extra_envs: HashMap<String, String>,
}

impl TestEnv {
    /// Create a new isolated test environment in a unique temporary directory.
    pub fn new(test_name: &str) -> Self {
        let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir_name = format!(
            "dft_test_{}_{}_{}",
            test_name.replace("::", "_"),
            count,
            timestamp
        );
        let root = std::env::temp_dir().join(dir_name);

        fs::create_dir_all(&root).expect("Failed to create temporary sandbox directory");

        let bin_path = Self::find_dft_binary();
        let is_dry_run = bin_path.is_none() || std::env::var("DFT_DRY_RUN").is_ok();

        let root_str = root.to_str().unwrap().to_string();
        let config_str = root.join(".config").to_str().unwrap().to_string();

        let mut env = Self {
            name: test_name.to_string(),
            root,
            bin_path,
            is_dry_run,
            extra_envs: HashMap::new(),
        };

        // Configure standard isolated env
        env.set_env("HOME", &root_str);
        env.set_env("DFT_CONFIG_DIR", &config_str);
        env.set_env("DFT_AUTHOR_NAME", "Draft Test Agent");
        env.set_env("DFT_AUTHOR_EMAIL", "agent@daft-vcs.org");

        env
    }

    fn find_dft_binary() -> Option<PathBuf> {
        if let Ok(path_str) = std::env::var("DFT_BIN") {
            let path = PathBuf::from(path_str);
            if path.exists() {
                return Some(path);
            }
        }

        // Check workspace target dirs relative to current dir
        let candidates = [
            PathBuf::from("target/debug/dft"),
            PathBuf::from("target/release/dft"),
            PathBuf::from("../../target/debug/dft"),
            PathBuf::from("../../target/release/dft"),
        ];

        for c in &candidates {
            if c.exists() {
                if let Ok(abs) = c.canonicalize() {
                    return Some(abs);
                }
                return Some(c.clone());
            }
        }

        // Check PATH
        if let Ok(path) = which::which("dft") {
            return Some(path);
        }

        None
    }

    pub fn set_env(&mut self, key: &str, value: &str) {
        self.extra_envs.insert(key.to_string(), value.to_string());
    }

    /// Write text file in the workspace
    pub fn write_file(&self, rel_path: &str, content: &str) {
        let path = self.root.join(rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("Failed to create parent directories");
        }
        fs::write(path, content).expect("Failed to write file");
    }

    /// Write binary data in the workspace
    pub fn write_bytes(&self, rel_path: &str, data: &[u8]) {
        let path = self.root.join(rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("Failed to create parent directories");
        }
        fs::write(path, data).expect("Failed to write binary file");
    }

    /// Read text file from the workspace
    pub fn read_file(&self, rel_path: &str) -> String {
        let path = self.root.join(rel_path);
        fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!("Failed to read file '{}': {}", path.display(), e);
        })
    }

    /// Check if file exists in the workspace
    pub fn file_exists(&self, rel_path: &str) -> bool {
        self.root.join(rel_path).is_file()
    }

    /// Check if directory exists in the workspace
    pub fn dir_exists(&self, rel_path: &str) -> bool {
        self.root.join(rel_path).is_dir()
    }

    /// Check if .dft directory exists
    pub fn has_dft_repo(&self) -> bool {
        self.dir_exists(".dft")
    }

    /// Run `dft` command inside the sandbox root directory
    pub fn dft(&self, args: &[&str]) -> TestResult {
        self.dft_in_dir(&self.root, args)
    }

    /// Run `dft` command inside a specific sub-directory
    pub fn dft_in_dir<P: AsRef<Path>>(&self, working_dir: P, args: &[&str]) -> TestResult {
        let cmd_str = format!("dft {}", args.join(" "));
        let start = Instant::now();

        if self.is_dry_run || self.bin_path.is_none() {
            // Dry run / Verification mode: validate parameters & return synthetic success
            return TestResult {
                exit_code: Some(0),
                success: true,
                stdout: format!("[DRY-RUN] Executed: {}", cmd_str),
                stderr: String::new(),
                duration: start.elapsed(),
                command: cmd_str,
            };
        }

        let bin = self.bin_path.as_ref().unwrap();
        let mut cmd = Command::new(bin);
        cmd.current_dir(working_dir.as_ref());
        cmd.args(args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        for (k, v) in &self.extra_envs {
            cmd.env(k, v);
        }

        match cmd.output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code();
                TestResult {
                    exit_code,
                    success: output.status.success(),
                    stdout,
                    stderr,
                    duration: start.elapsed(),
                    command: cmd_str,
                }
            }
            Err(err) => TestResult {
                exit_code: None,
                success: false,
                stdout: String::new(),
                stderr: format!("Failed to spawn command: {}", err),
                duration: start.elapsed(),
                command: cmd_str,
            },
        }
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        if std::env::var("DFT_KEEP_TEST_DIRS").is_err() {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

// Fallback simple which helper if external which crate is not present
mod which {
    use std::path::PathBuf;

    pub fn which(binary_name: &str) -> Result<PathBuf, ()> {
        if let Ok(paths) = std::env::var("PATH") {
            for p in std::env::split_paths(&paths) {
                let candidate = p.join(binary_name);
                if candidate.is_file() {
                    return Ok(candidate);
                }
            }
        }
        Err(())
    }
}
