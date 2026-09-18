use super::error::GraphError;
use super::CommitAccessor;
use crate::cas::ObjectId;
use std::collections::{HashSet, VecDeque};

/// Checks if `ancestor` is reachable from `descendant` by following parent pointers.
/// Properties:
/// - is_ancestor(C, C) == true (reflexive)
/// - Transitive: if A -> B -> C, is_ancestor(A, C) == true
pub fn is_ancestor(
    accessor: &impl CommitAccessor,
    ancestor: &ObjectId,
    descendant: &ObjectId,
) -> Result<bool, GraphError> {
    if ancestor == descendant {
        return Ok(true);
    }
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    visited.insert(*descendant);
    queue.push_back(*descendant);

    while let Some(curr_id) = queue.pop_front() {
        if curr_id == *ancestor {
            return Ok(true);
        }
        let commit = accessor.get_commit(&curr_id)?;
        for parent_id in &commit.parents {
            if visited.insert(*parent_id) {
                queue.push_back(*parent_id);
            }
        }
    }
    Ok(false)
}
