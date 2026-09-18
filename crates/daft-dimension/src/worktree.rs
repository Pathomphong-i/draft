//! Dimension Worktree Isolation Adapter (`DimensionRepository`).
//!
//! Encapsulates the Isolation Triplet:
//! 1. Isolated HEAD (`.dft/dimensions/<id>/HEAD`)
//! 2. Isolated Index (`.dft/dimensions/<id>/index`)
//! 3. Dedicated Workspace (`.dft/dimensions/<id>/workspace/`)
//!
//! Enables standard `daft-core` operations (add, status, commit, diff) to run
//! concurrently inside any dimension without touching the root repository worktree.

use crate::dimension::DimensionError;
use daft_core::cas::{ObjectId, ObjectStore};
use daft_core::index::Index;
use daft_core::Repository;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct DimensionRepository {
    repo: Arc<Repository>,
    dimension_name: String,
    workdir: PathBuf,
    index_path: PathBuf,
    head_path: PathBuf,
}

impl DimensionRepository {
    /// Mounts the dimension repository adapter for the named dimension.
    pub fn for_dimension(
        repo: Arc<Repository>,
        dimension_name: &str,
    ) -> Result<Self, DimensionError> {
        let dft_dir = repo.dft_dir();

        let (workdir, index_path, head_path) = if dimension_name == "mainline" {
            let wd = repo
                .workdir()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| dft_dir.to_path_buf());
            (wd, repo.index_path(), dft_dir.join("HEAD"))
        } else {
            let dim_dir = dft_dir.join("dimensions").join(dimension_name);
            if !dim_dir.exists() {
                return Err(DimensionError::NotFound(dimension_name.to_string()));
            }
            (
                dim_dir.join("workspace"),
                dim_dir.join("index"),
                dim_dir.join("HEAD"),
            )
        };

        Ok(Self {
            repo,
            dimension_name: dimension_name.to_string(),
            workdir,
            index_path,
            head_path,
        })
    }

    pub fn dimension_name(&self) -> &str {
        &self.dimension_name
    }

    pub fn workdir(&self) -> &Path {
        &self.workdir
    }

    pub fn index_path(&self) -> &Path {
        &self.index_path
    }

    pub fn head_path(&self) -> &Path {
        &self.head_path
    }

    pub fn cas(&self) -> &Arc<ObjectStore> {
        self.repo.cas()
    }

    pub fn underlying_repo(&self) -> &Arc<Repository> {
        &self.repo
    }

    /// Reads the dimension-specific staging index.
    pub fn index(&self) -> Result<Index, daft_core::error::IndexError> {
        if self.index_path.exists() {
            Index::read_from(&self.index_path)
        } else {
            Ok(Index::new())
        }
    }

    /// Writes the updated staging index back to disk.
    pub fn write_index(&self, index: &Index) -> Result<(), daft_core::error::IndexError> {
        if let Some(parent) = self.index_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        index.write_to(&self.index_path)
    }

    /// Reads the HEAD commit OID or resolves symbolic ref in this dimension.
    pub fn head_commit(&self) -> Option<ObjectId> {
        if let Ok(content) = fs::read_to_string(&self.head_path) {
            let trimmed = content.trim();
            if let Ok(oid) = ObjectId::from_hex(trimmed) {
                return Some(oid);
            }
            if let Some(sym) = trimmed.strip_prefix("ref: ") {
                if let Ok(target) = self.repo.refs().read_ref(sym) {
                    if let daft_core::refs::ReferenceTarget::Direct(oid) = target.target {
                        return Some(oid);
                    }
                }
            }
        }
        None
    }

    /// Sets the dimension's HEAD to direct commit OID or symbolic ref.
    pub fn set_head(&self, target: &str) -> Result<(), DimensionError> {
        fs::write(&self.head_path, format!("{}\n", target))?;
        Ok(())
    }
}
