//! Reset HEAD, staging index, and working tree to a specified state.

use crate::cas::{ObjectId, ObjectType};
use crate::error::DaftError;
use crate::merge::checkout_tree;
use crate::object::Commit;
use crate::refs::ReferenceTarget;
use crate::repo::Repository;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResetMode {
    Soft,
    #[default]
    Mixed,
    Hard,
}

/// Resolves a revision specifier like "HEAD", "HEAD~1", a branch name, or an OID.
pub fn resolve_commit(repo: &Repository, rev: &str) -> Result<ObjectId, DaftError> {
    let trimmed = rev.trim();

    if trimmed == "HEAD" {
        return crate::refs::peel_reference(repo.dft_dir(), "HEAD").map_err(DaftError::Ref);
    }

    if let Some(rest) = trimmed.strip_prefix("HEAD~") {
        let count: usize = rest
            .parse()
            .map_err(|_| DaftError::Config(format!("Invalid revision specifier '{}'", rev)))?;
        let mut curr =
            crate::refs::peel_reference(repo.dft_dir(), "HEAD").map_err(DaftError::Ref)?;
        for _ in 0..count {
            let raw = repo.cas().read_raw(&curr)?;
            let commit = Commit::deserialize(&raw.data)?;
            if commit.parents.is_empty() {
                return Err(DaftError::Config(format!(
                    "Cannot resolve '{}': commit has no parents",
                    rev
                )));
            }
            curr = commit.parents[0];
        }
        return Ok(curr);
    }

    // Try branch ref
    let branch_ref = format!("refs/heads/{}", trimmed);
    if let Ok(oid) = repo.refs().resolve(&branch_ref) {
        return Ok(oid);
    }

    // Try tag ref
    let tag_ref = format!("refs/tags/{}", trimmed);
    if let Ok(oid) = repo.refs().resolve(&tag_ref) {
        // Peel tag if annotated
        let raw = repo.cas().read_raw(&oid)?;
        if raw.object_type == ObjectType::Tag {
            let tag = crate::object::Tag::deserialize(&raw.data)?;
            return Ok(tag.target);
        }
        return Ok(oid);
    }

    // Try direct ObjectId (hex)
    if let Ok(oid) = ObjectId::from_hex(trimmed) {
        if repo.cas().has_object(&oid) {
            return Ok(oid);
        }
    }

    Err(DaftError::Config(format!(
        "fatal: ambiguous argument '{}': unknown revision",
        rev
    )))
}

pub fn reset(repo: &Repository, target: &str, mode: ResetMode) -> Result<ObjectId, DaftError> {
    let target_oid = resolve_commit(repo, target)?;
    let raw = repo.cas().read_raw(&target_oid)?;
    if raw.object_type != ObjectType::Commit {
        return Err(DaftError::Config(
            "Cannot reset to non-commit object".into(),
        ));
    }
    let commit = Commit::deserialize(&raw.data)?;

    let head_ref = repo.head()?;
    match &head_ref.target {
        ReferenceTarget::Symbolic(sym) => {
            repo.refs()
                .write_ref(sym, &ReferenceTarget::Direct(target_oid), None, None)?;
        }
        ReferenceTarget::Direct(_) => {
            repo.set_head(&ReferenceTarget::Direct(target_oid))?;
        }
    }

    match mode {
        ResetMode::Soft => {
            // Do not touch index or worktree
        }
        ResetMode::Mixed => {
            // Reset index to target commit tree
            let mut tree_files = std::collections::BTreeMap::new();
            crate::diff::tree::flatten_tree(
                repo.cas().as_ref(),
                &commit.tree,
                "",
                &mut tree_files,
            )?;

            let mut new_index = crate::index::Index::new();
            for (path, (fmode, oid)) in tree_files {
                let raw_blob = repo.cas().read_raw(&oid)?;
                let entry = crate::index::IndexEntry::new(
                    path,
                    oid,
                    fmode.0,
                    crate::index::Stage::Normal,
                    raw_blob.data.len() as u32,
                )?;
                new_index.add_entry(entry);
            }
            new_index.write_to(&repo.index_path())?;
        }
        ResetMode::Hard => {
            if let Some(workdir) = repo.workdir() {
                let mut index = repo.index()?;
                checkout_tree(repo.cas().as_ref(), &commit.tree, workdir, &mut index)?;
                index.write_to(&repo.index_path())?;
            }
        }
    }

    Ok(target_oid)
}
