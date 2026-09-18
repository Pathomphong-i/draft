use crate::error::AwarenessError;
use daft_core::diff::tree::flatten_tree;
use daft_core::diff::unified::generate_file_patch;
use daft_core::diff::FilePatch;
use daft_core::graph::{walk_commits, StoreCommitGraph};
use daft_core::history::LogEntry;
use daft_core::object::Commit;
use daft_core::Repository;
use daft_dimension::DimensionRepository;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionStatusReport {
    pub dimension: String,
    pub branch: String,
    pub head_oid: Option<String>,
    pub is_clean: bool,
    pub staged_added: Vec<PathBuf>,
    pub staged_modified: Vec<PathBuf>,
    pub staged_deleted: Vec<PathBuf>,
    pub unstaged_modified: Vec<PathBuf>,
    pub unstaged_deleted: Vec<PathBuf>,
    pub untracked: Vec<PathBuf>,
}

pub struct ObserveSubsystem {
    repo: Arc<Repository>,
}

impl ObserveSubsystem {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self { repo }
    }

    /// Validates if dimension exists or is mainline.
    fn ensure_dimension_exists(&self, dimension: &str) -> Result<(), AwarenessError> {
        if dimension == "mainline" {
            return Ok(());
        }
        let dim_dir = self.repo.dft_dir().join("dimensions").join(dimension);
        if !dim_dir.exists() {
            return Err(AwarenessError::DimensionNotFound(dimension.to_string()));
        }
        Ok(())
    }

    /// Non-destructively reads file content in target dimension using 3-tier fallback.
    pub fn cat_file(&self, dimension: &str, path: &Path) -> Result<Vec<u8>, AwarenessError> {
        self.ensure_dimension_exists(dimension)?;

        let rel_path_str = path
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches("./")
            .to_string();

        let dim_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), dimension)?;

        // Tier 1: Dedicated Workspace
        let ws_file = dim_repo.workdir().join(path);
        if ws_file.is_file() {
            if let Ok(bytes) = fs::read(&ws_file) {
                return Ok(bytes);
            }
        }

        // Tier 2: Staging Index
        if let Ok(index) = dim_repo.index() {
            if let Some(entry) = index.entries().iter().find(|e| e.path == rel_path_str) {
                if let Ok(raw) = self.repo.cas().read_raw(&entry.oid) {
                    return Ok(raw.data);
                }
            }
        }

        // Tier 3: HEAD Commit Tree
        if let Some(head_oid) = dim_repo.head_commit() {
            if let Ok(head_raw) = self.repo.cas().read_raw(&head_oid) {
                if let Ok(commit) = Commit::deserialize(&head_raw.data) {
                    let mut files = BTreeMap::new();
                    if flatten_tree(self.repo.cas().as_ref(), &commit.tree, "", &mut files).is_ok()
                    {
                        if let Some((_mode, oid)) = files.get(&rel_path_str) {
                            if let Ok(raw) = self.repo.cas().read_raw(oid) {
                                return Ok(raw.data);
                            }
                        }
                    }
                }
            }
        }

        Err(AwarenessError::FileNotFoundInDimension {
            path: path.to_path_buf(),
            dimension: dimension.to_string(),
        })
    }

    /// Remotely inspects working tree, staging index, and HEAD status of dimension.
    pub fn status(&self, dimension: &str) -> Result<DimensionStatusReport, AwarenessError> {
        self.ensure_dimension_exists(dimension)?;

        let dim_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), dimension)?;
        let head_oid_opt = dim_repo.head_commit();

        let mut head_files = BTreeMap::new();
        if let Some(head_oid) = head_oid_opt {
            if let Ok(raw) = self.repo.cas().read_raw(&head_oid) {
                if let Ok(commit) = Commit::deserialize(&raw.data) {
                    let _ =
                        flatten_tree(self.repo.cas().as_ref(), &commit.tree, "", &mut head_files);
                }
            }
        }

        let index = dim_repo.index().unwrap_or_default();
        let mut index_entries = BTreeMap::new();
        for entry in index.entries() {
            index_entries.insert(entry.path.clone(), entry);
        }

        let mut staged_added = Vec::new();
        let mut staged_modified = Vec::new();
        let mut staged_deleted = Vec::new();

        // Staged changes: index vs HEAD tree
        for (path, entry) in &index_entries {
            match head_files.get(path) {
                Some((_head_mode, head_oid)) => {
                    if entry.oid != *head_oid {
                        staged_modified.push(PathBuf::from(path));
                    }
                }
                None => {
                    staged_added.push(PathBuf::from(path));
                }
            }
        }
        for path in head_files.keys() {
            if !index_entries.contains_key(path) {
                staged_deleted.push(PathBuf::from(path));
            }
        }

        // Unstaged changes: workspace vs index
        let mut unstaged_modified = Vec::new();
        let mut unstaged_deleted = Vec::new();
        let mut untracked = Vec::new();

        let workdir = dim_repo.workdir();
        if workdir.exists() {
            for entry in WalkDir::new(workdir)
                .into_iter()
                .filter_entry(|e| {
                    let name = e.file_name().to_string_lossy();
                    name != ".dft" && name != ".git"
                })
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() {
                    if let Ok(rel) = entry.path().strip_prefix(workdir) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        if let Some(idx_entry) = index_entries.get(&rel_str) {
                            // Check file size or modification
                            if let Ok(meta) = entry.metadata() {
                                if meta.len() != idx_entry.file_size as u64 {
                                    unstaged_modified.push(PathBuf::from(&rel_str));
                                } else if let Ok(data) = fs::read(entry.path()) {
                                    let current_oid =
                                        daft_core::cas::RawObject::blob(data).compute_id();
                                    if current_oid != idx_entry.oid {
                                        unstaged_modified.push(PathBuf::from(&rel_str));
                                    }
                                }
                            }
                        } else {
                            untracked.push(PathBuf::from(&rel_str));
                        }
                    }
                }
            }

            for path in index_entries.keys() {
                let p = workdir.join(path);
                if !p.exists() {
                    unstaged_deleted.push(PathBuf::from(path));
                }
            }
        }

        staged_added.sort();
        staged_modified.sort();
        staged_deleted.sort();
        unstaged_modified.sort();
        unstaged_deleted.sort();
        untracked.sort();

        let is_clean = staged_added.is_empty()
            && staged_modified.is_empty()
            && staged_deleted.is_empty()
            && unstaged_modified.is_empty()
            && unstaged_deleted.is_empty()
            && untracked.is_empty();

        Ok(DimensionStatusReport {
            dimension: dimension.to_string(),
            branch: dimension.to_string(),
            head_oid: head_oid_opt.map(|o| o.to_hex()),
            is_clean,
            staged_added,
            staged_modified,
            staged_deleted,
            unstaged_modified,
            unstaged_deleted,
            untracked,
        })
    }

    /// Computes virtual files projection for a dimension (HEAD tree + Index + Workspace).
    pub fn get_dimension_virtual_files(
        &self,
        dimension: &str,
    ) -> Result<BTreeMap<String, Vec<u8>>, AwarenessError> {
        self.ensure_dimension_exists(dimension)?;

        let mut files = BTreeMap::new();
        let dim_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), dimension)?;

        // 1. HEAD tree
        if let Some(head_oid) = dim_repo.head_commit() {
            if let Ok(raw) = self.repo.cas().read_raw(&head_oid) {
                if let Ok(commit) = Commit::deserialize(&raw.data) {
                    let mut head_entries = BTreeMap::new();
                    let _ = flatten_tree(
                        self.repo.cas().as_ref(),
                        &commit.tree,
                        "",
                        &mut head_entries,
                    );
                    for (path, (_mode, oid)) in head_entries {
                        if let Ok(blob_raw) = self.repo.cas().read_raw(&oid) {
                            files.insert(path, blob_raw.data);
                        }
                    }
                }
            }
        }

        // 2. Index overlay
        if let Ok(index) = dim_repo.index() {
            for entry in index.entries() {
                if let Ok(blob_raw) = self.repo.cas().read_raw(&entry.oid) {
                    files.insert(entry.path.clone(), blob_raw.data);
                }
            }
        }

        // 3. Workspace overlay
        let workdir = dim_repo.workdir();
        if workdir.exists() {
            for entry in WalkDir::new(workdir)
                .into_iter()
                .filter_entry(|e| {
                    let name = e.file_name().to_string_lossy();
                    name != ".dft" && name != ".git"
                })
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() {
                    if let Ok(rel) = entry.path().strip_prefix(workdir) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        if let Ok(bytes) = fs::read(entry.path()) {
                            files.insert(rel_str, bytes);
                        }
                    }
                }
            }
        }

        Ok(files)
    }

    /// Computes unified diff between two dimensions without switching to either.
    pub fn diff(
        &self,
        dim1: &str,
        dim2: &str,
        path_filter: Option<&Path>,
    ) -> Result<Vec<FilePatch>, AwarenessError> {
        let files1 = self.get_dimension_virtual_files(dim1)?;
        let files2 = self.get_dimension_virtual_files(dim2)?;

        let filter_str = path_filter.map(|p| p.to_string_lossy().replace('\\', "/"));

        let mut all_paths = BTreeSet::new();
        all_paths.extend(files1.keys().cloned());
        all_paths.extend(files2.keys().cloned());

        let mut patches = Vec::new();
        for path in all_paths {
            if let Some(ref filter) = filter_str {
                if &path != filter && !path.starts_with(&format!("{}/", filter)) {
                    continue;
                }
            }

            let d1 = files1.get(&path).map(|v| v.as_slice());
            let d2 = files2.get(&path).map(|v| v.as_slice());

            if d1 != d2 {
                let patch = generate_file_patch(
                    d1.map(|_| path.as_str()),
                    d2.map(|_| path.as_str()),
                    d1,
                    d2,
                    3,
                );
                patches.push(patch);
            }
        }

        Ok(patches)
    }

    /// Traverses commit DAG of target dimension from its current HEAD.
    pub fn log(
        &self,
        dimension: &str,
        max_count: Option<usize>,
    ) -> Result<Vec<LogEntry>, AwarenessError> {
        self.ensure_dimension_exists(dimension)?;
        let dim_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), dimension)?;

        let head_oid = match dim_repo.head_commit() {
            Some(oid) => oid,
            None => return Ok(Vec::new()),
        };

        let graph = StoreCommitGraph::new(self.repo.cas().as_ref());
        let ordered = walk_commits(&graph, &[head_oid])?;

        let mut entries = Vec::new();
        for oid in ordered {
            if let Some(limit) = max_count {
                if entries.len() >= limit {
                    break;
                }
            }
            if let Ok(raw) = self.repo.cas().read_raw(&oid) {
                if let Ok(commit) = Commit::deserialize(&raw.data) {
                    entries.push(LogEntry {
                        oid,
                        parents: commit.parents,
                        author: commit.author,
                        committer: commit.committer,
                        message: commit.message,
                    });
                }
            }
        }

        Ok(entries)
    }
}
