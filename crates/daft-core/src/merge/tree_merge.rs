//! 3-way tree merge algorithm and decision matrix.

use super::three_way::{merge_text_3way, TextMergeResult};
use crate::cas::{ObjectId, ObjectStore, ObjectType};
use crate::diff::binary::is_binary;
use crate::diff::tree::flatten_tree;
use crate::error::DaftError;
use crate::object::{Blob, FileMode, Tree, TreeEntry};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct ConflictedFile {
    pub base: Option<ObjectId>,
    pub ours: Option<ObjectId>,
    pub theirs: Option<ObjectId>,
    pub marker_content: String,
}

#[derive(Debug, Clone)]
pub struct TreeMergeResult {
    pub clean_files: BTreeMap<String, (FileMode, ObjectId)>,
    pub conflicted_files: BTreeMap<String, ConflictedFile>,
}

impl TreeMergeResult {
    pub fn has_conflicts(&self) -> bool {
        !self.conflicted_files.is_empty()
    }

    /// If no conflicts exist, constructs the merged Tree object in CAS and returns its ObjectId.
    pub fn write_clean_tree(&self, cas: &ObjectStore) -> Result<ObjectId, DaftError> {
        if self.has_conflicts() {
            return Err(DaftError::Config(
                "Cannot write tree with unresolved merge conflicts".into(),
            ));
        }
        build_hierarchical_tree(cas, &self.clean_files)
    }
}

/// Recursively builds hierarchical Tree objects from a flat map of relative paths to (FileMode, ObjectId).
pub fn build_hierarchical_tree(
    cas: &ObjectStore,
    files: &BTreeMap<String, (FileMode, ObjectId)>,
) -> Result<ObjectId, DaftError> {
    // Group files by root directory component
    let mut direct_entries = Vec::new();
    let mut subdirs: BTreeMap<String, BTreeMap<String, (FileMode, ObjectId)>> = BTreeMap::new();

    for (path, (mode, oid)) in files {
        if let Some(slash_pos) = path.find('/') {
            let dir_name = &path[..slash_pos];
            let rest = &path[slash_pos + 1..];
            subdirs
                .entry(dir_name.to_string())
                .or_default()
                .insert(rest.to_string(), (*mode, *oid));
        } else {
            direct_entries.push(TreeEntry::new(*mode, path.as_str(), *oid)?);
        }
    }

    // Recurse into subdirectories
    for (dir_name, dir_files) in subdirs {
        let subtree_oid = build_hierarchical_tree(cas, &dir_files)?;
        direct_entries.push(TreeEntry::new(FileMode::TREE, dir_name, subtree_oid)?);
    }

    let tree = Tree::from_entries(direct_entries)?;
    let serialized = tree.serialize();
    let raw_obj = crate::cas::RawObject::new(ObjectType::Tree, serialized);
    let oid = cas.write_raw(&raw_obj)?;
    Ok(oid)
}

