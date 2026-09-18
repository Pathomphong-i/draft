use super::error::GraphError;
use super::CommitAccessor;
use crate::cas::ObjectId;
use std::collections::{HashSet, VecDeque};

/// Find all lowest common ancestors (LCAs) between two commits.
/// In criss-cross merge scenarios, multiple merge bases may be returned.
pub fn all_merge_bases(
    accessor: &impl CommitAccessor,
    commit_a: &ObjectId,
    commit_b: &ObjectId,
) -> Result<Vec<ObjectId>, GraphError> {
    if commit_a == commit_b {
        return Ok(vec![*commit_a]);
    }

    // Step 1: Collect all ancestors of A (including A)
    let mut ancestors_a = HashSet::new();
    let mut queue_a = VecDeque::new();
    queue_a.push_back(*commit_a);
    ancestors_a.insert(*commit_a);

    while let Some(curr) = queue_a.pop_front() {
        let c = accessor.get_commit(&curr)?;
        for p in &c.parents {
            if ancestors_a.insert(*p) {
                queue_a.push_back(*p);
            }
        }
    }

    // Step 2: Traverse from B to identify common ancestors
    let mut common = HashSet::new();
    let mut queue_b = VecDeque::new();
    let mut visited_b = HashSet::new();
    queue_b.push_back(*commit_b);
    visited_b.insert(*commit_b);

    while let Some(curr) = queue_b.pop_front() {
        if ancestors_a.contains(&curr) {
            common.insert(curr);
        }
        let c = accessor.get_commit(&curr)?;
        for p in &c.parents {
            if visited_b.insert(*p) {
                queue_b.push_back(*p);
            }
        }
    }

    if common.is_empty() {
        return Ok(Vec::new());
    }

    // Step 3: Prune redundant ancestors (an ancestor is redundant if it is an ancestor of another common ancestor)
    let mut redundant = HashSet::new();
    for c_id in &common {
        let commit = accessor.get_commit(c_id)?;
        for parent in &commit.parents {
            let mut q = VecDeque::new();
            q.push_back(*parent);
            while let Some(p) = q.pop_front() {
                if redundant.insert(p) {
                    let pc = accessor.get_commit(&p)?;
                    for pp in &pc.parents {
                        q.push_back(*pp);
                    }
                }
            }
        }
    }

    let mut lcas: Vec<ObjectId> = common
        .into_iter()
        .filter(|c| !redundant.contains(c))
        .collect();

    // Deterministic sort: committer timestamp descending, break ties with OID
    lcas.sort_by(|x, y| {
        let cx = accessor.get_commit(x).ok();
        let cy = accessor.get_commit(y).ok();
        match (cx, cy) {
            (Some(a), Some(b)) => b
                .committer
                .time
                .cmp(&a.committer.time)
                .then_with(|| x.cmp(y)),
            _ => x.cmp(y),
        }
    });

    Ok(lcas)
}

/// Find the best lowest common ancestor between two commits.
pub fn merge_base(
    accessor: &impl CommitAccessor,
    commit_a: &ObjectId,
    commit_b: &ObjectId,
) -> Result<Option<ObjectId>, GraphError> {
    let bases = all_merge_bases(accessor, commit_a, commit_b)?;
    Ok(bases.into_iter().next())
}

/// Find a common merge base across N commits (octopus merge base).
pub fn merge_base_octopus(
    accessor: &impl CommitAccessor,
    commits: &[ObjectId],
) -> Result<Option<ObjectId>, GraphError> {
    if commits.is_empty() {
        return Ok(None);
    }
    let mut current_base = commits[0];
    for commit in &commits[1..] {
        match merge_base(accessor, &current_base, commit)? {
            Some(b) => current_base = b,
            None => return Ok(None),
        }
    }
    Ok(Some(current_base))
}
