//! 3-Way Recursive Merge Engine for Daft VCS.

pub mod three_way;
pub mod tree_merge;

pub use three_way::{merge_text_3way, TextMergeResult};
pub use tree_merge::{build_hierarchical_tree, merge_trees_3way, ConflictedFile, TreeMergeResult};

use crate::cas::{ObjectId, ObjectStore, ObjectType};
use crate::diff::tree::flatten_tree;
use crate::error::DaftError;
use crate::graph::{all_merge_bases, is_ancestor};
use crate::index::{Index, IndexEntry, IndexTime, Stage};
use crate::object::{Commit, Signature};
use crate::refs::ReferenceTarget;
use crate::repo::Repository;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub enum MergeOutcome {
    AlreadyUpToDate,
    FastForward { new_head: ObjectId },
    Clean { commit_oid: ObjectId },
    Conflict { conflicting_files: Vec<String> },
}

/// Resolves user signature from environment variables or sensible defaults.
pub fn get_signature() -> Signature {
    let name = std::env::var("DFT_AUTHOR_NAME")
        .or_else(|_| std::env::var("DRAFT_AUTHOR_NAME"))
        .or_else(|_| std::env::var("GIT_AUTHOR_NAME"))
        .unwrap_or_else(|_| "Draft User".to_string());
    let email = std::env::var("DFT_AUTHOR_EMAIL")
        .or_else(|_| std::env::var("DRAFT_AUTHOR_EMAIL"))
        .or_else(|_| std::env::var("GIT_AUTHOR_EMAIL"))
        .unwrap_or_else(|_| "user@draft-vcs.org".to_string());
    Signature::now(name, email)
}

/// Restores / writes a tree object to working directory and syncs the index.
pub fn checkout_tree(
    cas: &ObjectStore,
    tree_oid: &ObjectId,
    workdir: &Path,
    index: &mut Index,
) -> Result<(), DaftError> {
    let mut tree_files = std::collections::BTreeMap::new();
    flatten_tree(cas, tree_oid, "", &mut tree_files)?;

    // Remove deleted files from disk & index
    let mut paths_to_remove = Vec::new();
    for entry in index.entries() {
        if !tree_files.contains_key(&entry.path) {
            paths_to_remove.push(entry.path.clone());
            let p = workdir.join(&entry.path);
            if p.exists() {
                let _ = fs::remove_file(&p);
            }
        }
    }
    for p in paths_to_remove {
        index.remove_path(&p);
    }

    // Write added / updated files
    for (path, (mode, oid)) in tree_files {
        let raw = cas.read_raw(&oid)?;
        let full_path = workdir.join(&path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&full_path, &raw.data)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if mode.is_executable() {
                let _ = fs::set_permissions(&full_path, fs::Permissions::from_mode(0o755));
            } else {
                let _ = fs::set_permissions(&full_path, fs::Permissions::from_mode(0o644));
            }
        }

        let meta = fs::metadata(&full_path)?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| IndexTime {
                sec: d.as_secs() as u32,
                nsec: d.subsec_nanos(),
            })
            .unwrap_or_default();

        let mut entry = IndexEntry::new(&path, oid, mode.0, Stage::Normal, raw.data.len() as u32)?;
        entry.mtime = mtime;
        index.add_entry(entry);
    }

    Ok(())
}

