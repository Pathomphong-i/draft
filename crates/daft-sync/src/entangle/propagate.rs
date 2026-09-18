use crate::cronos::conflict::{ConflictLogger, ConflictRecord, SyncLogEntry};
use crate::entangle::echo::{compute_hash, EchoCancellation};
use crate::entangle::log::{AuditLogger, EntangleEvent};
use crate::entangle::rule::{EntangleDirection, EntangleRule};
use crate::error::SyncError;
use chrono::Utc;
use daft_awareness::matches_glob;
use daft_core::index::{IndexEntry, IndexTime};
use daft_core::merge::merge_text_3way;
use daft_core::Repository;
use daft_dimension::DimensionRepository;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropagationResult {
    pub rule_id: String,
    pub source_dim: String,
    pub target_dim: String,
    pub files_propagated: Vec<String>,
    pub files_skipped: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    pub errors: Vec<String>,
}

pub struct Propagator {
    repo: Arc<Repository>,
    entangle_dir: PathBuf,
}

impl Propagator {
    pub fn new(repo: Arc<Repository>) -> Self {
        let entangle_dir = repo.dft_dir().join("entangle");
        Self { repo, entangle_dir }
    }

    fn resolve_workdir(&self, dim: &str) -> Result<PathBuf, SyncError> {
        if dim == "mainline" {
            self.repo
                .workdir()
                .map(|p| p.to_path_buf())
                .ok_or_else(|| SyncError::General("Mainline workdir not configured".into()))
        } else {
            let dim_dir = self.repo.dft_dir().join("dimensions").join(dim);
            if !dim_dir.exists() {
                return Err(SyncError::DimensionNotFound(dim.to_string()));
            }
            Ok(dim_dir.join("workspace"))
        }
    }

    fn matches_rule_paths(rule: &EntangleRule, rel_path: &str) -> bool {
        let patterns = match &rule.paths {
            Some(p) => p,
            None => return true,
        };

        for pat in patterns.split(',') {
            let trimmed = pat.trim();
            if trimmed.is_empty() {
                continue;
            }
            if matches_glob(trimmed, rel_path) {
                return true;
            }
        }
        false
    }

