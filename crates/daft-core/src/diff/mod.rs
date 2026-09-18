//! Diff Engine for Daft VCS.

pub mod binary;
pub mod myers;
pub mod tree;
pub mod unified;

pub use binary::is_binary;
pub use myers::{myers_diff, EditOp, EditOpKind};
pub use tree::{diff_trees, flatten_tree};
pub use unified::{
    create_hunks, format_unified_diff, generate_file_patch, FilePatch, Hunk, HunkLine,
};

use crate::cas::{ObjectId, ObjectStore, ObjectType};
use crate::error::DaftError;
use crate::index::Index;
use crate::object::Commit;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Computes the diff between the working tree and the staging index.
/// Only inspects files tracked in the index (untracked files are ignored by diff).
pub fn diff_worktree_to_index(
    workdir: &Path,
    index: &Index,
    cas: &ObjectStore,
) -> Result<Vec<FilePatch>, DaftError> {
    let mut patches = Vec::new();

    for entry in index.entries() {
        let file_path = workdir.join(&entry.path);
        let old_raw = cas.read_raw(&entry.oid)?;

        if !file_path.exists() {
            // Tracked in index but deleted from worktree
            let patch = generate_file_patch(Some(&entry.path), None, Some(&old_raw.data), None, 3);
            patches.push(patch);
        } else if file_path.is_file() {
            let disk_bytes = fs::read(&file_path)?;
            if disk_bytes != old_raw.data {
                let patch = generate_file_patch(
                    Some(&entry.path),
                    Some(&entry.path),
                    Some(&old_raw.data),
                    Some(&disk_bytes),
                    3,
                );
                patches.push(patch);
            }
        }
    }

    Ok(patches)
}

/// Computes the diff between the staging index and a target commit tree.
pub fn diff_index_to_tree(
    index: &Index,
    tree_oid: Option<&ObjectId>,
    cas: &ObjectStore,
) -> Result<Vec<FilePatch>, DaftError> {
    let mut tree_files = BTreeMap::new();
    if let Some(oid) = tree_oid {
        flatten_tree(cas, oid, "", &mut tree_files)?;
    }

    let mut index_files = BTreeMap::new();
    for entry in index.entries() {
        index_files.insert(entry.path.clone(), (entry.mode, entry.oid));
    }

    let mut all_paths = BTreeMap::new();
    for (path, info) in &tree_files {
        all_paths.entry(path.clone()).or_insert((None, None)).0 = Some(*info);
    }
    for (path, info) in &index_files {
        all_paths.entry(path.clone()).or_insert((None, None)).1 = Some(*info);
    }

    let mut patches = Vec::new();
    for (path, (old_info, new_info)) in all_paths {
        match (old_info, new_info) {
            (Some((_, old_oid)), Some((_, new_oid))) => {
                if old_oid != new_oid {
                    let old_raw = cas.read_raw(&old_oid)?;
                    let new_raw = cas.read_raw(&new_oid)?;
                    let patch = generate_file_patch(
                        Some(&path),
                        Some(&path),
                        Some(&old_raw.data),
                        Some(&new_raw.data),
                        3,
                    );
                    patches.push(patch);
                }
            }
            (Some((_, old_oid)), None) => {
                let old_raw = cas.read_raw(&old_oid)?;
                let patch = generate_file_patch(Some(&path), None, Some(&old_raw.data), None, 3);
                patches.push(patch);
            }
            (None, Some((_, new_oid))) => {
                let new_raw = cas.read_raw(&new_oid)?;
                let patch = generate_file_patch(None, Some(&path), None, Some(&new_raw.data), 3);
                patches.push(patch);
            }
            (None, None) => unreachable!(),
        }
    }

    Ok(patches)
}

/// Computes the diff between two commits.
pub fn diff_commits(
    commit_a_id: &ObjectId,
    commit_b_id: &ObjectId,
    cas: &ObjectStore,
) -> Result<Vec<FilePatch>, DaftError> {
    let raw_a = cas.read_raw(commit_a_id)?;
    let raw_b = cas.read_raw(commit_b_id)?;

    if raw_a.object_type != ObjectType::Commit || raw_b.object_type != ObjectType::Commit {
        return Err(DaftError::Object(crate::object::ObjectError::UnknownType(
            "Expected commit objects for diff_commits".to_string(),
        )));
    }

    let commit_a = Commit::deserialize(&raw_a.data)?;
    let commit_b = Commit::deserialize(&raw_b.data)?;

    diff_trees(cas, Some(&commit_a.tree), Some(&commit_b.tree))
}
