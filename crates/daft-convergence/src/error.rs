use daft_awareness::AwarenessError;
use daft_core::cas::CasError;
use daft_core::error::IndexError;
use daft_core::graph::GraphError;
use daft_core::object::ObjectError;
use daft_core::DaftError;
use daft_dimension::DimensionError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConvergenceError {
    #[error("Dimension error: {0}")]
    Dimension(#[from] DimensionError),

    #[error("Awareness error: {0}")]
    Awareness(#[from] AwarenessError),

    #[error("Core VCS error: {0}")]
    Core(#[from] DaftError),

    #[error("CAS error: {0}")]
    Cas(#[from] CasError),

    #[error("Object error: {0}")]
    Object(#[from] ObjectError),

    #[error("Index error: {0}")]
    Index(#[from] IndexError),

    #[error("Graph error: {0}")]
    Graph(#[from] GraphError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Dimension '{0}' does not exist")]
    DimensionNotFound(String),

    #[error("Cannot converge dimension '{0}' into itself")]
    SelfConvergence(String),

    #[error("Empty dimension list provided")]
    EmptyDimensionList,

    #[error("No common ancestor found across dimensions: {0:?}")]
    NoCommonAncestor(Vec<String>),

    #[error("Merge conflicts encountered in {0} files: {1:?}")]
    Conflict(usize, Vec<String>),

    #[error(
        "Cascade aborted at dimension '{at_dimension}' due to conflict in files: {conflicts:?}"
    )]
    CascadeAborted {
        at_dimension: String,
        conflicts: Vec<String>,
        completed: Vec<String>,
    },

    #[error("Invalid commit range '{range}': {reason}")]
    InvalidCommitRange { range: String, reason: String },

    #[error("Commit '{0}' not found in dimension '{1}'")]
    CommitNotFound(String, String),

    #[error("Failed to acquire dimension lock for '{0}': {1}")]
    LockFailure(String, String),

    #[error("General convergence failure: {0}")]
    General(String),
}
