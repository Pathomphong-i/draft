//! Stash engine: saves dirty working tree and index state into stash stack.

use super::add::add_paths;
use super::reset::{reset, ResetMode};
use super::status::get_status;
use crate::cas::{ObjectId, ObjectType, RawObject};
use crate::error::DaftError;
use crate::merge::{checkout_tree, get_signature};
use crate::object::Commit;
use crate::repo::Repository;
use std::fs;
use std::path::PathBuf;

fn stash_ref_path(repo: &Repository) -> PathBuf {
    repo.dft_dir().join("refs").join("stash")
}

fn stash_log_path(repo: &Repository) -> PathBuf {
    repo.dft_dir().join("logs").join("refs").join("stash")
}

pub fn stash_push(repo: &Repository, message: Option<&str>) -> Result<Option<ObjectId>, DaftError> {
    let status = get_status(repo)?;
    if status.staged_added.is_empty()
        && status.staged_modified.is_empty()
        && status.staged_deleted.is_empty()
        && status.unstaged_modified.is_empty()
        && status.unstaged_deleted.is_empty()
        && status.untracked.is_empty()
    {
        return Ok(None);
    }

    let head_oid = crate::refs::peel_reference(repo.dft_dir(), "HEAD").map_err(DaftError::Ref)?;

    let sig = get_signature();
    let branch_name = &status.branch;
    let desc = message.unwrap_or("WIP");
    let commit_msg = format!("WIP on {}: {}", branch_name, desc);

    // 1. Stage all dirty changes temporarily to build worktree tree
    add_paths(repo, &[PathBuf::from(".")], true, false)?;
    let dirty_index = repo.index()?;

    // 2. Build tree from dirty index
    let mut tree_files = std::collections::BTreeMap::new();
    for entry in dirty_index.entries() {
        tree_files.insert(
            entry.path.clone(),
            (crate::object::FileMode(entry.mode), entry.oid),
        );
    }
    let worktree_tree_oid =
        crate::merge::tree_merge::build_hierarchical_tree(repo.cas().as_ref(), &tree_files)?;

    // 3. Create stash commit
    let stash_commit = Commit::new(
        worktree_tree_oid,
        vec![head_oid],
        sig.clone(),
        sig,
        &commit_msg,
    );
    let serialized = stash_commit.serialize();
    let raw = RawObject::new(ObjectType::Commit, serialized);
    let stash_oid = repo.cas().write_raw(&raw)?;

    // 4. Record into stash ref and reflog
    let ref_path = stash_ref_path(repo);
    if let Some(parent) = ref_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&ref_path, format!("{}\n", stash_oid))?;

    let log_path = stash_log_path(repo);
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let log_line = format!(
        "0000000000000000000000000000000000000000000000000000000000000000 {} Draft User <user@draft-vcs.org> 0 +0000\t{}\n",
        stash_oid, commit_msg
    );
    let mut existing_log = if log_path.exists() {
        fs::read_to_string(&log_path)?
    } else {
        String::new()
    };
    existing_log.insert_str(0, &log_line);
    fs::write(&log_path, existing_log)?;

    // 5. Restore original index or reset worktree hard to HEAD
    reset(repo, "HEAD", ResetMode::Hard)?;
    if let Some(workdir) = repo.workdir() {
        for untracked_file in &status.untracked {
            let p = workdir.join(untracked_file);
            if p.exists() {
                let _ = fs::remove_file(p);
            }
        }
    }

    Ok(Some(stash_oid))
}

pub fn stash_list(repo: &Repository) -> Result<Vec<String>, DaftError> {
    let log_path = stash_log_path(repo);
    if !log_path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&log_path)?;
    let mut items = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        if let Some((_, msg)) = line.split_once('\t') {
            items.push(format!("stash@{}: {}", idx, msg));
        }
    }
    Ok(items)
}

pub fn stash_apply(repo: &Repository, index: usize) -> Result<(), DaftError> {
    let log_path = stash_log_path(repo);
    if !log_path.exists() {
        return Err(DaftError::Config("No stash found".into()));
    }
    let content = fs::read_to_string(&log_path)?;
    let line = content
        .lines()
        .nth(index)
        .ok_or_else(|| DaftError::Config(format!("No stash entry at index {}", index)))?;

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(DaftError::Config("Corrupt stash log".into()));
    }
    let stash_oid = ObjectId::from_hex(parts[1])
        .map_err(|_| DaftError::Config("Invalid stash commit OID".into()))?;

    let raw = repo.cas().read_raw(&stash_oid)?;
    let commit = Commit::deserialize(&raw.data)?;

    let workdir = repo
        .workdir()
        .ok_or_else(|| DaftError::Config("Cannot apply stash in a bare repository".into()))?;
    let mut current_index = repo.index()?;

    checkout_tree(
        repo.cas().as_ref(),
        &commit.tree,
        workdir,
        &mut current_index,
    )?;
    current_index.write_to(&repo.index_path())?;

    Ok(())
}

pub fn stash_drop(repo: &Repository, index: usize) -> Result<(), DaftError> {
    let log_path = stash_log_path(repo);
    if !log_path.exists() {
        return Err(DaftError::Config("No stash found".into()));
    }
    let content = fs::read_to_string(&log_path)?;
    let mut lines: Vec<&str> = content.lines().collect();
    if index >= lines.len() {
        return Err(DaftError::Config(format!(
            "No stash entry at index {}",
            index
        )));
    }
    lines.remove(index);

    if lines.is_empty() {
        stash_clear(repo)?;
    } else {
        let new_content = lines.join("\n") + "\n";
        fs::write(&log_path, new_content)?;

        // Update refs/stash to top entry
        let top_parts: Vec<&str> = lines[0].split_whitespace().collect();
        let ref_path = stash_ref_path(repo);
        fs::write(&ref_path, format!("{}\n", top_parts[1]))?;
    }

    Ok(())
}

pub fn stash_pop(repo: &Repository, index: usize) -> Result<(), DaftError> {
    stash_apply(repo, index)?;
    stash_drop(repo, index)?;
    Ok(())
}

pub fn stash_clear(repo: &Repository) -> Result<(), DaftError> {
    let ref_path = stash_ref_path(repo);
    if ref_path.exists() {
        let _ = fs::remove_file(ref_path);
    }
    let log_path = stash_log_path(repo);
    if log_path.exists() {
        let _ = fs::remove_file(log_path);
    }
    Ok(())
}
