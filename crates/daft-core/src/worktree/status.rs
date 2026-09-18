//! Status reconciliation between HEAD commit tree, Index, and Working Tree.

use crate::cas::ObjectType;
use crate::diff::tree::flatten_tree;
use crate::error::DaftError;
use crate::object::Commit;
use crate::refs::ReferenceTarget;
use crate::repo::Repository;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Default, Clone)]
pub struct StatusReport {
    pub branch: String,
    pub is_detached: bool,
    pub is_empty_repo: bool,
    pub staged_added: Vec<String>,
    pub staged_modified: Vec<String>,
    pub staged_deleted: Vec<String>,
    pub unstaged_modified: Vec<String>,
    pub unstaged_deleted: Vec<String>,
    pub untracked: Vec<String>,
    pub unmerged: Vec<String>,
}

impl StatusReport {
    pub fn is_clean(&self) -> bool {
        self.staged_added.is_empty()
            && self.staged_modified.is_empty()
            && self.staged_deleted.is_empty()
            && self.unstaged_modified.is_empty()
            && self.unstaged_deleted.is_empty()
            && self.untracked.is_empty()
            && self.unmerged.is_empty()
    }
}

/// Computes the status report for the current repository.
pub fn get_status(repo: &Repository) -> Result<StatusReport, DaftError> {
    let mut report = StatusReport::default();

    // 1. Determine active branch / HEAD
    let head_ref = repo.head()?;
    match &head_ref.target {
        ReferenceTarget::Symbolic(sym) => {
            report.branch = sym.strip_prefix("refs/heads/").unwrap_or(sym).to_string();
            report.is_detached = false;
        }
        ReferenceTarget::Direct(oid) => {
            report.branch = oid.to_hex()[..8].to_string();
            report.is_detached = true;
        }
    }

    // 2. Read HEAD commit tree (if repository has commits)
    let head_commit_oid = crate::refs::peel_reference(repo.dft_dir(), "HEAD").ok();
    let mut head_tree_files = BTreeMap::new();

    if let Some(commit_oid) = head_commit_oid {
        let raw = repo.cas().read_raw(&commit_oid)?;
        if raw.object_type == ObjectType::Commit {
            let commit = Commit::deserialize(&raw.data)?;
            flatten_tree(repo.cas().as_ref(), &commit.tree, "", &mut head_tree_files)?;
        }
    } else {
        report.is_empty_repo = true;
    }

    // 3. Read Index
    let index = repo.index()?;
    let mut index_files = BTreeMap::new();
    let mut unmerged_paths = BTreeSet::new();

    for entry in index.entries() {
        if entry.stage() != crate::index::Stage::Normal {
            unmerged_paths.insert(entry.path.clone());
        } else {
            index_files.insert(
                entry.path.clone(),
                (entry.mode, entry.oid, entry.mtime.sec, entry.file_size),
            );
        }
    }
    report.unmerged = unmerged_paths.into_iter().collect();

    // 4. Staged Changes: Compare HEAD tree vs Index
    let mut all_staged_paths = BTreeSet::new();
    for p in head_tree_files.keys() {
        all_staged_paths.insert(p.clone());
    }
    for p in index_files.keys() {
        all_staged_paths.insert(p.clone());
    }

    for p in all_staged_paths {
        let in_head = head_tree_files.get(&p);
        let in_index = index_files.get(&p);

        match (in_head, in_index) {
            (Some((_, head_oid)), Some((_, idx_oid, _, _))) => {
                if head_oid != idx_oid {
                    report.staged_modified.push(p);
                }
            }
            (None, Some(_)) => {
                report.staged_added.push(p);
            }
            (Some(_), None) => {
                report.staged_deleted.push(p);
            }
            (None, None) => {}
        }
    }

    // 5. Unstaged Modifications & Untracked Files: Compare Index vs Worktree
    if let Some(workdir) = repo.workdir() {
        // Check tracked files in index against disk
        for (path, (_mode, oid, _mtime_sec, size)) in &index_files {
            let full_path = workdir.join(path);
            if !full_path.exists() {
                report.unstaged_deleted.push(path.clone());
            } else if full_path.is_file() {
                let meta = fs::metadata(&full_path)?;
                let disk_size = meta.len() as u32;

                // Quick size check, fallback to byte comparison
                if disk_size != *size {
                    report.unstaged_modified.push(path.clone());
                } else {
                    let disk_bytes = fs::read(&full_path)?;
                    let raw = repo.cas().read_raw(oid)?;
                    if disk_bytes != raw.data {
                        report.unstaged_modified.push(path.clone());
                    }
                }
            }
        }

        // Find untracked files on disk
        let untracked = find_untracked_files(workdir, &index, repo.dft_dir())?;
        report.untracked = untracked;
    }

    report.staged_added.sort();
    report.staged_modified.sort();
    report.staged_deleted.sort();
    report.unstaged_modified.sort();
    report.unstaged_deleted.sort();
    report.untracked.sort();

    Ok(report)
}

/// Recursively traverses workdir to find untracked files.
pub fn find_untracked_files(
    workdir: &Path,
    index: &crate::index::Index,
    dft_dir: &Path,
) -> Result<Vec<String>, DaftError> {
    let mut untracked = Vec::new();

    for entry in WalkDir::new(workdir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            name != ".dft"
                && name != ".git"
                && name != "target"
                && name != ".agents"
                && name != "node_modules"
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !workdir.starts_with(dft_dir) && path.starts_with(dft_dir) {
            continue;
        }

        if entry.file_type().is_file() {
            if let Ok(rel) = path.strip_prefix(workdir) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                if rel_str.is_empty() {
                    continue;
                }
                // Check if in index
                if index
                    .find_entry(&rel_str, crate::index::Stage::Normal)
                    .is_none()
                    && index
                        .find_entry(&rel_str, crate::index::Stage::Ours)
                        .is_none()
                    && index
                        .find_entry(&rel_str, crate::index::Stage::Theirs)
                        .is_none()
                    && index
                        .find_entry(&rel_str, crate::index::Stage::Ancestor)
                        .is_none()
                {
                    untracked.push(rel_str);
                }
            }
        }
    }

    Ok(untracked)
}