    pub fn propagate_between(
        &self,
        rule: &EntangleRule,
        source_dim: &str,
        target_dim: &str,
        session_id: &str,
    ) -> Result<PropagationResult, SyncError> {
        let source_workdir = self.resolve_workdir(source_dim)?;
        let target_workdir = self.resolve_workdir(target_dim)?;

        let mut result = PropagationResult {
            rule_id: rule.id.clone(),
            source_dim: source_dim.to_string(),
            target_dim: target_dim.to_string(),
            files_propagated: Vec::new(),
            files_skipped: Vec::new(),
            conflicts: Vec::new(),
            errors: Vec::new(),
        };

        if !source_workdir.exists() && !target_workdir.exists() {
            return Ok(result);
        }

        let echo = EchoCancellation::new(&self.entangle_dir);
        let cronos_dir = self.repo.dft_dir().join("cronos");

        // Optional target repo for index sync
        let target_dim_repo =
            DimensionRepository::for_dimension(Arc::clone(&self.repo), target_dim).ok();
        let mut target_index = target_dim_repo
            .as_ref()
            .and_then(|r| r.index().ok())
            .unwrap_or_default();
        let mut target_index_modified = false;

        let source_dim_repo =
            DimensionRepository::for_dimension(Arc::clone(&self.repo), source_dim).ok();
        let mut source_index = source_dim_repo
            .as_ref()
            .and_then(|r| r.index().ok())
            .unwrap_or_default();
        let mut source_index_modified = false;

        let mut visited_source_paths = std::collections::HashSet::new();

        // --------------------------------------------------------------------
        // Phase 1: Forward File Propagation (Creations & Updates in Source)
        // --------------------------------------------------------------------
        if source_workdir.exists() {
            for entry in WalkDir::new(&source_workdir)
                .into_iter()
                .filter_entry(|e| {
                    let fname = e.file_name().to_string_lossy();
                    !fname.starts_with(".dft") && !fname.starts_with(".git")
                })
                .flatten()
            {
                if !entry.file_type().is_file() {
                    continue;
                }

                let rel_path = match entry.path().strip_prefix(&source_workdir) {
                    Ok(p) => p.to_string_lossy().to_string(),
                    Err(_) => continue,
                };

                if !Self::matches_rule_paths(rule, &rel_path) {
                    continue;
                }

                visited_source_paths.insert(rel_path.clone());

                let source_bytes = match fs::read(entry.path()) {
                    Ok(b) => b,
                    Err(e) => {
                        result
                            .errors
                            .push(format!("Failed to read {}: {}", rel_path, e));
                        continue;
                    }
                };

                let source_hash = compute_hash(&source_bytes);
                let target_file_path = target_workdir.join(&rel_path);

                let (target_exists, target_bytes, target_hash) = if target_file_path.exists() {
                    let bytes = fs::read(&target_file_path).unwrap_or_default();
                    let h = compute_hash(&bytes);
                    (true, bytes, Some(h))
                } else {
                    (false, Vec::new(), None)
                };

                // Tier 1: Check Echo Loop
                if echo.is_echo(
                    &rel_path,
                    &source_hash,
                    target_hash.as_deref(),
                    source_dim,
                    target_dim,
                ) {
                    result.files_skipped.push(rel_path);
                    continue;
                }

                // Check if target was previously synced and has now been deleted in target
                if !target_exists && rule.direction == EntangleDirection::Bidirectional {
                    if let Some(last_synced_h) = echo.get_last_synced_hash(target_dim, &rel_path) {
                        if source_hash == last_synced_h {
                            // Target deleted the file and source has not modified it!
                            // Propagate deletion to source workspace to prevent resurrection:
                            let _ = fs::remove_file(entry.path());
                            source_index.remove_path(&rel_path);
                            source_index_modified = true;
                            let _ =
                                echo.record_deletion(&rel_path, source_dim, target_dim, session_id);

                            let event = EntangleEvent {
                                event_id: format!(
                                    "evt-{}-del-{}",
                                    Utc::now().timestamp_millis(),
                                    result.files_propagated.len()
                                ),
                                timestamp: Utc::now(),
                                rule_id: rule.id.clone(),
                                source_dim: target_dim.to_string(),
                                target_dim: source_dim.to_string(),
                                path: rel_path.clone(),
                                source_hash: String::new(),
                                target_previous_hash: source_hash,
                                action: "deleted".to_string(),
                                session_id: session_id.to_string(),
                                status: "success".to_string(),
                            };
                            let _ = AuditLogger::append(&self.entangle_dir, &event);
                            result
                                .files_propagated
                                .push(format!("deleted: {}", rel_path));
                            continue;
                        }
                    }
                }

                // Conflict Detection: Target exists and differs from source
                if let Some(target_h) = target_hash.clone() {
                    let last_synced_h = echo.get_last_synced_hash(target_dim, &rel_path);
                    let has_uncommitted_target_edits = match last_synced_h.as_deref() {
                        Some(lsh) => target_h != lsh,
                        None => true, // Target exists without prior sync provenance
                    };

                    if has_uncommitted_target_edits {
                        // CONFLICT DETECTED!
                        // 1. Audit log in .dft/entangle/log.jsonl
                        let event = EntangleEvent {
                            event_id: format!(
                                "evt-{}-{}",
                                Utc::now().timestamp_millis(),
                                result.conflicts.len()
                            ),
                            timestamp: Utc::now(),
                            rule_id: rule.id.clone(),
                            source_dim: source_dim.to_string(),
                            target_dim: target_dim.to_string(),
                            path: rel_path.clone(),
                            source_hash: source_hash.clone(),
                            target_previous_hash: target_h.clone(),
                            action: "conflict".to_string(),
                            session_id: session_id.to_string(),
                            status: "conflict_detected".to_string(),
                        };
                        let _ = AuditLogger::append(&self.entangle_dir, &event);

                        // 2. Conflict log in .dft/cronos/sync.log
                        let sync_entry = SyncLogEntry {
                            timestamp: Utc::now(),
                            level: "WARN".to_string(),
                            tx_id: session_id.to_string(),
                            source_dim: source_dim.to_string(),
                            target_dim: target_dim.to_string(),
                            strategy: "entangle".to_string(),
                            action: "conflict_detected".to_string(),
                            conflicts: vec![ConflictRecord {
                                path: rel_path.clone(),
                                reason:
                                    "Concurrent uncommitted modifications in entangled dimension"
                                        .to_string(),
                                base_hash: last_synced_h.clone(),
                                source_hash: source_hash.clone(),
                                target_hash: target_h.clone(),
                            }],
                            message: format!(
                                "Conflict detected on {} between {} and {}",
                                rel_path, source_dim, target_dim
                            ),
                        };
                        let _ = ConflictLogger::append(&cronos_dir, &sync_entry);

                        // 3. Safe Content Synthesis: 3-way merge markers + sidecar backup
                        if let (Ok(t_text), Ok(s_text)) = (
                            std::str::from_utf8(&target_bytes),
                            std::str::from_utf8(&source_bytes),
                        ) {
                            let merge_res =
                                merge_text_3way("", t_text, s_text, target_dim, source_dim);
                            let _ = fs::write(&target_file_path, merge_res.content.as_bytes());
                        }
                        let backup_path =
                            target_workdir.join(format!("{}.conflict.{}", rel_path, source_dim));
                        let _ = fs::write(&backup_path, &source_bytes);

                        result.conflicts.push(rel_path.clone());
                        result.files_skipped.push(rel_path);
                        continue;
                    }
                }

                // Clean propagation
                if let Some(parent) = target_file_path.parent() {
                    if let Err(e) = fs::create_dir_all(parent) {
                        result.errors.push(format!(
                            "Failed to create dir for {}: {}",
                            target_file_path.display(),
                            e
                        ));
                        continue;
                    }
                }

                if let Err(e) = fs::write(&target_file_path, &source_bytes) {
                    result
                        .errors
                        .push(format!("Failed to write {}: {}", rel_path, e));
                    continue;
                }

                // Sync index if available
                if let Ok(blob_oid) = self.repo.cas().write_blob(&source_bytes) {
                    let mode = entry
                        .metadata()
                        .map(|m| {
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                m.permissions().mode()
                            }
                            #[cfg(not(unix))]
                            {
                                0o100644
                            }
                        })
                        .unwrap_or(0o100644);

                    target_index.add_entry(IndexEntry {
                        ctime: IndexTime::default(),
                        mtime: IndexTime::default(),
                        dev: 0,
                        ino: 0,
                        mode,
                        uid: 0,
                        gid: 0,
                        file_size: source_bytes.len() as u32,
                        oid: blob_oid,
                        flags: 0,
                        path: rel_path.clone(),
                    });
                    target_index_modified = true;
                }

                // Record provenance for BOTH dimensions
                let _ =
                    echo.record_sync(&rel_path, &source_hash, source_dim, target_dim, session_id);

                let event = EntangleEvent {
                    event_id: format!(
                        "evt-{}-{}",
                        Utc::now().timestamp_millis(),
                        result.files_propagated.len()
                    ),
                    timestamp: Utc::now(),
                    rule_id: rule.id.clone(),
                    source_dim: source_dim.to_string(),
                    target_dim: target_dim.to_string(),
                    path: rel_path.clone(),
                    source_hash,
                    target_previous_hash: target_hash.unwrap_or_default(),
                    action: "propagated".to_string(),
                    session_id: session_id.to_string(),
                    status: "success".to_string(),
                };
                let _ = AuditLogger::append(&self.entangle_dir, &event);

                result.files_propagated.push(rel_path);
            }
        }

