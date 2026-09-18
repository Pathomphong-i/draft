use super::id::ObjectId;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CasError {
    #[error("Object not found: {0}")]
    ObjectNotFound(ObjectId),

    #[error("Object integrity verification failed for {id}: expected {id}, computed {computed}")]
    HashMismatch { id: ObjectId, computed: ObjectId },

    #[error("Corrupt object {0}: {1}")]
    CorruptObject(ObjectId, String),

    #[error("Invalid object ID hex string: '{0}' ({1})")]
    InvalidObjectId(String, String),

    #[error("Invalid object framing header: {0}")]
    InvalidHeader(String),

    #[error(
        "Object payload size mismatch: header declared {declared} bytes, found {actual} bytes"
    )]
    SizeMismatch { declared: usize, actual: usize },

    #[error("Unknown object type: '{0}'")]
    UnknownObjectType(String),

    #[error("Compression error: {0}")]
    Compression(#[source] std::io::Error),

    #[error("Decompression error: {0}")]
    Decompression(#[source] std::io::Error),

    #[error("CAS I/O error at '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to persist temporary object file to destination '{target}': {error}")]
    PersistFailed { target: PathBuf, error: String },
}
