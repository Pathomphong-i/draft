//! Dimension Lifecycle Manager (`DimensionManager`).
//! Coordinates create, list, enter/switch, destroy/delete, rename, and info operations.

use crate::cow::detect_best_strategy;
use crate::dimension::{DimensionError, DimensionListItem, DimensionMetadata};
use crate::lock::DimensionLockGuard;
use daft_core::cas::ObjectId;
use daft_core::merge::checkout_tree;
use daft_core::object::Commit;
use daft_core::refs::peel_reference;
use daft_core::worktree::resolve_commit;
use daft_core::Repository;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use walkdir::WalkDir;

pub struct DimensionManager {
    repo: Arc<Repository>,
    dimensions_dir: PathBuf,
}

impl DimensionManager {
    pub fn new(repo: Arc<Repository>) -> Self {
        let dimensions_dir = repo.dft_dir().join("dimensions");
        Self {
            repo,
            dimensions_dir,
        }
    }

    pub fn dimensions_dir(&self) -> &Path {
        &self.dimensions_dir
    }

    pub fn repo(&self) -> &Arc<Repository> {
        &self.repo
    }

    /// Initializes dimension subsystem directories and pointers if not present.
    pub fn init(&self) -> Result<(), DimensionError> {
        fs::create_dir_all(&self.dimensions_dir)?;
        let current_file = self.repo.dft_dir().join("current_dimension");
        if !current_file.exists() {
            fs::write(current_file, "mainline\n")?;
        }
        Ok(())
    }

    /// Returns the name of the currently active dimension (default: "mainline").
    pub fn current_dimension_name(&self) -> String {
        let file = self.repo.dft_dir().join("current_dimension");
        if let Ok(content) = fs::read_to_string(file) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        "mainline".to_string()
    }

    /// Sets the active dimension pointer in `.dft/current_dimension`.
    pub fn set_current_dimension(&self, name: &str) -> Result<(), DimensionError> {
        let file = self.repo.dft_dir().join("current_dimension");
        fs::write(file, format!("{}\n", name))?;
        Ok(())
    }

    /// Validates dimension name syntax (disallowing empty names, traversal, separators).
    pub fn validate_dimension_name(name: &str) -> Result<(), DimensionError> {
        if name.is_empty()
            || name.contains("..")
            || name.contains('/')
            || name.contains('\\')
            || name.contains('\0')
        {
            return Err(DimensionError::InvalidName(name.to_string()));
        }
        Ok(())
    }

    /// Creates a new parallel dimension workspace.
    pub fn create_dimension(
        &self,
        name: &str,
        from_ref: Option<&str>,
        creator: Option<&str>,
    ) -> Result<DimensionMetadata, DimensionError> {
        Self::validate_dimension_name(name)?;
        if name == "mainline" {
            return Err(DimensionError::AlreadyExists("mainline".to_string()));
        }

        let target_dir = self.dimensions_dir.join(name);
        if target_dir.exists() {
            return Err(DimensionError::AlreadyExists(name.to_string()));
        }

        fs::create_dir_all(&target_dir)?;
        let ws_dir = target_dir.join("workspace");
        fs::create_dir_all(&ws_dir)?;

        let dft_dir = self.repo.dft_dir();

        // Resolve base commit / HEAD
        let (head_content, base_commit_oid) = if let Some(from) = from_ref {
            let oid = resolve_commit(self.repo.as_ref(), from)?;
            (format!("{}\n", oid.to_hex()), Some(oid))
        } else if let Ok(h) = fs::read_to_string(dft_dir.join("HEAD")) {
            let trimmed = h.trim();
            let oid = if let Ok(oid) = ObjectId::from_hex(trimmed) {
                Some(oid)
            } else if let Some(sym) = trimmed.strip_prefix("ref: ") {
                self.repo.refs().resolve(sym).ok()
            } else {
                peel_reference(dft_dir, "HEAD").ok()
            };
            (h, oid)
        } else {
            ("ref: refs/heads/main\n".to_string(), None)
        };

        fs::write(target_dir.join("HEAD"), &head_content)?;

        let index_path = self.repo.index_path();
        if index_path.exists() {
            let _ = fs::copy(&index_path, target_dir.join("index"));
        } else {
            let index = daft_core::Index::new();
            let _ = index.write_to(&target_dir.join("index"));
        }

        let cow_strategy = detect_best_strategy(dft_dir);
        let now = chrono::Utc::now().to_rfc3339();
        let meta = DimensionMetadata {
            name: name.to_string(),
            creator: creator.unwrap_or("Daft Agent").to_string(),
            branch: name.to_string(),
            cow_mode: cow_strategy.to_string(),
            status: "clean".to_string(),
            created_at: now,
            parent: Some("mainline".to_string()),
            head_commit: base_commit_oid,
            description: None,
        };

        let json_pretty = serde_json::to_string_pretty(&meta)?;
        fs::write(target_dir.join("dimension.json"), &json_pretty)?;
        fs::write(target_dir.join("meta.json"), &json_pretty)?;

        Ok(meta)
    }