        // --------------------------------------------------------------------
        // Phase 2: Deletion Reconciliation (Deleted in Source -> Remove in Target)
        // --------------------------------------------------------------------
        if target_workdir.exists() {
            for entry in WalkDir::new(&target_workdir)
                .into_iter()
                .filter_entry(|e| {
                    let fname = e.file_name().to_string_lossy();
                    !fname.starts_with(".dft") && !fname.starts_with(".git")
                })
                .flatten()
            {
                if !entry.file_type().is_file() {
                    continue;
                }

                let rel_path = match entry.path().strip_prefix(&target_workdir) {
                    Ok(p) => p.to_string_lossy().to_string(),
                    Err(_) => continue,
                };

                if !Self::matches_rule_paths(rule, &rel_path) {
                    continue;
                }

                // If file was already visited in source, it's present in source
                if visited_source_paths.contains(&rel_path) {
                    continue;
                }

                // Check if file previously existed and was synced
                if let Some(last_synced_h) = echo.get_last_synced_hash(target_dim, &rel_path) {
                    let target_bytes = fs::read(entry.path()).unwrap_or_default();
                    let target_h = compute_hash(&target_bytes);

                    if target_h == last_synced_h {
                        // Clean deletion in source! Propagate removal to target:
                        let _ = fs::remove_file(entry.path());
                        target_index.remove_path(&rel_path);
                        target_index_modified = true;
                        let _ = echo.record_deletion(&rel_path, source_dim, target_dim, session_id);

                        let event = EntangleEvent {
                            event_id: format!(
                                "evt-{}-del-{}",
                                Utc::now().timestamp_millis(),
                                result.files_propagated.len()
                            ),
                            timestamp: Utc::now(),
                            rule_id: rule.id.clone(),
                            source_dim: source_dim.to_string(),
                            target_dim: target_dim.to_string(),
                            path: rel_path.clone(),
                            source_hash: String::new(),
                            target_previous_hash: target_h,
                            action: "deleted".to_string(),
                            session_id: session_id.to_string(),
                            status: "success".to_string(),
                        };
                        let _ = AuditLogger::append(&self.entangle_dir, &event);
                        result
                            .files_propagated
                            .push(format!("deleted: {}", rel_path));
                    } else {
                        // Delete-Modify conflict: target modified, source deleted
                        let event = EntangleEvent {
                            event_id: format!(
                                "evt-{}-delconf-{}",
                                Utc::now().timestamp_millis(),
                                result.conflicts.len()
                            ),
                            timestamp: Utc::now(),
                            rule_id: rule.id.clone(),
                            source_dim: source_dim.to_string(),
                            target_dim: target_dim.to_string(),
                            path: rel_path.clone(),
                            source_hash: String::new(),
                            target_previous_hash: target_h.clone(),
                            action: "conflict".to_string(),
                            session_id: session_id.to_string(),
                            status: "delete_modify_conflict".to_string(),
                        };
                        let _ = AuditLogger::append(&self.entangle_dir, &event);
                        result.conflicts.push(rel_path);
                    }
                }
            }
        }

        if target_index_modified {
            if let Some(dim_repo) = &target_dim_repo {
                let _ = dim_repo.write_index(&target_index);
            }
        }

        if source_index_modified {
            if let Some(dim_repo) = &source_dim_repo {
                let _ = dim_repo.write_index(&source_index);
            }
        }

        Ok(result)
    }

    pub fn propagate_rule(
        &self,
        rule: &EntangleRule,
        session_id: &str,
    ) -> Result<Vec<PropagationResult>, SyncError> {
        let mut results = Vec::new();

        // 1. source -> target (dim1 -> dim2)
        results.push(self.propagate_between(rule, &rule.dim1, &rule.dim2, session_id)?);

        // 2. if bidirectional: target -> source (dim2 -> dim1)
        if rule.direction == EntangleDirection::Bidirectional {
            results.push(self.propagate_between(rule, &rule.dim2, &rule.dim1, session_id)?);
        }

        Ok(results)
    }
}
