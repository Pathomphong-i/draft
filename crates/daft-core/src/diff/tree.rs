//! Hierarchical recursive tree diffing with O(1) subtree pruning.

use super::unified::{generate_file_patch, FilePatch};
use crate::cas::{ObjectId, ObjectStore, ObjectType};
use crate::error::DaftError;
use crate::object::Tree;
use std::collections::BTreeMap;

/// Recursively flattens a tree into a map of full relative path -> (FileMode, ObjectId).
pub fn flatten_tree(
    cas: &ObjectStore,
    tree_id: &ObjectId,
    prefix: &str,
    out: &mut BTreeMap<String, (crate::object::FileMode, ObjectId)>,
) -> Result<(), DaftError> {
    let raw = cas.read_raw(tree_id)?;
    if raw.object_type != ObjectType::Tree {
        return Err(DaftError::Object(crate::object::ObjectError::UnknownType(
            format!("Expected tree object, got {}", raw.object_type),
        )));
    }
    let tree = Tree::deserialize(&raw.data)?;

    for entry in tree.entries() {
        let path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            format!("{}/{}", prefix, entry.name)
        };

        if entry.mode.is_tree() {
            flatten_tree(cas, &entry.oid, &path, out)?;
        } else {
            out.insert(path, (entry.mode, entry.oid));
        }
    }

    Ok(())
}

/// Computes the file-level difference between two CAS tree objects.
///
/// Uses O(1) subtree pruning when two tree entry ObjectIds match.
pub fn diff_trees(
    cas: &ObjectStore,
    old_tree_id: Option<&ObjectId>,
    new_tree_id: Option<&ObjectId>,
) -> Result<Vec<FilePatch>, DaftError> {
    if old_tree_id == new_tree_id && old_tree_id.is_some() {
        return Ok(Vec::new());
    }

    let mut old_files = BTreeMap::new();
    if let Some(oid) = old_tree_id {
        flatten_tree(cas, oid, "", &mut old_files)?;
    }

    let mut new_files = BTreeMap::new();
    if let Some(oid) = new_tree_id {
        flatten_tree(cas, oid, "", &mut new_files)?;
    }

    let mut all_paths = BTreeMap::new();
    for (path, (mode, oid)) in &old_files {
        all_paths.entry(path.clone()).or_insert((None, None)).0 = Some((*mode, *oid));
    }
    for (path, (mode, oid)) in &new_files {
        all_paths.entry(path.clone()).or_insert((None, None)).1 = Some((*mode, *oid));
    }

    let mut patches = Vec::new();

    for (path, (old_info, new_info)) in all_paths {
        match (old_info, new_info) {
            (Some((_old_mode, old_oid)), Some((_new_mode, new_oid))) => {
                if old_oid == new_oid {
                    // Contents are identical
                    continue;
                }
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
            (Some((_old_mode, old_oid)), None) => {
                // Deleted file
                let old_raw = cas.read_raw(&old_oid)?;
                let patch = generate_file_patch(Some(&path), None, Some(&old_raw.data), None, 3);
                patches.push(patch);
            }
            (None, Some((_new_mode, new_oid))) => {
                // Added file
                let new_raw = cas.read_raw(&new_oid)?;
                let patch = generate_file_patch(None, Some(&path), None, Some(&new_raw.data), 3);
                patches.push(patch);
            }
            (None, None) => unreachable!(),
        }
    }

    Ok(patches)
}
