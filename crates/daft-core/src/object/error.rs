use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ObjectError {
    #[error("Malformed envelope: {0}")]
    MalformedEnvelope(String),

    #[error("Unknown object type: '{0}'")]
    UnknownType(String),

    #[error("Object size mismatch: declared {declared} bytes, but payload is {actual} bytes")]
    SizeMismatch { declared: usize, actual: usize },

    #[error("Malformed header: {0}")]
    MalformedHeader(String),

    #[error("Malformed signature: {0}")]
    MalformedSignature(String),

    #[error("Invalid mode string '{0}': {1}")]
    InvalidMode(String, String),

    #[error("Invalid tree entry name: '{0}'")]
    InvalidEntryName(String),

    #[error("Tree entries out of order: '{0}' followed by '{1}'")]
    TreeOutOfOrder(String, String),

    #[error("Duplicate tree entry name: '{0}'")]
    DuplicateTreeEntry(String),

    #[error("Unexpected end of tree data at byte offset {0}")]
    TruncatedTree(usize),

    #[error("Invalid SHA-256 hash: {0}")]
    InvalidHash(String),

    #[error("Missing required commit header: '{0}'")]
    MissingCommitHeader(String),

    #[error("UTF-8 decoding error: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),

    #[error("Integer parse error: {0}")]
    ParseIntError(#[from] std::num::ParseIntError),
}
