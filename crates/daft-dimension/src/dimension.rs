//! Dimension data models, metadata schemas, and error definitions.

use crate::lock::DimensionLockError;
use daft_core::cas::ObjectId;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use thiserror::Error;

/// Dimension error categories and descriptive messages.
#[derive(Error, Debug)]
pub enum DimensionError {
    #[error("Dimension '{0}' already exists")]
    AlreadyExists(String),

    #[error("Dimension '{0}' does not exist")]
    NotFound(String),

    #[error("Invalid dimension name: '{0}'")]
    InvalidName(String),

    #[error("Cannot destroy active dimension without --force")]
    CannotDestroyActive(String),

    #[error("Cannot destroy dimension with uncommitted changes without --force")]
    CannotDestroyDirty(String),

    #[error("Cannot destroy mainline dimension")]
    MainlineReserved,

    #[error("Cannot enter dimension in bare repository")]
    BareRepository,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Core VCS error: {0}")]
    Core(#[from] daft_core::DaftError),

    #[error("Index error: {0}")]
    Index(#[from] daft_core::error::IndexError),

    #[error("CAS error: {0}")]
    Cas(#[from] daft_core::cas::CasError),

    #[error("Ref error: {0}")]
    Ref(#[from] daft_core::error::RefError),

    #[error("Lock error: {0}")]
    Lock(#[from] DimensionLockError),

    #[error("{0}")]
    General(String),
}

/// Dynamic status of a dimension workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionStatus {
    Clean,
    Dirty,
}

impl fmt::Display for DimensionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DimensionStatus::Clean => write!(f, "clean"),
            DimensionStatus::Dirty => write!(f, "dirty"),
        }
    }
}

/// Canonical metadata persisted to `dimension.json` and dual-written to `meta.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DimensionMetadata {
    pub name: String,
    pub creator: String,
    pub branch: String,
    pub cow_mode: String,
    pub status: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_commit: Option<ObjectId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Lightweight item representation for listing operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DimensionListItem {
    pub name: String,
    pub branch: String,
    pub active: bool,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cow_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_usage_bytes: Option<u64>,
}

/// Rich dimension representation for inspection queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionInfo {
    pub name: String,
    pub branch: String,
    pub active: bool,
    pub status: DimensionStatus,
    pub head_commit: Option<ObjectId>,
    pub parent: Option<String>,
    pub created_at: String,
    pub cow_mode: String,
    pub disk_usage_bytes: u64,
}

/// In-memory handle to an initialized dimension directory on disk.
#[derive(Debug, Clone)]
pub struct Dimension {
    pub metadata: DimensionMetadata,
    pub root_dir: PathBuf,
    pub workspace_dir: PathBuf,
    pub head_path: PathBuf,
    pub index_path: PathBuf,
    pub lock_path: PathBuf,
}

impl Dimension {
    pub fn new(root_dir: PathBuf, metadata: DimensionMetadata) -> Self {
        let workspace_dir = root_dir.join("workspace");
        let head_path = root_dir.join("HEAD");
        let index_path = root_dir.join("index");
        let lock_path = root_dir.join(".lock");
        Self {
            metadata,
            root_dir,
            workspace_dir,
            head_path,
            index_path,
            lock_path,
        }
    }
}
