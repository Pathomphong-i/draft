//! Low-level Plumbing Operations for Daft VCS.

use crate::cas::{ObjectId, ObjectStore, ObjectType, RawObject};
use crate::diff::tree::flatten_tree;
use crate::error::DaftError;
use crate::merge::{build_hierarchical_tree, get_signature};
use crate::object::{Commit, FileMode, Tree};
use crate::refs::ReferenceTarget;
use crate::repo::Repository;
use crate::worktree::reset::resolve_commit;
use crate::worktree::status::find_untracked_files;
use std::fs;
use std::path::Path;

/// Computes SHA-256 object ID and optionally writes object into CAS.
pub fn hash_object(
    path: &Path,
    object_type: ObjectType,
    write: bool,
    cas: &ObjectStore,
) -> Result<ObjectId, DaftError> {
    let data = fs::read(path)?;
    let raw = RawObject::new(object_type, data);
    let id = raw.compute_id();
    if write {
        let _ = cas.write_raw(&raw)?;
    }
    Ok(id)
}

/// Retrieves raw object by revision or hash.
pub fn cat_file(repo: &Repository, object_ref: &str) -> Result<RawObject, DaftError> {
    let oid = if let Ok(id) = ObjectId::from_hex(object_ref) {
        if repo.cas().has_object(&id) {
            id
        } else {
            resolve_commit(repo, object_ref)?
        }
    } else {
        resolve_commit(repo, object_ref)?
    };

    let raw = repo.cas().read_raw(&oid)?;
    Ok(raw)
}

/// Creates a tree object from the current staging index.
pub fn write_tree(repo: &Repository) -> Result<ObjectId, DaftError> {
    let index = repo.index()?;
    if index.has_conflicts() {
        return Err(DaftError::Config(
            "Cannot write tree: unmerged conflict entries in index".into(),
        ));
    }

    let mut tree_files = std::collections::BTreeMap::new();
    for entry in index.entries() {
        tree_files.insert(entry.path.clone(), (FileMode(entry.mode), entry.oid));
    }

    build_hierarchical_tree(repo.cas().as_ref(), &tree_files)
}

/// Creates a commit object directly from a tree and parents.
pub fn commit_tree(
    repo: &Repository,
    tree_oid: &ObjectId,
    parents: &[ObjectId],
    message: &str,
) -> Result<ObjectId, DaftError> {
    let sig = get_signature();
    let commit = Commit::new(*tree_oid, parents.to_vec(), sig.clone(), sig, message);
    let serialized = commit.serialize();
    let raw = RawObject::new(ObjectType::Commit, serialized);
    let oid = repo.cas().write_raw(&raw)?;
    Ok(oid)
}

#[derive(Debug, Clone)]
pub struct LsTreeEntry {
    pub mode: FileMode,
    pub object_type: ObjectType,
    pub oid: ObjectId,
    pub path: String,
}

/// Lists contents of a tree object.
pub fn ls_tree(
    repo: &Repository,
    tree_ref: &str,
    recursive: bool,
) -> Result<Vec<LsTreeEntry>, DaftError> {
    let tree_oid = if let Ok(oid) = ObjectId::from_hex(tree_ref) {
        if repo.cas().has_object(&oid) {
            oid
        } else {
            let commit_oid = resolve_commit(repo, tree_ref)?;
            let raw = repo.cas().read_raw(&commit_oid)?;
            let commit = Commit::deserialize(&raw.data)?;
            commit.tree
        }
    } else {
        let commit_oid = resolve_commit(repo, tree_ref)?;
        let raw = repo.cas().read_raw(&commit_oid)?;
        let commit = Commit::deserialize(&raw.data)?;
        commit.tree
    };

    let mut results = Vec::new();

    if recursive {
        let mut flat = std::collections::BTreeMap::new();
        flatten_tree(repo.cas().as_ref(), &tree_oid, "", &mut flat)?;
        for (path, (mode, oid)) in flat {
            results.push(LsTreeEntry {
                mode,
                object_type: ObjectType::Blob,
                oid,
                path,
            });
        }
    } else {
        let raw = repo.cas().read_raw(&tree_oid)?;
        let tree = Tree::deserialize(&raw.data)?;
        for entry in tree.entries() {
            let obj_type = if entry.mode.is_tree() {
                ObjectType::Tree
            } else {
                ObjectType::Blob
            };
            results.push(LsTreeEntry {
                mode: entry.mode,
                object_type: obj_type,
                oid: entry.oid,
                path: entry.name.clone(),
            });
        }
    }

    Ok(results)
}

/// Lists files in index or working tree.
pub fn ls_files(
    repo: &Repository,
    stages: bool,
    unmerged: bool,
    untracked: bool,
) -> Result<Vec<String>, DaftError> {
    let index = repo.index()?;
    let mut results = Vec::new();

    if untracked {
        if let Some(workdir) = repo.workdir() {
            let untracked_files = find_untracked_files(workdir, &index, repo.dft_dir())?;
            return Ok(untracked_files);
        }
    }

    for entry in index.entries() {
        if unmerged && entry.stage() == crate::index::Stage::Normal {
            continue;
        }

        if stages {
            results.push(format!(
                "{:06o} {} {}\t{}",
                entry.mode,
                entry.oid,
                entry.stage() as u8,
                entry.path
            ));
        } else {
            results.push(entry.path.clone());
        }
    }

    Ok(results)
}

/// Parses a revision specifier into an ObjectId.
pub fn rev_parse(repo: &Repository, expr: &str) -> Result<ObjectId, DaftError> {
    resolve_commit(repo, expr)
}

/// Updates a reference atomically with optional compare-and-swap check.
pub fn update_ref(
    repo: &Repository,
    ref_name: &str,
    new_oid: &ObjectId,
    old_oid: Option<&ObjectId>,
) -> Result<(), DaftError> {
    let expected = old_oid.map(|id| ReferenceTarget::Direct(*id));
    repo.refs().write_ref(
        ref_name,
        &ReferenceTarget::Direct(*new_oid),
        expected.as_ref(),
        None,
    )?;
    Ok(())
}

/// Reads or updates a symbolic reference.
pub fn symbolic_ref(
    repo: &Repository,
    name: &str,
    target: Option<&str>,
) -> Result<String, DaftError> {
    if let Some(tgt) = target {
        let sym_target = if tgt.starts_with("refs/") {
            tgt.to_string()
        } else {
            format!("refs/heads/{}", tgt)
        };
        repo.refs().write_ref(
            name,
            &ReferenceTarget::Symbolic(sym_target.clone()),
            None,
            None,
        )?;
        Ok(sym_target)
    } else {
        let r = repo.refs().read_ref(name)?;
        match r.target {
            ReferenceTarget::Symbolic(sym) => Ok(sym),
            ReferenceTarget::Direct(_) => Err(DaftError::Config(format!(
                "ref '{}' is not a symbolic ref",
                name
            ))),
        }
    }
}
