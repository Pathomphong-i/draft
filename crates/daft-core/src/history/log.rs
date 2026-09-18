//! Commit history traversal and formatting.

use crate::cas::{ObjectId, ObjectType};
use crate::error::DaftError;
use crate::graph::{walk_commits, StoreCommitGraph};
use crate::object::{Commit, Signature};
use crate::repo::Repository;
use crate::worktree::reset::resolve_commit;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub oid: ObjectId,
    pub parents: Vec<ObjectId>,
    pub author: Signature,
    pub committer: Signature,
    pub message: String,
}

/// Walks commit history from a revision or range.
pub fn get_log(
    repo: &Repository,
    revision_range: Option<&str>,
    max_count: Option<usize>,
    reverse: bool,
) -> Result<Vec<LogEntry>, DaftError> {
    let cas = repo.cas();
    let graph = StoreCommitGraph::new(cas.as_ref());

    let (start_oid, exclude_oid) = if let Some(range) = revision_range {
        if let Some((left, right)) = range.split_once("..") {
            let left_oid = resolve_commit(repo, left)?;
            let right_oid = resolve_commit(repo, right)?;
            (right_oid, Some(left_oid))
        } else {
            (resolve_commit(repo, range)?, None)
        }
    } else {
        match crate::refs::peel_reference(repo.dft_dir(), "HEAD") {
            Ok(oid) => (oid, None),
            Err(_) => return Ok(Vec::new()), // Empty repo / unborn branch
        }
    };

    let mut excluded = HashSet::new();
    if let Some(exc) = exclude_oid {
        let exc_walk = walk_commits(&graph, &[exc])?;
        for c in exc_walk {
            excluded.insert(c);
        }
    }

    let oids = walk_commits(&graph, &[start_oid])?;
    let mut entries = Vec::new();

    for oid in oids {
        if excluded.contains(&oid) {
            continue;
        }

        let raw = cas.read_raw(&oid)?;
        if raw.object_type == ObjectType::Commit {
            let commit = Commit::deserialize(&raw.data)?;
            entries.push(LogEntry {
                oid,
                parents: commit.parents,
                author: commit.author,
                committer: commit.committer,
                message: commit.message,
            });
        }

        if let Some(max) = max_count {
            if entries.len() >= max {
                break;
            }
        }
    }

    if reverse {
        entries.reverse();
    }

    Ok(entries)
}
