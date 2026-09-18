use daft_awareness::AwarenessError;
use daft_core::DaftError;
use daft_dimension::DimensionError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Agent '{0}' already exists")]
    AgentAlreadyExists(String),

    #[error("Agent '{0}' not found")]
    AgentNotFound(String),

    #[error("Invalid agent name '{0}': {1}")]
    InvalidAgentName(String, String),

    #[error("Dimension '{0}' not found")]
    DimensionNotFound(String),

    #[error("Message '{0}' not found")]
    MessageNotFound(String),

    #[error("Registry lock error: {0}")]
    LockError(String),

    #[error("I/O error during agent operation: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Core VCS error: {0}")]
    Core(#[from] DaftError),

    #[error("Dimension error: {0}")]
    Dimension(#[from] DimensionError),

    #[error("Awareness error: {0}")]
    Awareness(#[from] AwarenessError),

    #[error("General agent error: {0}")]
    General(String),
}