/// Merges `theirs` commit into current HEAD.
pub fn merge_commits(
    repo: &Repository,
    theirs_name: &str,
    theirs_oid: &ObjectId,
    message: Option<&str>,
) -> Result<MergeOutcome, DaftError> {
    let head_ref = repo.head()?;
    let ours_oid = match head_ref.target {
        ReferenceTarget::Direct(oid) => oid,
        ReferenceTarget::Symbolic(ref sym) => {
            let target = repo.refs().read_ref(sym)?;
            match target.target {
                ReferenceTarget::Direct(oid) => oid,
                ReferenceTarget::Symbolic(_) => {
                    return Err(DaftError::Ref(crate::error::RefError::SymbolicRefLoop(
                        sym.clone(),
                    )));
                }
            }
        }
    };

    if ours_oid == *theirs_oid {
        return Ok(MergeOutcome::AlreadyUpToDate);
    }

    let cas = repo.cas();
    let graph = crate::graph::StoreCommitGraph::new(cas.as_ref());

    // Check if theirs is already an ancestor of ours
    if is_ancestor(&graph, theirs_oid, &ours_oid)? {
        return Ok(MergeOutcome::AlreadyUpToDate);
    }

    // Check if fast-forward is possible (ours is ancestor of theirs)
    if is_ancestor(&graph, &ours_oid, theirs_oid)? {
        // Fast-forward merge!
        let theirs_raw = cas.read_raw(theirs_oid)?;
        let theirs_commit = Commit::deserialize(&theirs_raw.data)?;

        // Update branch ref or HEAD
        if let ReferenceTarget::Symbolic(target_name) = &head_ref.target {
            repo.refs().write_ref(
                target_name,
                &ReferenceTarget::Direct(*theirs_oid),
                None,
                None,
            )?;
        } else {
            repo.set_head(&ReferenceTarget::Direct(*theirs_oid))?;
        }

        // Checkout tree and update index
        if let Some(workdir) = repo.workdir() {
            let mut index = repo.index()?;
            checkout_tree(cas.as_ref(), &theirs_commit.tree, workdir, &mut index)?;
            index.write_to(&repo.index_path())?;
        }

        return Ok(MergeOutcome::FastForward {
            new_head: *theirs_oid,
        });
    }

    // Standard 3-way merge
    let bases = all_merge_bases(&graph, &ours_oid, theirs_oid)?;
    let base_tree_oid = if let Some(base_oid) = bases.first() {
        let base_raw = cas.read_raw(base_oid)?;
        let base_commit = Commit::deserialize(&base_raw.data)?;
        Some(base_commit.tree)
    } else {
        None
    };

    let ours_raw = cas.read_raw(&ours_oid)?;
    let ours_commit = Commit::deserialize(&ours_raw.data)?;
    let theirs_raw = cas.read_raw(theirs_oid)?;
    let theirs_commit = Commit::deserialize(&theirs_raw.data)?;

    let merge_result = merge_trees_3way(
        cas.as_ref(),
        base_tree_oid.as_ref(),
        &ours_commit.tree,
        &theirs_commit.tree,
        "HEAD",
        theirs_name,
    )?;

    if !merge_result.has_conflicts() {
        // Clean merge!
        let merged_tree_oid = merge_result.write_clean_tree(cas.as_ref())?;
        let sig = get_signature();
        let default_msg = format!("Merge branch '{}'", theirs_name);
        let commit_msg = message.unwrap_or(&default_msg);

        let new_commit = Commit::new(
            merged_tree_oid,
            vec![ours_oid, *theirs_oid],
            sig.clone(),
            sig,
            commit_msg,
        );
        let new_commit_raw = new_commit.serialize();
        let raw_commit_obj = crate::cas::RawObject::new(ObjectType::Commit, new_commit_raw);
        let new_commit_oid = cas.write_raw(&raw_commit_obj)?;

        // Update branch ref or HEAD
        if let ReferenceTarget::Symbolic(target_name) = &head_ref.target {
            repo.refs().write_ref(
                target_name,
                &ReferenceTarget::Direct(new_commit_oid),
                None,
                None,
            )?;
        } else {
            repo.set_head(&ReferenceTarget::Direct(new_commit_oid))?;
        }

        // Checkout tree and update index
        if let Some(workdir) = repo.workdir() {
            let mut index = repo.index()?;
            checkout_tree(cas.as_ref(), &merged_tree_oid, workdir, &mut index)?;
            index.write_to(&repo.index_path())?;
        }

        // Clean up MERGE_HEAD if any
        let merge_head = repo.dft_dir().join("MERGE_HEAD");
        if merge_head.exists() {
            let _ = fs::remove_file(merge_head);
        }
        let merge_msg = repo.dft_dir().join("MERGE_MSG");
        if merge_msg.exists() {
            let _ = fs::remove_file(merge_msg);
        }

        Ok(MergeOutcome::Clean {
            commit_oid: new_commit_oid,
        })
    } else {
        // Conflicts encountered
        let mut index = repo.index()?;
        let workdir = repo.workdir().ok_or_else(|| {
            DaftError::Config("Cannot perform conflicted merge in bare repository".into())
        })?;

        // Write clean files
        for (path, (mode, oid)) in &merge_result.clean_files {
            let raw = cas.read_raw(oid)?;
            let full_path = workdir.join(path);
            if let Some(p) = full_path.parent() {
                fs::create_dir_all(p)?;
            }
            fs::write(&full_path, &raw.data)?;
            index.add_entry(IndexEntry::new(
                path,
                *oid,
                mode.0,
                Stage::Normal,
                raw.data.len() as u32,
            )?);
        }

        // Write conflicted files and stages
        let mut conflicting_paths = Vec::new();
        for (path, conf) in &merge_result.conflicted_files {
            conflicting_paths.push(path.clone());
            let full_path = workdir.join(path);
            if let Some(p) = full_path.parent() {
                fs::create_dir_all(p)?;
            }
            fs::write(&full_path, conf.marker_content.as_bytes())?;

            // Remove normal stage if any
            index.remove_path(path);

            if let Some(base_oid) = conf.base {
                index.add_entry(IndexEntry::new(
                    path,
                    base_oid,
                    0o100644,
                    Stage::Ancestor,
                    0,
                )?);
            }
            if let Some(ours_oid) = conf.ours {
                index.add_entry(IndexEntry::new(path, ours_oid, 0o100644, Stage::Ours, 0)?);
            }
            if let Some(theirs_oid) = conf.theirs {
                index.add_entry(IndexEntry::new(
                    path,
                    theirs_oid,
                    0o100644,
                    Stage::Theirs,
                    0,
                )?);
            }
        }

        index.write_to(&repo.index_path())?;

        // Record merge state
        fs::write(
            repo.dft_dir().join("MERGE_HEAD"),
            format!("{}\n", theirs_oid),
        )?;
        fs::write(
            repo.dft_dir().join("MERGE_MSG"),
            message.unwrap_or(&format!("Merge branch '{}'", theirs_name)),
        )?;

        Ok(MergeOutcome::Conflict {
            conflicting_files: conflicting_paths,
        })
    }
}

/// Aborts the current conflicted merge and restores the pre-merge state.
pub fn merge_abort(repo: &Repository) -> Result<(), DaftError> {
    let merge_head_path = repo.dft_dir().join("MERGE_HEAD");
    if !merge_head_path.exists() {
        return Err(DaftError::Config("There is no merge to abort".into()));
    }

    // Re-read HEAD commit tree
    let head_oid = crate::refs::peel_reference(repo.dft_dir(), "HEAD")?;
    let raw = repo.cas().read_raw(&head_oid)?;
    let commit = Commit::deserialize(&raw.data)?;

    if let Some(workdir) = repo.workdir() {
        let mut index = repo.index()?;
        checkout_tree(repo.cas().as_ref(), &commit.tree, workdir, &mut index)?;
        index.write_to(&repo.index_path())?;
    }

    let _ = fs::remove_file(merge_head_path);
    let merge_msg = repo.dft_dir().join("MERGE_MSG");
    if merge_msg.exists() {
        let _ = fs::remove_file(merge_msg);
    }

    Ok(())
}
