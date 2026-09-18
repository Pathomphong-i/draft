//! Dimension Snapshots Engine (`SnapshotManager`).
//!
//! Provides point-in-time state capture of dimension workspaces:
//! - Freezes complete worktree state (including untracked and dirty files) into immutable CAS trees
//! - Stored under `.dft/snapshots/<dimension_id>/<snapshot_id>.json`
//! - Non-destructive: creates zero commits on the branch DAG and leaves working tree untouched
//! - Supports full restore to any prior snapshot checkpoint

use crate::dimension::DimensionError;
use chrono::{DateTime, Utc};
use daft_core::cas::{ObjectId, ObjectType, RawObject};
use daft_core::merge::{build_hierarchical_tree, checkout_tree};
use daft_core::object::FileMode;
use daft_core::Repository;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Immutable representation of a captured dimension snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    pub snapshot_id: String,
    pub name: String,
    pub dimension_id: String,
    pub tree_oid: ObjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_oid: Option<ObjectId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_ref: Option<String>,
    pub created_at: DateTime<Utc>,
    pub description: String,
    pub file_count: usize,
    pub total_bytes: u64,
}

impl Snapshot {
    pub fn to_list_item(&self) -> SnapshotListItem {
        SnapshotListItem {
            snapshot_id: self.snapshot_id.clone(),
            name: self.name.clone(),
            dimension_id: self.dimension_id.clone(),
            tree_oid: self.tree_oid.to_hex(),
            commit_oid: self.commit_oid.map(|o| o.to_hex()),
            created_at: self.created_at.to_rfc3339(),
            description: self.description.clone(),
            file_count: self.file_count,
        }
    }
}

/// Lightweight snapshot summary item for listing queries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnapshotListItem {
    pub snapshot_id: String,
    pub name: String,
    pub dimension_id: String,
    pub tree_oid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_oid: Option<String>,
    pub created_at: String,
    pub description: String,
    pub file_count: usize,
}

pub struct SnapshotManager;

impl SnapshotManager {
    /// Base snapshots directory.
    pub fn snapshots_dir(dft_dir: &Path) -> PathBuf {
        dft_dir.join("snapshots")
    }

    /// Snapshots directory for a specific dimension.
    pub fn dimension_snapshots_dir(dft_dir: &Path, dimension_id: &str) -> PathBuf {
        Self::snapshots_dir(dft_dir).join(dimension_id)
    }

    /// Creates an immutable snapshot of a dimension's live state.
    pub fn create_snapshot(
        repo: &Repository,
        dimension_id: &str,
        name: &str,
        description: Option<&str>,
    ) -> Result<Snapshot, DimensionError> {
        let dft_dir = repo.dft_dir();
        let snap_dir = Self::dimension_snapshots_dir(dft_dir, dimension_id);
        fs::create_dir_all(&snap_dir)?;

        // Ensure name uniqueness within dimension
        let existing = Self::list_snapshots(repo, Some(dimension_id))?;
        if existing.iter().any(|s| s.name == name) {
            return Err(DimensionError::General(format!(
                "Snapshot '{}' already exists in dimension '{}'",
                name, dimension_id
            )));
        }

        // Determine working directory for target dimension
        let current_dim = {
            let file = dft_dir.join("current_dimension");
            if let Ok(c) = fs::read_to_string(file) {
                let trimmed = c.trim();
                if !trimmed.is_empty() {
                    trimmed.to_string()
                } else {
                    "mainline".to_string()
                }
            } else {
                "mainline".to_string()
            }
        };

        let worktree_dir = if current_dim == dimension_id || dimension_id == "mainline" {
            repo.workdir()
                .ok_or(DimensionError::BareRepository)?
                .to_path_buf()
        } else {
            dft_dir
                .join("dimensions")
                .join(dimension_id)
                .join("workspace")
        };

        if !worktree_dir.exists() {
            return Err(DimensionError::NotFound(format!(
                "Dimension workspace for '{}' does not exist",
                dimension_id
            )));
        }

        // Scan working directory files and write blobs to CAS
        let mut tree_files: BTreeMap<String, (FileMode, ObjectId)> = BTreeMap::new();
        let mut total_bytes = 0u64;
        let mut file_count = 0usize;

        for entry in WalkDir::new(&worktree_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.starts_with(dft_dir) {
                continue;
            }
            if entry.file_type().is_file() {
                if let Ok(rel) = path.strip_prefix(&worktree_dir) {
                    let rel_str = rel.to_string_lossy().to_string();
                    let data = fs::read(path)?;
                    total_bytes += data.len() as u64;
                    file_count += 1;

                    let raw = RawObject::new(ObjectType::Blob, data);
                    let blob_oid = repo.cas().write_raw(&raw)?;

                    #[cfg(unix)]
                    let mode = {
                        use std::os::unix::fs::PermissionsExt;
                        let meta = entry
                            .metadata()
                            .map_err(std::io::Error::other)?;
                        if meta.permissions().mode() & 0o111 != 0 {
                            FileMode(0o100755)
                        } else {
                            FileMode(0o100644)
                        }
                    };
                    #[cfg(not(unix))]
                    let mode = FileMode(0o100644);

                    tree_files.insert(rel_str, (mode, blob_oid));
                }
            }
        }

        // Construct hierarchical CAS tree
        let tree_oid = build_hierarchical_tree(repo.cas().as_ref(), &tree_files)?;

        // Capture HEAD reference & commit OID
        let dim_dir = dft_dir.join("dimensions").join(dimension_id);
        let head_content = if current_dim == dimension_id || dimension_id == "mainline" {
            fs::read_to_string(dft_dir.join("HEAD")).unwrap_or_default()
        } else {
            fs::read_to_string(dim_dir.join("HEAD")).unwrap_or_default()
        };

        let trimmed_head = head_content.trim();
        let head_ref = trimmed_head.strip_prefix("ref: ").map(|sym| sym.to_string());

        let commit_oid = if let Some(ref sym) = head_ref {
            repo.refs().resolve(sym).ok()
        } else {
            ObjectId::from_hex(trimmed_head).ok()
        };

        let now = Utc::now();
        let snapshot_id = format!(
            "snap_{}_{}",
            now.timestamp_millis(),
            &tree_oid.to_hex()[..8]
        );
        let desc = description
            .unwrap_or("Point-in-time dimension checkpoint")
            .to_string();

        let snapshot = Snapshot {
            snapshot_id: snapshot_id.clone(),
            name: name.to_string(),
            dimension_id: dimension_id.to_string(),
            tree_oid,
            commit_oid,
            head_ref,
            created_at: now,
            description: desc,
            file_count,
            total_bytes,
        };

        // Atomic write
        let tmp_file = snap_dir.join(format!(".tmp_{}.json", snapshot_id));
        let final_file = snap_dir.join(format!("{}.json", snapshot_id));
        let serialized = serde_json::to_string_pretty(&snapshot)?;

        fs::write(&tmp_file, serialized)?;
        fs::rename(tmp_file, final_file)?;

        Ok(snapshot)
    }