/// Executes a 3-way tree merge over base, ours, and theirs tree objects.
pub fn merge_trees_3way(
    cas: &ObjectStore,
    base_tree_id: Option<&ObjectId>,
    ours_tree_id: &ObjectId,
    theirs_tree_id: &ObjectId,
    ours_label: &str,
    theirs_label: &str,
) -> Result<TreeMergeResult, DaftError> {
    let mut base_files = BTreeMap::new();
    if let Some(b) = base_tree_id {
        flatten_tree(cas, b, "", &mut base_files)?;
    }

    let mut ours_files = BTreeMap::new();
    flatten_tree(cas, ours_tree_id, "", &mut ours_files)?;

    let mut theirs_files = BTreeMap::new();
    flatten_tree(cas, theirs_tree_id, "", &mut theirs_files)?;

    let mut all_paths = BTreeMap::new();
    for (p, info) in &base_files {
        all_paths.entry(p.clone()).or_insert((None, None, None)).0 = Some(*info);
    }
    for (p, info) in &ours_files {
        all_paths.entry(p.clone()).or_insert((None, None, None)).1 = Some(*info);
    }
    for (p, info) in &theirs_files {
        all_paths.entry(p.clone()).or_insert((None, None, None)).2 = Some(*info);
    }

    let mut clean_files = BTreeMap::new();
    let mut conflicted_files = BTreeMap::new();

    for (path, (base_info, ours_info, theirs_info)) in all_paths {
        match (base_info, ours_info, theirs_info) {
            // Case 1: Unchanged in all three
            (Some(o), Some(a), Some(b)) if o.1 == a.1 && a.1 == b.1 => {
                clean_files.insert(path, a);
            }
            // Case 2: Only ours changed
            (Some(o), Some(a), Some(b)) if o.1 == b.1 && o.1 != a.1 => {
                clean_files.insert(path, a);
            }
            // Case 3: Only theirs changed
            (Some(o), Some(a), Some(b)) if o.1 == a.1 && o.1 != b.1 => {
                clean_files.insert(path, b);
            }
            // Case 4: Both changed identically
            (Some(_), Some(a), Some(b)) if a.1 == b.1 => {
                clean_files.insert(path, a);
            }
            // Case 5: Deleted by ours, unchanged by theirs
            (Some(o), None, Some(b)) if o.1 == b.1 => {
                // Clean delete
            }
            // Case 6: Deleted by theirs, unchanged by ours
            (Some(o), Some(a), None) if o.1 == a.1 => {
                // Clean delete
            }
            // Case 7: Deleted by both
            (Some(_), None, None) => {
                // Clean delete
            }
            // Case 8: Added by ours only
            (None, Some(a), None) => {
                clean_files.insert(path, a);
            }
            // Case 9: Added by theirs only
            (None, None, Some(b)) => {
                clean_files.insert(path, b);
            }
            // Case 10: Added by both identically
            (None, Some(a), Some(b)) if a.1 == b.1 => {
                clean_files.insert(path, a);
            }
            // Case 11: Added by both differently (Add/Add conflict)
            (None, Some(a), Some(b)) => {
                let a_raw = cas.read_raw(&a.1)?;
                let b_raw = cas.read_raw(&b.1)?;
                let text_res = if is_binary(&a_raw.data) || is_binary(&b_raw.data) {
                    TextMergeResult {
                        content: format!(
                            "<<<<<<< {}\n[Binary file]\n=======\n[Binary file]\n>>>>>>> {}\n",
                            ours_label, theirs_label
                        ),
                        has_conflicts: true,
                    }
                } else {
                    let a_str = std::str::from_utf8(&a_raw.data).unwrap_or("");
                    let b_str = std::str::from_utf8(&b_raw.data).unwrap_or("");
                    merge_text_3way("", a_str, b_str, ours_label, theirs_label)
                };

                conflicted_files.insert(
                    path,
                    ConflictedFile {
                        base: None,
                        ours: Some(a.1),
                        theirs: Some(b.1),
                        marker_content: text_res.content,
                    },
                );
            }
            // Case 12: Modify/Delete conflict (Ours modified, theirs deleted)
            (Some(o), Some(a), None) => {
                let a_raw = cas.read_raw(&a.1)?;
                let a_str = std::str::from_utf8(&a_raw.data).unwrap_or("");
                conflicted_files.insert(
                    path,
                    ConflictedFile {
                        base: Some(o.1),
                        ours: Some(a.1),
                        theirs: None,
                        marker_content: format!(
                            "<<<<<<< {}\n{}=======\n>>>>>>> {}\n",
                            ours_label, a_str, theirs_label
                        ),
                    },
                );
            }
            // Case 13: Delete/Modify conflict (Ours deleted, theirs modified)
            (Some(o), None, Some(b)) => {
                let b_raw = cas.read_raw(&b.1)?;
                let b_str = std::str::from_utf8(&b_raw.data).unwrap_or("");
                conflicted_files.insert(
                    path,
                    ConflictedFile {
                        base: Some(o.1),
                        ours: None,
                        theirs: Some(b.1),
                        marker_content: format!(
                            "<<<<<<< {}\n=======\n{}>>>>>>> {}\n",
                            ours_label, b_str, theirs_label
                        ),
                    },
                );
            }
            // Case 14: Modified by both differently (Text or Binary merge)
            (Some(o), Some(a), Some(b)) => {
                let o_raw = cas.read_raw(&o.1)?;
                let a_raw = cas.read_raw(&a.1)?;
                let b_raw = cas.read_raw(&b.1)?;

                if is_binary(&o_raw.data) || is_binary(&a_raw.data) || is_binary(&b_raw.data) {
                    conflicted_files.insert(
                        path,
                        ConflictedFile {
                            base: Some(o.1),
                            ours: Some(a.1),
                            theirs: Some(b.1),
                            marker_content: format!(
                                "<<<<<<< {}\n[Binary file]\n=======\n[Binary file]\n>>>>>>> {}\n",
                                ours_label, theirs_label
                            ),
                        },
                    );
                } else {
                    let o_str = std::str::from_utf8(&o_raw.data).unwrap_or("");
                    let a_str = std::str::from_utf8(&a_raw.data).unwrap_or("");
                    let b_str = std::str::from_utf8(&b_raw.data).unwrap_or("");

                    let text_res = merge_text_3way(o_str, a_str, b_str, ours_label, theirs_label);
                    if text_res.has_conflicts {
                        conflicted_files.insert(
                            path,
                            ConflictedFile {
                                base: Some(o.1),
                                ours: Some(a.1),
                                theirs: Some(b.1),
                                marker_content: text_res.content,
                            },
                        );
                    } else {
                        // Clean 3-way text merge! Write new blob to CAS
                        let new_blob = Blob::new(text_res.content.into_bytes());
                        let raw_obj =
                            crate::cas::RawObject::new(ObjectType::Blob, new_blob.data().to_vec());
                        let new_oid = cas.write_raw(&raw_obj)?;
                        clean_files.insert(path, (a.0, new_oid));
                    }
                }
            }
            (None, None, None) => unreachable!(),
        }
    }

    Ok(TreeMergeResult {
        clean_files,
        conflicted_files,
    })
}