    /// Enumerates all dimensions (including mainline) and their statuses.
    pub fn list_dimensions(&self) -> Result<Vec<DimensionListItem>, DimensionError> {
        let current = self.current_dimension_name();
        let mut list = Vec::new();

        // 1. Always include mainline
        list.push(DimensionListItem {
            name: "mainline".to_string(),
            branch: "main".to_string(),
            active: current == "mainline",
            status: "clean".to_string(),
            parent: None,
            cow_mode: Some("mainline".to_string()),
            disk_usage_bytes: Some(0),
        });

        // 2. Enumerate dimensions under .dft/dimensions
        if let Ok(entries) = fs::read_dir(&self.dimensions_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name == "mainline" {
                        continue;
                    }

                    let active = current == name;
                    let meta = self.read_dimension_metadata(&name).ok();
                    let branch = meta
                        .as_ref()
                        .map(|m| m.branch.clone())
                        .unwrap_or_else(|| "main".to_string());
                    let parent = meta.as_ref().and_then(|m| m.parent.clone());
                    let cow_mode = meta.as_ref().map(|m| m.cow_mode.clone());

                    let status = if let Some(ref m) = meta {
                        m.status.clone()
                    } else if path.join("workspace").exists()
                        && fs::read_dir(path.join("workspace"))
                            .map(|mut r| r.next().is_some())
                            .unwrap_or(false)
                    {
                        "dirty".to_string()
                    } else {
                        "clean".to_string()
                    };

                    let ws = path.join("workspace");
                    let disk_usage = if ws.exists() {
                        calculate_disk_usage(&ws)
                    } else {
                        0
                    };

                    list.push(DimensionListItem {
                        name,
                        branch,
                        active,
                        status,
                        parent,
                        cow_mode,
                        disk_usage_bytes: Some(disk_usage),
                    });
                }
            }
        }

        list.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(list)
    }

    /// Switches the active dimension context (CLI `enter` / `switch`).
    pub fn switch_dimension(&self, name: &str) -> Result<(), DimensionError> {
        let dft_dir = self.repo.dft_dir();
        if name != "mainline" && !self.dimensions_dir.join(name).exists() {
            return Err(DimensionError::NotFound(name.to_string()));
        }

        let current = self.current_dimension_name();
        if current == name {
            return Ok(());
        }

        let workdir = self.repo.workdir().ok_or(DimensionError::BareRepository)?;

        // Acquire lock on current and target
        let _current_lock = if current != "mainline" && self.dimensions_dir.join(&current).exists()
        {
            Some(DimensionLockGuard::acquire_timeout(
                &self.dimensions_dir,
                &current,
                None,
                "switch_out",
                Duration::from_secs(5),
            )?)
        } else {
            None
        };

        let _target_lock = if name != "mainline" && self.dimensions_dir.join(name).exists() {
            Some(DimensionLockGuard::acquire_timeout(
                &self.dimensions_dir,
                name,
                None,
                "switch_in",
                Duration::from_secs(5),
            )?)
        } else {
            None
        };

        // 1. Save current worktree to old dimension workspace
        let old_ws = if current == "mainline" {
            self.dimensions_dir.join("mainline/workspace")
        } else {
            self.dimensions_dir.join(&current).join("workspace")
        };
        fs::create_dir_all(&old_ws)?;

        let mut current_files = Vec::new();
        for entry in WalkDir::new(workdir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.starts_with(dft_dir) {
                continue;
            }
            if entry.file_type().is_file() {
                if let Ok(rel) = path.strip_prefix(workdir) {
                    current_files.push(rel.to_path_buf());
                    let dest = old_ws.join(rel);
                    if let Some(p) = dest.parent() {
                        let _ = fs::create_dir_all(p);
                    }
                    let _ = fs::copy(path, dest);
                }
            }
        }

        // Save current index to old dimension
        let old_dim_dir = if current == "mainline" {
            self.dimensions_dir.join("mainline")
        } else {
            self.dimensions_dir.join(&current)
        };
        let _ = fs::create_dir_all(&old_dim_dir);
        let _ = fs::copy(self.repo.index_path(), old_dim_dir.join("index"));

        // 2. Remove old worktree files
        for rel in current_files {
            let p = workdir.join(rel);
            if p.exists() {
                let _ = fs::remove_file(&p);
            }
        }
        clean_empty_dirs(workdir, dft_dir)?;

        // 3. Restore target dimension
        let target_dim_dir = if name == "mainline" {
            self.dimensions_dir.join("mainline")
        } else {
            self.dimensions_dir.join(name)
        };
        let target_ws = target_dim_dir.join("workspace");

        if target_ws.exists() && fs::read_dir(&target_ws)?.next().is_some() {
            // Restore from workspace files
            copy_dir_contents(&target_ws, workdir)?;
            if target_dim_dir.join("index").exists() {
                let _ = fs::copy(target_dim_dir.join("index"), self.repo.index_path());
            }
        } else {
            // Restore from HEAD commit tree
            let head_oid_opt = if name == "mainline" {
                peel_reference(dft_dir, "HEAD").ok()
            } else if let Ok(h) = fs::read_to_string(target_dim_dir.join("HEAD")) {
                let trimmed = h.trim();
                if let Ok(oid) = ObjectId::from_hex(trimmed) {
                    Some(oid)
                } else if let Some(sym) = trimmed.strip_prefix("ref: ") {
                    self.repo.refs().resolve(sym).ok()
                } else {
                    None
                }
            } else {
                None
            };

            let mut index = daft_core::Index::new();
            if let Some(head_oid) = head_oid_opt {
                if let Ok(raw) = self.repo.cas().read_raw(&head_oid) {
                    if let Ok(commit) = Commit::deserialize(&raw.data) {
                        let _ = checkout_tree(
                            self.repo.cas().as_ref(),
                            &commit.tree,
                            workdir,
                            &mut index,
                        );
                    }
                }
            }
            let _ = index.write_to(&self.repo.index_path());
        }

        self.set_current_dimension(name)?;
        Ok(())
    }

    /// Deletes a dimension and cleans up its isolated workspace (CLI `destroy` / `delete`).
    pub fn delete_dimension(&self, name: &str, force: bool) -> Result<(), DimensionError> {
        if name == "mainline" {
            return Err(DimensionError::MainlineReserved);
        }

        let target_dir = self.dimensions_dir.join(name);
        if !target_dir.exists() {
            return Err(DimensionError::NotFound(name.to_string()));
        }

        let current = self.current_dimension_name();
        if current == name && !force {
            return Err(DimensionError::CannotDestroyActive(name.to_string()));
        }

        let is_dirty = self.is_dimension_dirty(name);
        if is_dirty && !force {
            return Err(DimensionError::CannotDestroyDirty(name.to_string()));
        }

        // Acquire lock before removal
        let _guard = DimensionLockGuard::acquire_timeout(
            &self.dimensions_dir,
            name,
            None,
            "destroy",
            Duration::from_secs(5),
        )?;

        fs::remove_dir_all(&target_dir)?;

        if current == name {
            let _ = self.set_current_dimension("mainline");
        }

        Ok(())
    }

    /// Renames an existing dimension.
    pub fn rename_dimension(&self, old_name: &str, new_name: &str) -> Result<(), DimensionError> {
        if old_name == "mainline" || new_name == "mainline" {
            return Err(DimensionError::MainlineReserved);
        }
        Self::validate_dimension_name(new_name)?;

        let old_dir = self.dimensions_dir.join(old_name);
        if !old_dir.exists() {
            return Err(DimensionError::NotFound(old_name.to_string()));
        }

        let new_dir = self.dimensions_dir.join(new_name);
        if new_dir.exists() {
            return Err(DimensionError::AlreadyExists(new_name.to_string()));
        }

        let _guard = DimensionLockGuard::acquire_timeout(
            &self.dimensions_dir,
            old_name,
            None,
            "rename",
            Duration::from_secs(5),
        )?;

        fs::rename(&old_dir, &new_dir)?;

        // Update metadata
        if let Ok(mut meta) = self.read_dimension_metadata(new_name) {
            meta.name = new_name.to_string();
            meta.branch = new_name.to_string();
            let json = serde_json::to_string_pretty(&meta)?;
            let _ = fs::write(new_dir.join("dimension.json"), &json);
            let _ = fs::write(new_dir.join("meta.json"), &json);
        }

        let current = self.current_dimension_name();
        if current == old_name {
            let _ = self.set_current_dimension(new_name);
        }

        Ok(())
    }

    /// Reads metadata for a specific dimension from `dimension.json` (or `meta.json`).
    pub fn read_dimension_metadata(&self, name: &str) -> Result<DimensionMetadata, DimensionError> {
        if name == "mainline" {
            return Ok(DimensionMetadata {
                name: "mainline".to_string(),
                creator: "system".to_string(),
                branch: "main".to_string(),
                cow_mode: "mainline".to_string(),
                status: "clean".to_string(),
                created_at: "".to_string(),
                parent: None,
                head_commit: peel_reference(self.repo.dft_dir(), "HEAD").ok(),
                description: Some("Root mainline dimension".to_string()),
            });
        }

        let dim_dir = self.dimensions_dir.join(name);
        if !dim_dir.exists() {
            return Err(DimensionError::NotFound(name.to_string()));
        }

        let primary = dim_dir.join("dimension.json");
        let fallback = dim_dir.join("meta.json");

        let content = if primary.exists() {
            fs::read_to_string(primary)?
        } else if fallback.exists() {
            fs::read_to_string(fallback)?
        } else {
            return Err(DimensionError::NotFound(format!(
                "{}: missing metadata",
                name
            )));
        };

        let meta = serde_json::from_str::<DimensionMetadata>(&content)?;
        Ok(meta)
    }

    /// Checks whether a dimension workspace contains uncommitted changes.
    pub fn is_dimension_dirty(&self, name: &str) -> bool {
        let current = self.current_dimension_name();
        if current == name {
            if let Some(workdir) = self.repo.workdir() {
                // Check if any tracked/untracked file exists in workdir
                if let Ok(mut entries) = fs::read_dir(workdir) {
                    return entries.any(|e| {
                        if let Ok(entry) = e {
                            entry.file_name() != ".dft"
                        } else {
                            false
                        }
                    });
                }
            }
            return false;
        }

        let ws = self.dimensions_dir.join(name).join("workspace");
        ws.exists()
            && fs::read_dir(&ws)
                .map(|mut r| r.next().is_some())
                .unwrap_or(false)
    }
}

pub fn copy_dir_contents(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !src.exists() {
        return Ok(());
    }
    for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if entry.file_type().is_file() {
            if let Ok(rel) = path.strip_prefix(src) {
                let dest = dst.join(rel);
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(path, dest)?;
            }
        }
    }
    Ok(())
}

fn clean_empty_dirs(root: &Path, dft_dir: &Path) -> std::io::Result<()> {
    for entry in WalkDir::new(root)
        .contents_first(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path == root || path.starts_with(dft_dir) {
            continue;
        }
        if entry.file_type().is_dir() {
            let _ = fs::remove_dir(path);
        }
    }
    Ok(())
}

fn calculate_disk_usage(dir: &Path) -> u64 {
    let mut total = 0;
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    total
}
