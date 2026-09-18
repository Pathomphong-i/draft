//! Autonomous entanglement live sync and Cronos background synchronization daemon for Daft VCS.

pub mod cronos;
pub mod entangle;
pub mod error;

pub use cronos::{
    ConflictLogger, ConflictRecord, CronosConfig, CronosDaemon, CronosStatus, DaemonInfo,
    DaemonManager, RecoveryReport, StopResult, SyncLogEntry, SyncStrategy, TickResult, WalEngine,
    WalRecord, WalStep, WatchedDimension,
};
pub use entangle::{
    AuditLogger, EchoCancellation, EntangleDirection, EntangleEngine, EntangleEvent, EntangleRule,
    FileProvenance, PropagationResult, Propagator, RuleStorage,
};
pub use error::SyncError;