    /// Lists snapshots, optionally filtered by dimension.
    pub fn list_snapshots(
        repo: &Repository,
        dimension_filter: Option<&str>,
    ) -> Result<Vec<Snapshot>, DimensionError> {
        let base_dir = Self::snapshots_dir(repo.dft_dir());
        if !base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut snapshots = Vec::new();
        let target_dirs: Vec<PathBuf> = if let Some(dim) = dimension_filter {
            vec![base_dir.join(dim)]
        } else {
            fs::read_dir(&base_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .map(|e| e.path())
                .collect()
        };

        for dir in target_dirs {
            if !dir.exists() {
                continue;
            }
            for entry in fs::read_dir(dir)?.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(snap) = serde_json::from_str::<Snapshot>(&content) {
                            snapshots.push(snap);
                        }
                    }
                }
            }
        }

        snapshots.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        Ok(snapshots)
    }

    /// Restores a dimension workspace to a captured snapshot state.
    pub fn restore_snapshot(
        repo: &Repository,
        dimension_id: &str,
        name_or_id: &str,
        force: bool,
    ) -> Result<Snapshot, DimensionError> {
        let snapshots = Self::list_snapshots(repo, Some(dimension_id))?;
        let snapshot = snapshots
            .into_iter()
            .find(|s| s.name == name_or_id || s.snapshot_id == name_or_id)
            .ok_or_else(|| {
                DimensionError::NotFound(format!(
                    "Snapshot '{}' not found in dimension '{}'",
                    name_or_id, dimension_id
                ))
            })?;

        let dft_dir = repo.dft_dir();
        let current_dim = {
            let file = dft_dir.join("current_dimension");
            if let Ok(c) = fs::read_to_string(file) {
                let trimmed = c.trim();
                if !trimmed.is_empty() {
                    trimmed.to_string()
                } else {
                    "mainline".to_string()
                }
            } else {
                "mainline".to_string()
            }
        };

        let worktree_dir = if current_dim == dimension_id || dimension_id == "mainline" {
            repo.workdir()
                .ok_or(DimensionError::BareRepository)?
                .to_path_buf()
        } else {
            dft_dir
                .join("dimensions")
                .join(dimension_id)
                .join("workspace")
        };

        // Safety check: verify clean working tree if active and not forced
        if current_dim == dimension_id && !force {
            let status = daft_core::worktree::status::get_status(repo)?;
            let is_dirty = !status.staged_added.is_empty()
                || !status.staged_modified.is_empty()
                || !status.staged_deleted.is_empty()
                || !status.unstaged_modified.is_empty()
                || !status.unstaged_deleted.is_empty();
            if is_dirty {
                return Err(DimensionError::General(
                    "Working tree contains uncommitted changes. Use --force to overwrite."
                        .to_string(),
                ));
            }
        }

        // Checkout snapshot tree into worktree
        let mut index = repo.index()?;
        checkout_tree(
            repo.cas().as_ref(),
            &snapshot.tree_oid,
            &worktree_dir,
            &mut index,
        )?;

        let index_path = if current_dim == dimension_id || dimension_id == "mainline" {
            repo.index_path()
        } else {
            dft_dir.join("dimensions").join(dimension_id).join("index")
        };
        index.write_to(&index_path)?;

        Ok(snapshot)
    }

    /// Deletes a snapshot record.
    pub fn delete_snapshot(
        repo: &Repository,
        dimension_id: &str,
        name_or_id: &str,
    ) -> Result<Snapshot, DimensionError> {
        let snapshots = Self::list_snapshots(repo, Some(dimension_id))?;
        let snapshot = snapshots
            .into_iter()
            .find(|s| s.name == name_or_id || s.snapshot_id == name_or_id)
            .ok_or_else(|| {
                DimensionError::NotFound(format!(
                    "Snapshot '{}' not found in dimension '{}'",
                    name_or_id, dimension_id
                ))
            })?;

        let file_path = Self::dimension_snapshots_dir(repo.dft_dir(), dimension_id)
            .join(format!("{}.json", snapshot.snapshot_id));

        if file_path.exists() {
            fs::remove_file(file_path)?;
        }

        Ok(snapshot)
    }
}
