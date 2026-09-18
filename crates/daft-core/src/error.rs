use std::path::PathBuf;
use thiserror::Error;

pub use crate::cas::CasError;
pub use crate::graph::GraphError;
pub use crate::object::ObjectError;

#[derive(Error, Debug)]
pub enum IndexError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Index magic header mismatch: expected 'DIRC', found {0:?}")]
    BadMagic([u8; 4]),

    #[error("Unsupported index version: {0} (supported: 2)")]
    UnsupportedVersion(u32),

    #[error("Corrupt index checksum: expected {expected}, calculated {actual}")]
    CorruptChecksum { expected: String, actual: String },

    #[error("Index entry truncated at byte offset {0}")]
    TruncatedEntry(usize),

    #[error("Index is currently locked by another process: {0}")]
    IndexLocked(PathBuf),

    #[error("Attempted operation on already committed index lock")]
    LockAlreadyCommitted,

    #[error("Invalid stage: {0}")]
    InvalidStage(u16),

    #[error("Invalid path in index entry: {0}")]
    InvalidPath(String),
}

#[derive(Error, Debug)]
pub enum RefError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid reference name '{0}'")]
    InvalidRefName(String),

    #[error("Reference '{0}' not found")]
    RefNotFound(String),

    #[error("Reference '{0}' is on an unborn branch")]
    UnbornBranch(String),

    #[error("Symbolic reference loop detected while resolving '{0}'")]
    SymbolicRefLoop(String),

    #[error("Reference '{0}' locked by another process")]
    RefLocked(String),

    #[error("Compare-and-swap mismatch for ref '{name}': expected {expected:?}, found {actual:?}")]
    CasMismatch {
        name: String,
        expected: Option<String>,
        actual: Option<String>,
    },
}

#[derive(Error, Debug)]
pub enum ReflogError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Malformed reflog line: '{0}'")]
    MalformedLine(String),

    #[error("CAS error in reflog: {0}")]
    Cas(#[from] CasError),
}

#[derive(Error, Debug)]
pub enum RepoError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Directory '{0}' is not a Daft repository")]
    NotARepository(PathBuf),

    #[error("Repository already exists at '{0}'")]
    AlreadyExists(PathBuf),

    #[error("Index error: {0}")]
    Index(#[from] IndexError),

    #[error("Ref error: {0}")]
    Ref(#[from] RefError),

    #[error("CAS error: {0}")]
    Cas(#[from] CasError),
}

#[derive(Error, Debug)]
pub enum DaftError {
    #[error("CAS error: {0}")]
    Cas(#[from] CasError),

    #[error("Object error: {0}")]
    Object(#[from] ObjectError),

    #[error("Graph error: {0}")]
    Graph(#[from] GraphError),

    #[error("Index error: {0}")]
    Index(#[from] IndexError),

    #[error("Ref error: {0}")]
    Ref(#[from] RefError),

    #[error("Reflog error: {0}")]
    Reflog(#[from] ReflogError),

    #[error("Repository error: {0}")]
    Repo(#[from] RepoError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, DaftError>;
