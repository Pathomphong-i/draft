//! Nearest reachable tag descriptor (`dft describe`).

use crate::cas::ObjectId;
use crate::error::DaftError;
use crate::object::Commit;
use crate::repo::Repository;
use crate::worktree::reset::resolve_commit;
use std::collections::{HashMap, HashSet, VecDeque};

pub fn describe(
    repo: &Repository,
    rev: Option<&str>,
    _tags: bool,
    always: bool,
) -> Result<String, DaftError> {
    let target_oid = if let Some(r) = rev {
        resolve_commit(repo, r)?
    } else {
        crate::refs::peel_reference(repo.dft_dir(), "HEAD").map_err(DaftError::Ref)?
    };

    // Collect all tag references and resolve them to commit OIDs
    let tag_manager = crate::tag::TagManager::new(repo);
    let tag_names = tag_manager.list()?;
    let mut commit_to_tags: HashMap<ObjectId, Vec<String>> = HashMap::new();

    for t_name in tag_names {
        let tag_ref = format!("refs/tags/{}", t_name);
        if let Ok(target) = repo.refs().resolve(&tag_ref) {
            let commit_oid = tag_manager.peel_to_commit(target).unwrap_or(target);
            commit_to_tags.entry(commit_oid).or_default().push(t_name);
        }
    }

    // Direct match
    if let Some(tags) = commit_to_tags.get(&target_oid) {
        return Ok(tags[0].clone());
    }

    // BFS for nearest tag
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    queue.push_back((target_oid, 0usize));
    visited.insert(target_oid);

    while let Some((curr, dist)) = queue.pop_front() {
        if dist > 0 {
            if let Some(tags) = commit_to_tags.get(&curr) {
                return Ok(format!(
                    "{}-{}-g{}",
                    tags[0],
                    dist,
                    &target_oid.to_hex()[..8]
                ));
            }
        }

        if let Ok(raw) = repo.cas().read_raw(&curr) {
            if let Ok(commit) = Commit::deserialize(&raw.data) {
                for parent in commit.parents {
                    if visited.insert(parent) {
                        queue.push_back((parent, dist + 1));
                    }
                }
            }
        }
    }

    if always {
        Ok(target_oid.to_hex()[..8].to_string())
    } else {
        Err(DaftError::Config(
            "fatal: No names found, cannot describe anything.".into(),
        ))
    }
}
