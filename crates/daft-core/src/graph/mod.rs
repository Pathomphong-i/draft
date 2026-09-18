pub mod ancestry;
pub mod error;
pub mod merge_base;
pub mod traverse;

pub use ancestry::is_ancestor;
pub use error::GraphError;
pub use merge_base::{all_merge_bases, merge_base, merge_base_octopus};
pub use traverse::walk_commits;

use crate::cas::{ObjectId, ObjectStore, ObjectType};
use crate::object::Commit;
use std::collections::HashMap;
use std::sync::Arc;

/// Interface for accessing commit objects in a DAG.
pub trait CommitAccessor {
    fn get_commit(&self, id: &ObjectId) -> Result<Arc<Commit>, GraphError>;
}

/// In-memory commit graph for fast testing and manipulation without disk I/O.
#[derive(Default, Clone)]
pub struct MemoryCommitGraph {
    commits: HashMap<ObjectId, Arc<Commit>>,
}

impl MemoryCommitGraph {
    pub fn new() -> Self {
        Self {
            commits: HashMap::new(),
        }
    }

    pub fn add(&mut self, commit: Commit) -> ObjectId {
        let envelope = commit.serialize();
        // Construct commit envelope for hashing: "commit <size>\0<payload>"
        let header = format!("commit {}\0", envelope.len());
        let mut framed = Vec::with_capacity(header.len() + envelope.len());
        framed.extend_from_slice(header.as_bytes());
        framed.extend_from_slice(&envelope);
        let oid = ObjectId::hash(&framed);
        self.commits.insert(oid, Arc::new(commit));
        oid
    }

    pub fn insert_with_id(&mut self, id: ObjectId, commit: Commit) {
        self.commits.insert(id, Arc::new(commit));
    }
}

impl CommitAccessor for MemoryCommitGraph {
    fn get_commit(&self, id: &ObjectId) -> Result<Arc<Commit>, GraphError> {
        self.commits
            .get(id)
            .cloned()
            .ok_or(GraphError::CommitNotFound(*id))
    }
}

/// Adapter allowing commit graph traversal directly on a CAS ObjectStore.
pub struct StoreCommitGraph<'a> {
    store: &'a ObjectStore,
}

impl<'a> StoreCommitGraph<'a> {
    pub fn new(store: &'a ObjectStore) -> Self {
        Self { store }
    }
}

impl<'a> CommitAccessor for StoreCommitGraph<'a> {
    fn get_commit(&self, id: &ObjectId) -> Result<Arc<Commit>, GraphError> {
        let raw = self
            .store
            .read_raw(id)
            .map_err(|e| GraphError::Cas(e.to_string()))?;
        if raw.object_type != ObjectType::Commit {
            return Err(GraphError::Cas(format!(
                "object {} is {} not commit",
                id, raw.object_type
            )));
        }
        let commit = Commit::deserialize(&raw.data)?;
        Ok(Arc::new(commit))
    }
}
