//! History replay and branch rebasing (`dft rebase`).

use super::cherry_pick::cherry_pick;
use crate::error::DaftError;
use crate::graph::{walk_commits, StoreCommitGraph};
use crate::refs::ReferenceTarget;
use crate::repo::Repository;
use crate::worktree::reset::resolve_commit;
use std::collections::HashSet;

pub fn rebase(repo: &Repository, upstream: &str) -> Result<(), DaftError> {
    let upstream_oid = resolve_commit(repo, upstream)?;
    let head_ref = repo.head()?;
    let (head_oid, branch_name) = match &head_ref.target {
        ReferenceTarget::Symbolic(sym) => {
            let oid = repo.refs().resolve(sym)?;
            (oid, Some(sym.clone()))
        }
        ReferenceTarget::Direct(oid) => (*oid, None),
    };

    if head_oid == upstream_oid {
        return Ok(());
    }

    let cas = repo.cas();
    let graph = StoreCommitGraph::new(cas.as_ref());

    let upstream_walk = walk_commits(&graph, &[upstream_oid])?;
    let upstream_set: HashSet<_> = upstream_walk.into_iter().collect();

    let head_walk = walk_commits(&graph, &[head_oid])?;
    let mut todo_commits: Vec<_> = head_walk
        .into_iter()
        .filter(|c| !upstream_set.contains(c))
        .collect();

    if todo_commits.is_empty() {
        return Ok(());
    }

    // Oldest to newest
    todo_commits.reverse();

    // Check out upstream in detached HEAD
    let engine = crate::checkout::CheckoutEngine::new(repo);
    engine.checkout_commit(&upstream_oid.to_hex())?;

    for c in todo_commits {
        cherry_pick(repo, &c.to_hex())?;
    }

    // If rebase was on a branch, fast-forward branch to current HEAD
    let new_head_oid =
        crate::refs::peel_reference(repo.dft_dir(), "HEAD").map_err(DaftError::Ref)?;

    if let Some(branch_ref) = branch_name {
        repo.refs().write_ref(
            &branch_ref,
            &ReferenceTarget::Direct(new_head_oid),
            None,
            None,
        )?;
        repo.set_head(&ReferenceTarget::Symbolic(branch_ref))?;
    }

    Ok(())
}
