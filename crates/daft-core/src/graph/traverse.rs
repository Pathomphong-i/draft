use super::error::GraphError;
use super::CommitAccessor;
use crate::cas::ObjectId;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};

#[derive(Eq, PartialEq)]
struct HeapItem {
    time: i64,
    oid: ObjectId,
}

impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time
            .cmp(&other.time)
            .then_with(|| self.oid.cmp(&other.oid))
    }
}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Walk reachable commits starting from `roots` in topological order (child before parent),
/// breaking ties by committer timestamp descending.
pub fn walk_commits(
    accessor: &impl CommitAccessor,
    roots: &[ObjectId],
) -> Result<Vec<ObjectId>, GraphError> {
    if roots.is_empty() {
        return Ok(Vec::new());
    }

    // Step 1: Collect all reachable commits and compute child relationships
    let mut reachable = HashSet::new();
    let mut queue = VecDeque::new();
    for root in roots {
        if reachable.insert(*root) {
            queue.push_back(*root);
        }
    }

    // in_degree[c] = number of children of c in the reachable subgraph that have not yet been emitted
    let mut in_degree: HashMap<ObjectId, usize> = HashMap::new();
    for r in &reachable {
        in_degree.entry(*r).or_insert(0);
    }

    while let Some(curr) = queue.pop_front() {
        let commit = accessor.get_commit(&curr)?;
        for p in &commit.parents {
            *in_degree.entry(*p).or_insert(0) += 1;
            if reachable.insert(*p) {
                queue.push_back(*p);
            }
        }
    }

    // Step 2: Initialize priority queue with commits having in-degree 0
    let mut heap = BinaryHeap::new();
    for (id, deg) in &in_degree {
        if *deg == 0 {
            let commit = accessor.get_commit(id)?;
            heap.push(HeapItem {
                time: commit.committer.time,
                oid: *id,
            });
        }
    }

    // Step 3: Kahn's topological sort
    let mut result = Vec::with_capacity(reachable.len());
    while let Some(item) = heap.pop() {
        result.push(item.oid);
        let commit = accessor.get_commit(&item.oid)?;
        for p in &commit.parents {
            if let Some(deg) = in_degree.get_mut(p) {
                *deg = deg.saturating_sub(1);
                if *deg == 0 {
                    let p_commit = accessor.get_commit(p)?;
                    heap.push(HeapItem {
                        time: p_commit.committer.time,
                        oid: *p,
                    });
                }
            }
        }
    }

    Ok(result)
}
