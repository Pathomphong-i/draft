//! Daft Dimension Engine (`daft-dimension`).
//!
//! Provides first-class parallel dimension workspaces, OS-native Copy-on-Write storage,
//! fine-grained per-dimension locking, uncommitted live state forking, and immutable snapshots.

pub mod cow;
pub mod dimension;
pub mod fork;
pub mod lock;
pub mod manager;
pub mod snapshot;
pub mod worktree;

pub use cow::{detect_best_strategy, CowEngine, CowStrategy};
pub use dimension::{
    Dimension, DimensionError, DimensionInfo, DimensionListItem, DimensionMetadata, DimensionStatus,
};
pub use lock::{DimensionLockError, DimensionLockGuard, LockInfo};
pub use manager::DimensionManager;
pub use snapshot::{Snapshot, SnapshotListItem, SnapshotManager};
pub use worktree::DimensionRepository;
