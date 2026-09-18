use crate::cas::ObjectId;
use crate::object::ObjectError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("Commit not found: {0}")]
    CommitNotFound(ObjectId),

    #[error("Object error: {0}")]
    Object(#[from] ObjectError),

    #[error("Cycle detected in commit graph at commit: {0}")]
    CycleDetected(ObjectId),

    #[error("Storage CAS error: {0}")]
    Cas(String),
}
