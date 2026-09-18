use daft_core::cas::CasError;
use daft_core::error::{IndexError, RefError};
use daft_core::graph::GraphError;
use daft_core::DaftError;
use daft_dimension::DimensionError;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AwarenessError {
    #[error("Dimension '{0}' not found")]
    DimensionNotFound(String),

    #[error("File '{path}' not found in dimension '{dimension}'")]
    FileNotFoundInDimension { path: PathBuf, dimension: String },

    #[error("Failed to find common ancestor between '{dim1}' and '{dim2}'")]
    NoCommonAncestor { dim1: String, dim2: String },

    #[error("Territory lock error: {0}")]
    TerritoryLock(String),

    #[error("Territory conflict: {0}")]
    TerritoryConflict(String),

    #[error("I/O error during awareness operation: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Core VCS error: {0}")]
    Core(#[from] DaftError),

    #[error("Dimension engine error: {0}")]
    Dimension(#[from] DimensionError),

    #[error("Index error: {0}")]
    Index(#[from] IndexError),

    #[error("CAS error: {0}")]
    Cas(#[from] CasError),

    #[error("Ref error: {0}")]
    Ref(#[from] RefError),

    #[error("Graph traversal error: {0}")]
    Graph(#[from] GraphError),

    #[error("General error: {0}")]
    General(String),
}
