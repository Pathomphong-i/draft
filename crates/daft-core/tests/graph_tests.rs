use daft_core::cas::ObjectId;
use daft_core::graph::{
    all_merge_bases, is_ancestor, merge_base, merge_base_octopus, walk_commits, MemoryCommitGraph,
};
use daft_core::object::{Commit, Signature};

fn make_commit(
    parents: Vec<ObjectId>,
    time: i64,
    message: &str,
    graph: &mut MemoryCommitGraph,
) -> ObjectId {
    let tree = ObjectId::hash(message.as_bytes());
    let author = Signature::new("Test", "test@example.com", time, 0);
    let committer = Signature::new("Test", "test@example.com", time, 0);
    let commit = Commit::new(tree, parents, author, committer, message);
    graph.add(commit)
}

#[test]
fn test_is_ancestor_and_merge_base_linear() {
    let mut graph = MemoryCommitGraph::new();
    let c1 = make_commit(vec![], 100, "initial commit", &mut graph);
    let c2 = make_commit(vec![c1], 200, "second commit", &mut graph);
    let c3 = make_commit(vec![c2], 300, "third commit", &mut graph);

    assert!(is_ancestor(&graph, &c1, &c3).unwrap());
    assert!(is_ancestor(&graph, &c2, &c3).unwrap());
    assert!(is_ancestor(&graph, &c3, &c3).unwrap());
    assert!(!is_ancestor(&graph, &c3, &c1).unwrap());

    assert_eq!(merge_base(&graph, &c1, &c3).unwrap(), Some(c1));
    assert_eq!(merge_base(&graph, &c2, &c3).unwrap(), Some(c2));
}

#[test]
fn test_merge_base_diamond() {
    let mut graph = MemoryCommitGraph::new();
    let root = make_commit(vec![], 100, "root", &mut graph);
    let branch_a = make_commit(vec![root], 200, "branch A", &mut graph);
    let branch_b = make_commit(vec![root], 210, "branch B", &mut graph);

    assert_eq!(
        merge_base(&graph, &branch_a, &branch_b).unwrap(),
        Some(root)
    );

    let merge = make_commit(vec![branch_a, branch_b], 300, "merge A and B", &mut graph);
    assert!(is_ancestor(&graph, &root, &merge).unwrap());
    assert!(is_ancestor(&graph, &branch_a, &merge).unwrap());
    assert!(is_ancestor(&graph, &branch_b, &merge).unwrap());
}

#[test]
fn test_merge_base_criss_cross() {
    // Criss-cross merge:
    //      R
    //     / \
    //    A   B
    //    | X |
    //    C   D
    // C has parents [A, B], D has parents [B, A]
    // Both A and B are minimal common ancestors of C and D!
    let mut graph = MemoryCommitGraph::new();
    let r = make_commit(vec![], 100, "root", &mut graph);
    let a = make_commit(vec![r], 200, "A", &mut graph);
    let b = make_commit(vec![r], 210, "B", &mut graph);

    let c = make_commit(vec![a, b], 300, "C", &mut graph);
    let d = make_commit(vec![b, a], 310, "D", &mut graph);

    let bases = all_merge_bases(&graph, &c, &d).unwrap();
    assert_eq!(bases.len(), 2);
    assert!(bases.contains(&a));
    assert!(bases.contains(&b));
    assert!(!bases.contains(&r)); // r is redundant because a and b are both descendants of r
}

#[test]
fn test_merge_base_orphan() {
    let mut graph = MemoryCommitGraph::new();
    let r1 = make_commit(vec![], 100, "root 1", &mut graph);
    let r2 = make_commit(vec![], 110, "root 2 (orphan)", &mut graph);

    let bases = all_merge_bases(&graph, &r1, &r2).unwrap();
    assert!(bases.is_empty());
    assert_eq!(merge_base(&graph, &r1, &r2).unwrap(), None);
}

#[test]
fn test_merge_base_octopus() {
    let mut graph = MemoryCommitGraph::new();
    let root = make_commit(vec![], 100, "root", &mut graph);
    let b1 = make_commit(vec![root], 200, "branch 1", &mut graph);
    let b2 = make_commit(vec![root], 210, "branch 2", &mut graph);
    let b3 = make_commit(vec![root], 220, "branch 3", &mut graph);

    let base = merge_base_octopus(&graph, &[b1, b2, b3]).unwrap();
    assert_eq!(base, Some(root));
}

#[test]
fn test_topo_sort_order() {
    let mut graph = MemoryCommitGraph::new();
    let r = make_commit(vec![], 100, "root", &mut graph);
    let b1 = make_commit(vec![r], 200, "b1", &mut graph);
    let b2 = make_commit(vec![r], 210, "b2", &mut graph);
    let m = make_commit(vec![b1, b2], 300, "merge", &mut graph);

    let order = walk_commits(&graph, &[m]).unwrap();
    assert_eq!(order.len(), 4);

    // In topological order (child before parent):
    // m must come before b1 and b2
    let pos_m = order.iter().position(|&x| x == m).unwrap();
    let pos_b1 = order.iter().position(|&x| x == b1).unwrap();
    let pos_b2 = order.iter().position(|&x| x == b2).unwrap();
    let pos_r = order.iter().position(|&x| x == r).unwrap();

    assert!(pos_m < pos_b1);
    assert!(pos_m < pos_b2);
    assert!(pos_b1 < pos_r);
    assert!(pos_b2 < pos_r);
}
