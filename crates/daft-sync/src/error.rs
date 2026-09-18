use daft_awareness::AwarenessError;
use daft_convergence::ConvergenceError;
use daft_core::DaftError;
use daft_dimension::DimensionError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SyncError {
    #[error("Dimension not found: {0}")]
    DimensionNotFound(String),

    #[error("Rule not found: {0}")]
    RuleNotFound(String),

    #[error("Rule already exists: {0}")]
    RuleAlreadyExists(String),

    #[error("Invalid rule: {0}")]
    InvalidRule(String),

    #[error("Daemon already running with PID {pid}")]
    DaemonAlreadyRunning { pid: u32 },

    #[error("Daemon not running")]
    DaemonNotRunning,

    #[error("Daemon process error: {0}")]
    ProcessError(String),

    #[error("WAL error: {0}")]
    WalError(String),

    #[error("Lock error: {0}")]
    LockError(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Core VCS error: {0}")]
    Core(#[from] DaftError),

    #[error("Dimension error: {0}")]
    Dimension(#[from] DimensionError),

    #[error("Awareness error: {0}")]
    Awareness(#[from] AwarenessError),

    #[error("Convergence error: {0}")]
    Convergence(#[from] ConvergenceError),

    #[error("{0}")]
    General(String),
}
