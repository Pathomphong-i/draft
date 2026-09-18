//! Daft Awareness Engine (`daft-awareness`).
//!
//! Provides cross-dimension radar, non-destructive observation, predictive conflict
//! detection, divergence entropy, and territory governance for Daft VCS.

pub mod entropy;
pub mod error;
pub mod foresee;
pub mod observe;
pub mod radar;
pub mod territory;

pub use entropy::{EntropyMetrics, EntropySubsystem};
pub use error::AwarenessError;
pub use foresee::{ConflictHunk, ConflictType, ForeseeReport, ForeseeSubsystem};
pub use observe::{DimensionStatusReport, ObserveSubsystem};
pub use radar::{PathActivity, RadarEvent, RadarReport, RadarSubsystem, WatchHandle};
pub use territory::{
    matches_glob, normalize_glob, parse_ttl_string, patterns_overlap, AuditReport, Claim,
    ClaimCollision, Fence, FenceConflict, TerritoryManager, TerritoryViolation, ViolationType,
};
