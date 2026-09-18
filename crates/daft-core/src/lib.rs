pub mod branch;
pub mod cas;
pub mod checkout;
pub mod diff;
pub mod error;
pub mod graph;
pub mod history;
pub mod index;
pub mod init;
pub mod maintenance;
pub mod merge;
pub mod object;
pub mod plumbing;
pub mod reflog;
pub mod refs;
pub mod repo;
pub mod tag;
pub mod worktree;

pub use cas::{ObjectId, ObjectStore, ObjectType, RawObject};
pub use error::{DaftError, Result};
pub use graph::{
    all_merge_bases, is_ancestor, merge_base, merge_base_octopus, walk_commits, CommitAccessor,
    MemoryCommitGraph, StoreCommitGraph,
};
pub use index::{Index, IndexEntry, IndexLock, IndexTime, Stage};
pub use init::{init, InitOptions};
pub use object::{Blob, Commit, FileMode, Object, Signature, Tag, Tree, TreeEntry};
pub use reflog::{ReflogEntry, ReflogManager};
pub use refs::{RefManager, Reference, ReferenceTarget, ReferenceType};
pub use repo::Repository;
pub use worktree::DaftIgnore;
