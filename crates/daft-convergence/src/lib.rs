//! Daft Convergence Engine (`daft-convergence`).
//!
//! Provides multi-dimension convergence, collapse, cascade, weave,
//! and splice operations for Daft VCS.

pub mod cascade;
pub mod collapse;
pub mod converge;
pub mod error;
pub mod lock_order;
pub mod splice;
pub mod strategy;
pub mod weave;

pub use cascade::{CascadeEngine, CascadeOutcome, CascadeStep};
pub use collapse::{CollapseEngine, CollapseOptions, CollapseOutcome};
pub use converge::{ConvergeEngine, ConvergeOptions, ConvergeOutcome};
pub use error::ConvergenceError;
pub use lock_order::MultiDimensionLockGuard;
pub use splice::{SpliceEngine, SpliceOutcome};
pub use strategy::MergeStrategy;
pub use weave::{WeaveEngine, WeaveOutcome};
