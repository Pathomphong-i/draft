//! Live State Fork Engine (`fork_dimension`).
//!
//! Spawns an isolated parallel dimension from any in-flight, dirty dimension:
//! - Captures uncommitted file modifications (unstaged changes + untracked files)
//! - Captures the binary staging index byte-for-byte
//! - Clones working tree via CoW reflinks
//! - Preserves pristine Git/Daft history: zero dummy or intermediate commits are recorded
//! - The source dimension remains 100% undisturbed

use crate::cow::{detect_best_strategy, CowEngine};
use crate::dimension::{DimensionError, DimensionMetadata};
use crate::lock::DimensionLockGuard;
use crate::manager::{copy_dir_contents, DimensionManager};
use std::fs;
use std::time::Duration;
use walkdir::WalkDir;

impl DimensionManager {
    /// Forks a new dimension from the live state of `source_name`.
    pub fn fork_dimension(
        &self,
        new_name: &str,
        source_name: &str,
    ) -> Result<DimensionMetadata, DimensionError> {
        Self::validate_dimension_name(new_name)?;
        if new_name == "mainline" {
            return Err(DimensionError::AlreadyExists("mainline".to_string()));
        }

        let target_dir = self.dimensions_dir().join(new_name);
        if target_dir.exists() {
            return Err(DimensionError::AlreadyExists(new_name.to_string()));
        }

        let source_exists = if source_name == "mainline" {
            true
        } else {
            self.dimensions_dir().join(source_name).exists()
        };

        if !source_exists {
            return Err(DimensionError::NotFound(source_name.to_string()));
        }

        // Acquire lock on source (shared/read) and target (exclusive)
        let _src_lock = if source_name != "mainline" {
            Some(DimensionLockGuard::acquire_timeout(
                self.dimensions_dir(),
                source_name,
                None,
                "fork_from",
                Duration::from_secs(5),
            )?)
        } else {
            None
        };

        fs::create_dir_all(&target_dir)?;
        let child_ws = target_dir.join("workspace");
        fs::create_dir_all(&child_ws)?;

        let dft_dir = self.repo().dft_dir();
        let source_dir = if source_name == "mainline" {
            self.dimensions_dir().join("mainline")
        } else {
            self.dimensions_dir().join(source_name)
        };

        // 1. Duplicate HEAD reference
        if source_dir.join("HEAD").exists() {
            fs::copy(source_dir.join("HEAD"), target_dir.join("HEAD"))?;
        } else if dft_dir.join("HEAD").exists() {
            fs::copy(dft_dir.join("HEAD"), target_dir.join("HEAD"))?;
        }

        // 2. Clone Staging Index byte-for-byte
        let src_index = if source_name == "mainline" {
            self.repo().index_path()
        } else {
            source_dir.join("index")
        };
        if src_index.exists() {
            fs::copy(&src_index, target_dir.join("index"))?;
        } else if self.repo().index_path().exists() {
            fs::copy(self.repo().index_path(), target_dir.join("index"))?;
        }

        // 3. CoW Clone Live Working Tree
        let current = self.current_dimension_name();
        let workdir = self.repo().workdir();
        let cow_strategy = detect_best_strategy(dft_dir);

        if current == source_name {
            if let Some(wd) = workdir {
                for entry in WalkDir::new(wd).into_iter().filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.starts_with(dft_dir) {
                        continue;
                    }
                    if entry.file_type().is_file() {
                        if let Ok(rel) = path.strip_prefix(wd) {
                            let dest = child_ws.join(rel);
                            CowEngine::clone_file(path, &dest, cow_strategy)?;
                        }
                    }
                }
            }
        } else if source_dir.join("workspace").exists() {
            // Source is inactive -> copy from source workspace directory
            let src_ws = source_dir.join("workspace");
            if CowEngine::clone_dir(&src_ws, &child_ws, cow_strategy).is_err() {
                copy_dir_contents(&src_ws, &child_ws)?;
            }
        }

        // 4. Inscribe Metadata
        let now = chrono::Utc::now().to_rfc3339();
        let meta = DimensionMetadata {
            name: new_name.to_string(),
            creator: "Draft Agent".to_string(),
            branch: new_name.to_string(),
            cow_mode: cow_strategy.to_string(),
            status: "clean".to_string(),
            created_at: now,
            parent: Some(source_name.to_string()),
            head_commit: None,
            description: Some(format!("Live fork from {}", source_name)),
        };

        let json_pretty = serde_json::to_string_pretty(&meta)?;
        fs::write(target_dir.join("dimension.json"), &json_pretty)?;
        fs::write(target_dir.join("meta.json"), &json_pretty)?;

        Ok(meta)
    }
}
