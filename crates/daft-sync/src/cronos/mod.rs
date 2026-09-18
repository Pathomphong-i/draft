pub mod config;
pub mod conflict;
pub mod daemon;
pub mod wal;

pub use config::{ConfigStorage, CronosConfig, SyncStrategy, WatchedDimension};
pub use conflict::{ConflictLogger, ConflictRecord, SyncLogEntry};
pub use daemon::{DaemonInfo, DaemonManager, StopResult};
pub use wal::{RecoveryReport, WalEngine, WalRecord, WalStep};

use crate::entangle::EntangleEngine;
use crate::error::SyncError;
use chrono::{DateTime, Utc};
use daft_core::cas::{ObjectId, ObjectType, RawObject};
use daft_core::graph::{is_ancestor, merge_base, StoreCommitGraph};
use daft_core::merge::{build_hierarchical_tree, checkout_tree, get_signature, merge_trees_3way};
use daft_core::object::Commit;
use daft_core::refs::ReferenceTarget;
use daft_core::Repository;
use daft_dimension::{DimensionManager, DimensionRepository};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronosStatus {
    pub is_running: bool,
    pub pid: Option<u32>,
    pub interval_secs: u64,
    pub default_strategy: SyncStrategy,
    pub watched_dimensions: Vec<WatchedDimension>,
    pub next_sync_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickResult {
    pub synced_dimensions: usize,
    pub entanglements_propagated: usize,
    pub conflicts_detected: usize,
}

pub struct CronosDaemon {
    repo: Arc<Repository>,
    cronos_dir: PathBuf,
    daemon_mgr: DaemonManager,
    wal_engine: WalEngine,
}

impl CronosDaemon {
    pub fn new(repo: Arc<Repository>) -> Self {
        let cronos_dir = repo.dft_dir().join("cronos");
        let daemon_mgr = DaemonManager::new(repo.dft_dir());
        let wal_engine = WalEngine::new(&cronos_dir);
        Self {
            repo,
            cronos_dir,
            daemon_mgr,
            wal_engine,
        }
    }

    pub fn status(&self) -> Result<CronosStatus, SyncError> {
        let is_running = self.daemon_mgr.is_running();
        let pid = self.daemon_mgr.get_running_pid();
        let cfg = ConfigStorage::load_config(&self.cronos_dir)?;

        let next_sync_time = if is_running {
            Some(Utc::now() + chrono::Duration::seconds(cfg.interval_secs as i64))
        } else {
            None
        };

        Ok(CronosStatus {
            is_running,
            pid,
            interval_secs: cfg.interval_secs,
            default_strategy: cfg.default_strategy,
            watched_dimensions: cfg.watched_dimensions,
            next_sync_time,
        })
    }

    pub fn is_running(&self) -> bool {
        self.daemon_mgr.is_running()
    }

    pub fn get_running_pid(&self) -> Option<u32> {
        self.daemon_mgr.get_running_pid()
    }

    pub fn start(
        &self,
        interval: Option<Duration>,
        strategy: Option<SyncStrategy>,
    ) -> Result<DaemonInfo, SyncError> {
        let mut cfg = ConfigStorage::load_config(&self.cronos_dir)?;
        if let Some(i) = interval {
            cfg.interval_secs = i.as_secs();
        }
        if let Some(s) = strategy {
            cfg.default_strategy = s;
        }
        ConfigStorage::save_config(&self.cronos_dir, &cfg)?;

        // Execute recovery on startup
        let _ = self.wal_engine.recover();

        self.daemon_mgr.start()
    }

    pub fn stop(&self) -> Result<StopResult, SyncError> {
        self.daemon_mgr.stop()
    }

    pub fn pause(&self, dimension: &str) -> Result<(), SyncError> {
        ConfigStorage::set_paused(&self.cronos_dir, dimension, true)
    }

    pub fn resume(&self, dimension: &str) -> Result<(), SyncError> {
        ConfigStorage::set_paused(&self.cronos_dir, dimension, false)
    }

    pub fn get_config(&self) -> Result<CronosConfig, SyncError> {
        ConfigStorage::load_config(&self.cronos_dir)
    }

    pub fn set_config(&self, config: CronosConfig) -> Result<(), SyncError> {
        ConfigStorage::save_config(&self.cronos_dir, &config)
    }

    pub fn verify_wal(&self) -> Result<usize, SyncError> {
        self.wal_engine.verify()
    }

    pub fn recover(&self) -> Result<RecoveryReport, SyncError> {
        self.wal_engine.recover()
    }

    pub fn read_sync_log(&self, limit: Option<usize>) -> Result<Vec<SyncLogEntry>, SyncError> {
        ConflictLogger::read_entries(&self.cronos_dir, limit)
    }
}

#[allow(dead_code)]
enum SyncExecutionResult {
    Success {
        new_target_head: ObjectId,
        files_updated: usize,
        auto_resolved: usize,
    },
    Conflict(Vec<ConflictRecord>),
    Aborted(String),
}

impl CronosDaemon {
    fn path_matches_watched(paths: &[String], rel_path: &str) -> bool {
        if paths.is_empty() {
            return true;
        }
        for pat in paths {
            let trimmed = pat.trim();
            if !trimmed.is_empty() && daft_awareness::matches_glob(trimmed, rel_path) {
                return true;
            }
        }
        false
    }

    fn sync_dimension_pair(
        &self,
        tx_id: &str,
        watched: &WatchedDimension,
        strategy: SyncStrategy,
        _source_repo: &DimensionRepository,
        target_repo: &DimensionRepository,
        s_oid: ObjectId,
        target_head: Option<ObjectId>,
    ) -> SyncExecutionResult {
        let cas = self.repo.cas().as_ref();
        let graph = StoreCommitGraph::new(cas);

        // Read source commit
        let s_raw = match cas.read_raw(&s_oid) {
            Ok(r) => r,
            Err(e) => {
                return SyncExecutionResult::Aborted(format!("Failed to read source commit: {}", e))
            }
        };
        let s_commit = match Commit::deserialize(&s_raw.data) {
            Ok(c) => c,
            Err(e) => {
                return SyncExecutionResult::Aborted(format!(
                    "Failed to deserialize source commit: {}",
                    e
                ))
            }
        };

        // Check if fast-forward is possible (only when watched.paths is empty)
        let can_fast_forward = watched.paths.is_empty()
            && match target_head {
                None => true,
                Some(t_oid) => is_ancestor(&graph, &t_oid, &s_oid).unwrap_or(false),
            };

        if can_fast_forward {
            let _ = self.wal_engine.append(
                tx_id,
                WalStep::TreePrepared {
                    base_commit: target_head.map(|o| o.to_hex()),
                    result_tree_oid: s_commit.tree.to_hex(),
                },
            );

            let _ = self.wal_engine.append(
                tx_id,
                WalStep::CommitCreated {
                    commit_oid: s_oid.to_hex(),
                },
            );

            if let Err(e) = target_repo.set_head(&s_oid.to_hex()) {
                return SyncExecutionResult::Aborted(format!(
                    "Failed to update target head: {}",
                    e
                ));
            }
            if watched.target == "mainline" {
                let _ = self.repo.refs().write_ref(
                    "refs/heads/main",
                    &ReferenceTarget::Direct(s_oid),
                    None,
                    None,
                );
            }

            let _ = self.wal_engine.append(
                tx_id,
                WalStep::HeadUpdated {
                    target_dim: watched.target.clone(),
                    old_oid: target_head.map(|o| o.to_hex()).unwrap_or_default(),
                    new_oid: s_oid.to_hex(),
                },
            );

            let mut idx = target_repo.index().unwrap_or_default();
            if let Err(e) = checkout_tree(cas, &s_commit.tree, target_repo.workdir(), &mut idx) {
                return SyncExecutionResult::Aborted(format!("Checkout failed: {}", e));
            }
            let _ = target_repo.write_index(&idx);

            let files_count = idx.entries().len();
            let _ = self.wal_engine.append(
                tx_id,
                WalStep::WorkspaceUpdated {
                    files_updated: files_count,
                },
            );

            let _ = self.wal_engine.append(
                tx_id,
                WalStep::TxCommit {
                    completion_time: Utc::now(),
                },
            );

            let entry = SyncLogEntry {
                timestamp: Utc::now(),
                level: "INFO".to_string(),
                tx_id: tx_id.to_string(),
                source_dim: watched.dimension.clone(),
                target_dim: watched.target.clone(),
                strategy: format!("{:?}", strategy).to_lowercase(),
                action: "fast_forward".to_string(),
                conflicts: vec![],
                message: format!(
                    "Fast-forward synchronized {} into {}",
                    watched.dimension, watched.target
                ),
            };
            let _ = ConflictLogger::append(&self.cronos_dir, &entry);

            return SyncExecutionResult::Success {
                new_target_head: s_oid,
                files_updated: files_count,
                auto_resolved: 0,
            };
        }

        // Divergent branches or filtered paths: evaluate strategy
        let t_oid = match target_head {
            Some(oid) => oid,
            None => s_oid,
        };

        if strategy == SyncStrategy::Rebase && target_head.is_some() {
            let lca = merge_base(&graph, &t_oid, &s_oid).ok().flatten();
            let splice_engine = daft_convergence::SpliceEngine::new(Arc::clone(&self.repo));
            let lca_hex = lca.unwrap_or(t_oid).to_hex();
            let range = format!("{}..{}", lca_hex, s_oid.to_hex());
            match splice_engine.splice(&watched.dimension, &watched.target, &range, None) {
                Ok(outcome) => {
                    let new_head = target_repo.head_commit().unwrap_or(s_oid);
                    let _ = self.wal_engine.append(
                        tx_id,
                        WalStep::TxCommit {
                            completion_time: Utc::now(),
                        },
                    );
                    let entry = SyncLogEntry {
                        timestamp: Utc::now(),
                        level: "INFO".to_string(),
                        tx_id: tx_id.to_string(),
                        source_dim: watched.dimension.clone(),
                        target_dim: watched.target.clone(),
                        strategy: "rebase".to_string(),
                        action: "rebase".to_string(),
                        conflicts: vec![],
                        message: format!("Rebased {} onto {}", watched.dimension, watched.target),
                    };
                    let _ = ConflictLogger::append(&self.cronos_dir, &entry);
                    return SyncExecutionResult::Success {
                        new_target_head: new_head,
                        files_updated: outcome.spliced_commits.len(),
                        auto_resolved: 0,
                    };
                }
                Err(e) => {
                    let rec = ConflictRecord {
                        path: format!("rebase {}..{}", watched.dimension, watched.target),
                        reason: format!("Rebase conflict: {}", e),
                        base_hash: lca.map(|o| o.to_hex()),
                        source_hash: s_oid.to_hex(),
                        target_hash: t_oid.to_hex(),
                    };
                    return SyncExecutionResult::Conflict(vec![rec]);
                }
            }
        }

        // Read target commit
        let t_raw = match cas.read_raw(&t_oid) {
            Ok(r) => r,
            Err(e) => {
                return SyncExecutionResult::Aborted(format!("Failed to read target commit: {}", e))
            }
        };
        let t_commit = match Commit::deserialize(&t_raw.data) {
            Ok(c) => c,
            Err(e) => {
                return SyncExecutionResult::Aborted(format!(
                    "Failed to deserialize target commit: {}",
                    e
                ))
            }
        };

        let lca = merge_base(&graph, &t_oid, &s_oid).ok().flatten();
        let base_tree_oid = lca
            .and_then(|lca_oid| cas.read_raw(&lca_oid).ok())
            .and_then(|raw| Commit::deserialize(&raw.data).ok())
            .map(|c| c.tree);

        let merge_res = match merge_trees_3way(
            cas,
            base_tree_oid.as_ref(),
            &t_commit.tree,
            &s_commit.tree,
            &watched.target,
            &watched.dimension,
        ) {
            Ok(res) => res,
            Err(e) => {
                return SyncExecutionResult::Aborted(format!("3-way tree merge failed: {}", e))
            }
        };

        let mut auto_resolved_count = 0;
        let mut clean_files = merge_res.clean_files.clone();

        if merge_res.has_conflicts() {
            match strategy {
                SyncStrategy::Theirs => {
                    let mut conflict_records = Vec::new();
                    for (path, c) in &merge_res.conflicted_files {
                        conflict_records.push(ConflictRecord {
                            path: path.clone(),
                            reason: "Auto-resolved using 'theirs' strategy".to_string(),
                            base_hash: c.base.map(|o| o.to_hex()),
                            source_hash: c.theirs.map(|o| o.to_hex()).unwrap_or_default(),
                            target_hash: c.ours.map(|o| o.to_hex()).unwrap_or_default(),
                        });
                        if let Some(theirs_oid) = c.theirs {
                            clean_files.insert(
                                path.clone(),
                                (daft_core::object::FileMode::REGULAR, theirs_oid),
                            );
                        }
                    }
                    auto_resolved_count = conflict_records.len();
                    let entry = SyncLogEntry {
                        timestamp: Utc::now(),
                        level: "INFO".to_string(),
                        tx_id: tx_id.to_string(),
                        source_dim: watched.dimension.clone(),
                        target_dim: watched.target.clone(),
                        strategy: "theirs".to_string(),
                        action: "auto_resolved".to_string(),
                        conflicts: conflict_records,
                        message: format!(
                            "Auto-resolved {} conflicts using 'theirs' strategy",
                            auto_resolved_count
                        ),
                    };
                    let _ = ConflictLogger::append(&self.cronos_dir, &entry);
                }
                SyncStrategy::Ours => {
                    let mut conflict_records = Vec::new();
                    for (path, c) in &merge_res.conflicted_files {
                        conflict_records.push(ConflictRecord {
                            path: path.clone(),
                            reason: "Auto-resolved using 'ours' strategy".to_string(),
                            base_hash: c.base.map(|o| o.to_hex()),
                            source_hash: c.theirs.map(|o| o.to_hex()).unwrap_or_default(),
                            target_hash: c.ours.map(|o| o.to_hex()).unwrap_or_default(),
                        });
                        if let Some(ours_oid) = c.ours {
                            clean_files.insert(
                                path.clone(),
                                (daft_core::object::FileMode::REGULAR, ours_oid),
                            );
                        }
                    }
                    auto_resolved_count = conflict_records.len();
                    let entry = SyncLogEntry {
                        timestamp: Utc::now(),
                        level: "INFO".to_string(),
                        tx_id: tx_id.to_string(),
                        source_dim: watched.dimension.clone(),
                        target_dim: watched.target.clone(),
                        strategy: "ours".to_string(),
                        action: "auto_resolved".to_string(),
                        conflicts: conflict_records,
                        message: format!(
                            "Auto-resolved {} conflicts using 'ours' strategy",
                            auto_resolved_count
                        ),
                    };
                    let _ = ConflictLogger::append(&self.cronos_dir, &entry);
                }
                _ => {
                    let mut conflict_records = Vec::new();
                    for (path, c) in &merge_res.conflicted_files {
                        conflict_records.push(ConflictRecord {
                            path: path.clone(),
                            reason: "3-way merge conflict".to_string(),
                            base_hash: c.base.map(|o| o.to_hex()),
                            source_hash: c.theirs.map(|o| o.to_hex()).unwrap_or_default(),
                            target_hash: c.ours.map(|o| o.to_hex()).unwrap_or_default(),
                        });
                    }
                    return SyncExecutionResult::Conflict(conflict_records);
                }
            }
        }

        // Apply path filtering if watched.paths is specified
        let mut final_files = std::collections::BTreeMap::new();
        let mut target_orig_files = std::collections::BTreeMap::new();
        let _ =
            daft_core::diff::tree::flatten_tree(cas, &t_commit.tree, "", &mut target_orig_files);

        for (path, (mode, oid)) in &clean_files {
            if Self::path_matches_watched(&watched.paths, path) {
                final_files.insert(path.clone(), (*mode, *oid));
            } else if let Some(orig) = target_orig_files.get(path) {
                final_files.insert(path.clone(), *orig);
            }
        }
        for (path, orig) in &target_orig_files {
            if !Self::path_matches_watched(&watched.paths, path) {
                final_files.insert(path.clone(), *orig);
            }
        }

        let new_tree_oid = match build_hierarchical_tree(cas, &final_files) {
            Ok(oid) => oid,
            Err(e) => {
                return SyncExecutionResult::Aborted(format!("Failed to build merged tree: {}", e))
            }
        };

        let _ = self.wal_engine.append(
            tx_id,
            WalStep::TreePrepared {
                base_commit: lca.map(|o| o.to_hex()),
                result_tree_oid: new_tree_oid.to_hex(),
            },
        );

        let sig = get_signature();
        let commit = Commit::new(
            new_tree_oid,
            vec![t_oid, s_oid],
            sig.clone(),
            sig,
            format!(
                "Cronos sync: merge {} into {}",
                watched.dimension, watched.target
            ),
        );
        let raw_commit = RawObject::new(ObjectType::Commit, commit.serialize());
        let commit_oid = match self.repo.cas().write_raw(&raw_commit) {
            Ok(oid) => oid,
            Err(e) => {
                return SyncExecutionResult::Aborted(format!("Failed to write commit: {}", e))
            }
        };

        let _ = self.wal_engine.append(
            tx_id,
            WalStep::CommitCreated {
                commit_oid: commit_oid.to_hex(),
            },
        );

        if let Err(e) = target_repo.set_head(&commit_oid.to_hex()) {
            return SyncExecutionResult::Aborted(format!("Failed to set target head: {}", e));
        }
        if watched.target == "mainline" {
            let _ = self.repo.refs().write_ref(
                "refs/heads/main",
                &ReferenceTarget::Direct(commit_oid),
                None,
                None,
            );
        }

        let _ = self.wal_engine.append(
            tx_id,
            WalStep::HeadUpdated {
                target_dim: watched.target.clone(),
                old_oid: t_oid.to_hex(),
                new_oid: commit_oid.to_hex(),
            },
        );

        let mut idx = target_repo.index().unwrap_or_default();
        if let Err(e) = checkout_tree(cas, &new_tree_oid, target_repo.workdir(), &mut idx) {
            return SyncExecutionResult::Aborted(format!("Checkout failed: {}", e));
        }
        let _ = target_repo.write_index(&idx);

        let _ = self.wal_engine.append(
            tx_id,
            WalStep::WorkspaceUpdated {
                files_updated: final_files.len(),
            },
        );

        let _ = self.wal_engine.append(
            tx_id,
            WalStep::TxCommit {
                completion_time: Utc::now(),
            },
        );

        let entry = SyncLogEntry {
            timestamp: Utc::now(),
            level: "INFO".to_string(),
            tx_id: tx_id.to_string(),
            source_dim: watched.dimension.clone(),
            target_dim: watched.target.clone(),
            strategy: format!("{:?}", strategy).to_lowercase(),
            action: if auto_resolved_count > 0 {
                "auto_resolved".to_string()
            } else {
                "merge".to_string()
            },
            conflicts: vec![],
            message: format!("Merged {} into {}", watched.dimension, watched.target),
        };
        let _ = ConflictLogger::append(&self.cronos_dir, &entry);

        SyncExecutionResult::Success {
            new_target_head: commit_oid,
            files_updated: final_files.len(),
            auto_resolved: auto_resolved_count,
        }
    }

    pub fn run_tick(&self) -> Result<TickResult, SyncError> {
        let mut cfg = self.get_config()?;
        let mut entanglements_propagated = 0;
        let mut conflicts_detected = 0;

        if cfg.auto_entangle {
            let entangle_engine = EntangleEngine::new(Arc::clone(&self.repo));
            if let Ok(res) = entangle_engine.propagate_all() {
                for r in res {
                    entanglements_propagated += r.files_propagated.len();
                    conflicts_detected += r.conflicts.len();
                }
            }
        }

        let mut synced_dimensions = 0;
        let mut cfg_changed = false;

        for watched in &mut cfg.watched_dimensions {
            if watched.paused {
                continue;
            }

            let strategy = watched.strategy.unwrap_or(cfg.default_strategy);
            let tx_id = format!("tx-{}-{}", Utc::now().timestamp_millis(), watched.dimension);

            let source_repo = match DimensionRepository::for_dimension(
                Arc::clone(&self.repo),
                &watched.dimension,
            ) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let target_repo =
                match DimensionRepository::for_dimension(Arc::clone(&self.repo), &watched.target) {
                    Ok(r) => r,
                    Err(_) => continue,
                };

            let source_head = source_repo.head_commit();
            let target_head = target_repo.head_commit();

            let s_oid = match source_head {
                Some(oid) => oid,
                None => continue,
            };

            if target_head == Some(s_oid) {
                continue; // Already identical HEAD
            }

            let _ = self.wal_engine.append(
                &tx_id,
                WalStep::TxBegin {
                    source_dim: watched.dimension.clone(),
                    target_dim: watched.target.clone(),
                    strategy,
                    source_head: s_oid.to_hex(),
                    target_head: target_head.map(|o| o.to_hex()).unwrap_or_default(),
                },
            );

            let dim_mgr = DimensionManager::new(Arc::clone(&self.repo));
            let lock_dims = vec![watched.dimension.clone(), watched.target.clone()];
            let _lock_guard =
                match daft_convergence::MultiDimensionLockGuard::acquire(&dim_mgr, lock_dims) {
                    Ok(g) => g,
                    Err(e) => {
                        let _ = self.wal_engine.append(
                            &tx_id,
                            WalStep::TxAbort {
                                reason: format!("Lock acquisition failed: {}", e),
                            },
                        );
                        continue;
                    }
                };

            let _ = self.wal_engine.append(
                &tx_id,
                WalStep::LocksAcquired {
                    dimensions: vec![watched.dimension.clone(), watched.target.clone()],
                },
            );

            match self.sync_dimension_pair(
                &tx_id,
                watched,
                strategy,
                &source_repo,
                &target_repo,
                s_oid,
                target_head,
            ) {
                SyncExecutionResult::Success {
                    new_target_head,
                    files_updated: _,
                    auto_resolved,
                } => {
                    synced_dimensions += 1;
                    conflicts_detected += auto_resolved;
                    watched.last_synced_at = Some(Utc::now());
                    watched.last_synced_commit = Some(new_target_head.to_hex());
                    cfg_changed = true;
                }
                SyncExecutionResult::Conflict(conflict_records) => {
                    conflicts_detected += conflict_records.len();
                    let entry = SyncLogEntry {
                        timestamp: Utc::now(),
                        level: "WARN".to_string(),
                        tx_id: tx_id.clone(),
                        source_dim: watched.dimension.clone(),
                        target_dim: watched.target.clone(),
                        strategy: format!("{:?}", strategy).to_lowercase(),
                        action: "conflict".to_string(),
                        conflicts: conflict_records,
                        message: format!(
                            "Sync conflict between {} and {}",
                            watched.dimension, watched.target
                        ),
                    };
                    let _ = ConflictLogger::append(&self.cronos_dir, &entry);
                    let _ = self.wal_engine.append(
                        &tx_id,
                        WalStep::TxAbort {
                            reason: "Merge conflicts detected".to_string(),
                        },
                    );
                }
                SyncExecutionResult::Aborted(reason) => {
                    let _ = self.wal_engine.append(&tx_id, WalStep::TxAbort { reason });
                }
            }
        }

        if cfg_changed {
            let _ = ConfigStorage::save_config(&self.cronos_dir, &cfg);
        }

        Ok(TickResult {
            synced_dimensions,
            entanglements_propagated,
            conflicts_detected,
        })
    }
}
