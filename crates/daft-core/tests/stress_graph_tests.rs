use daft_core::cas::{ObjectId, ObjectStore, RawObject};
use daft_core::graph::{
    all_merge_bases, is_ancestor, merge_base, merge_base_octopus, walk_commits, MemoryCommitGraph,
    StoreCommitGraph,
};
use daft_core::object::{Commit, Signature};
use std::collections::{HashMap, HashSet};
use tempfile::tempdir;

fn make_commit(
    parents: Vec<ObjectId>,
    time: i64,
    message: &str,
    graph: &mut MemoryCommitGraph,
) -> ObjectId {
    let tree = ObjectId::hash(message.as_bytes());
    let author = Signature::new("Challenger", "challenger@daft.test", time, 0);
    let committer = Signature::new("Challenger", "challenger@daft.test", time, 0);
    let commit = Commit::new(tree, parents, author, committer, message);
    graph.add(commit)
}

fn make_commit_in_store(
    parents: Vec<ObjectId>,
    time: i64,
    message: &str,
    store: &ObjectStore,
) -> ObjectId {
    let tree = ObjectId::hash(message.as_bytes());
    let author = Signature::new("Challenger", "challenger@daft.test", time, 0);
    let committer = Signature::new("Challenger", "challenger@daft.test", time, 0);
    let commit = Commit::new(tree, parents, author, committer, message);
    let raw = RawObject::commit(commit.serialize());
    store.write_raw(&raw).expect("write commit to store")
}

#[test]
fn test_stress_commit_graph_deep_chain_2000() {
    let mut graph = MemoryCommitGraph::new();
    const CHAIN_LEN: usize = 2000;
    let mut commits = Vec::with_capacity(CHAIN_LEN);

    let mut parent = None;
    for i in 0..CHAIN_LEN {
        let parents = match parent {
            Some(p) => vec![p],
            None => vec![],
        };
        let c = make_commit(
            parents,
            1000 + i as i64,
            &format!("commit-{}", i),
            &mut graph,
        );
        commits.push(c);
        parent = Some(c);
    }

    let root = commits[0];
    let tip = commits[CHAIN_LEN - 1];
    let mid = commits[CHAIN_LEN / 2];

    // Verify ancestry reachability without stack overflow
    assert!(is_ancestor(&graph, &root, &tip).unwrap());
    assert!(is_ancestor(&graph, &mid, &tip).unwrap());
    assert!(is_ancestor(&graph, &root, &mid).unwrap());
    assert!(!is_ancestor(&graph, &tip, &root).unwrap());
    assert!(!is_ancestor(&graph, &tip, &mid).unwrap());

    // Verify merge bases on deep linear chain
    assert_eq!(merge_base(&graph, &root, &tip).unwrap(), Some(root));
    assert_eq!(merge_base(&graph, &mid, &tip).unwrap(), Some(mid));
    assert_eq!(
        merge_base(&graph, &commits[750], &commits[1750]).unwrap(),
        Some(commits[750])
    );

    // Verify Kahn's topological sort on 2,000 nodes
    let order = walk_commits(&graph, &[tip]).unwrap();
    assert_eq!(order.len(), CHAIN_LEN);

    // Tip must be index 0, root must be index CHAIN_LEN - 1
    assert_eq!(order[0], tip);
    assert_eq!(order[CHAIN_LEN - 1], root);

    // Verify strict reverse chronological order in linear chain
    for i in 0..CHAIN_LEN {
        assert_eq!(order[i], commits[CHAIN_LEN - 1 - i]);
    }
}

#[test]
fn test_stress_commit_graph_wide_octopus_1000_parents() {
    let mut graph = MemoryCommitGraph::new();
    const NUM_BRANCHES: usize = 1000;

    let root = make_commit(vec![], 100, "root commit", &mut graph);

    let mut branches = Vec::with_capacity(NUM_BRANCHES);
    for i in 0..NUM_BRANCHES {
        let b = make_commit(
            vec![root],
            200 + (i as i64),
            &format!("branch-{}", i),
            &mut graph,
        );
        branches.push(b);
    }

    // Merge all 1,000 branches into single mega-octopus commit
    let mega_merge = make_commit(branches.clone(), 5000, "mega octopus merge", &mut graph);

    // Test merge_base_octopus across diverse branches
    let sample = vec![branches[0], branches[99], branches[499], branches[999]];
    let base = merge_base_octopus(&graph, &sample).unwrap();
    assert_eq!(base, Some(root));

    // Test walk_commits on 1002 commits
    let order = walk_commits(&graph, &[mega_merge]).unwrap();
    assert_eq!(order.len(), NUM_BRANCHES + 2);

    // Topological property: mega_merge must be at index 0, root must be at the end
    assert_eq!(order[0], mega_merge);
    assert_eq!(order[order.len() - 1], root);

    // Every branch must appear between mega_merge and root
    let root_pos = order.len() - 1;
    for b in &branches {
        let pos = order.iter().position(|x| x == b).unwrap();
        assert!(pos > 0 && pos < root_pos);
    }
}

#[test]
fn test_stress_complex_criss_cross_multibranch() {
    let mut graph = MemoryCommitGraph::new();

    // Multitier criss-cross merge:
    //          Root
    //       /  /  \  \
    //      A  B    C  D
    //     / \/ \  / \/ \
    //    M_AB M_BA M_CD M_DC
    //      \  /      \  /
    //       X          Y
    //        \        /
    //         \      /
    //       Final Merge
    let root = make_commit(vec![], 100, "root", &mut graph);

    let a = make_commit(vec![root], 200, "A", &mut graph);
    let b = make_commit(vec![root], 205, "B", &mut graph);
    let c = make_commit(vec![root], 210, "C", &mut graph);
    let d = make_commit(vec![root], 215, "D", &mut graph);

    // Tier 2: pairwise criss-cross
    let m_ab = make_commit(vec![a, b], 300, "M_AB", &mut graph);
    let m_ba = make_commit(vec![b, a], 305, "M_BA", &mut graph);
    let m_cd = make_commit(vec![c, d], 310, "M_CD", &mut graph);
    let m_dc = make_commit(vec![d, c], 315, "M_DC", &mut graph);

    // Check criss-cross LCAs for A-B pair
    let bases_ab = all_merge_bases(&graph, &m_ab, &m_ba).unwrap();
    assert_eq!(bases_ab.len(), 2);
    assert!(bases_ab.contains(&a));
    assert!(bases_ab.contains(&b));
    assert!(!bases_ab.contains(&root)); // Root pruned as redundant

    // Check criss-cross LCAs for C-D pair
    let bases_cd = all_merge_bases(&graph, &m_cd, &m_dc).unwrap();
    assert_eq!(bases_cd.len(), 2);
    assert!(bases_cd.contains(&c));
    assert!(bases_cd.contains(&d));
    assert!(!bases_cd.contains(&root));

    // Tier 3: cross-group criss-cross
    let x = make_commit(vec![m_ab, m_cd], 400, "X", &mut graph);
    let y = make_commit(vec![m_ba, m_dc], 405, "Y", &mut graph);

    let bases_xy = all_merge_bases(&graph, &x, &y).unwrap();
    // Common ancestors include A, B, C, D (and Root, which is redundant).
    // None of A, B, C, D is an ancestor of each other, so all 4 are LCAs!
    assert_eq!(bases_xy.len(), 4);
    assert!(bases_xy.contains(&a));
    assert!(bases_xy.contains(&b));
    assert!(bases_xy.contains(&c));
    assert!(bases_xy.contains(&d));
    assert!(!bases_xy.contains(&root));

    // Verify topological sort of X and Y combined
    let topo = walk_commits(&graph, &[x, y]).unwrap();
    assert_eq!(topo.len(), 11);

    // In topological sort, X and Y must precede their parents
    let pos_x = topo.iter().position(|oid| oid == &x).unwrap();
    let pos_y = topo.iter().position(|oid| oid == &y).unwrap();
    let pos_m_ab = topo.iter().position(|oid| oid == &m_ab).unwrap();
    let pos_m_ba = topo.iter().position(|oid| oid == &m_ba).unwrap();
    let pos_root = topo.iter().position(|oid| oid == &root).unwrap();

    assert!(pos_x < pos_m_ab);
    assert!(pos_y < pos_m_ba);
    assert!(pos_m_ab < pos_root);
    assert!(pos_m_ba < pos_root);
}

#[test]
fn test_stress_large_random_dag_1500_nodes() {
    let mut graph = MemoryCommitGraph::new();
    const NUM_COMMITS: usize = 1500;
    let mut commit_ids = Vec::with_capacity(NUM_COMMITS);
    let mut commit_parents: HashMap<ObjectId, Vec<ObjectId>> = HashMap::new();

    // Deterministic pseudo-random number generator (LCG)
    let mut seed: u64 = 0xDEADBEEFCAFE1234;
    let mut next_rand = || -> u64 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        seed
    };

    // Root commit
    let root = make_commit(vec![], 1000, "root", &mut graph);
    commit_ids.push(root);
    commit_parents.insert(root, vec![]);

    for i in 1..NUM_COMMITS {
        // Choose number of parents: 1, 2, or 3
        let r = next_rand();
        let num_parents = match r % 10 {
            0..=6 => 1, // 70% linear steps
            7..=8 => 2, // 20% 2-way merges
            _ => 3,     // 10% 3-way merges
        };

        // Pick parents from the recent window (up to 40 previous commits)
        let window_size = 40.min(commit_ids.len());
        let window_start = commit_ids.len() - window_size;

        let mut parents = Vec::new();
        let mut picked_indices = HashSet::new();
        for _ in 0..num_parents {
            let offset = (next_rand() as usize) % window_size;
            let p_idx = window_start + offset;
            if picked_indices.insert(p_idx) {
                parents.push(commit_ids[p_idx]);
            }
        }
        if parents.is_empty() {
            parents.push(commit_ids[commit_ids.len() - 1]);
        }

        // Committer time generally progresses but allows small jitter to test tie-breaking
        let time = 1000 + (i as i64 * 10) + ((next_rand() % 5) as i64);
        let cid = make_commit(parents.clone(), time, &format!("commit-{}", i), &mut graph);
        commit_parents.insert(cid, parents);
        commit_ids.push(cid);
    }

    assert_eq!(commit_ids.len(), NUM_COMMITS);

    // Topological sort from the last commit
    let tip = commit_ids[NUM_COMMITS - 1];
    let order = walk_commits(&graph, &[tip]).expect("walk commits on 1500 node DAG");

    // All emitted commits must satisfy topological invariant:
    // For every commit C, ALL parents of C must appear strictly AFTER C in the order.
    let mut pos_map = HashMap::new();
    for (pos, oid) in order.iter().enumerate() {
        pos_map.insert(*oid, pos);
    }

    for oid in &order {
        let parents = commit_parents.get(oid).unwrap();
        let c_pos = pos_map[oid];
        for p in parents {
            if let Some(p_pos) = pos_map.get(p) {
                assert!(
                    c_pos < *p_pos,
                    "topological violation: child {} (pos {}) must precede parent {} (pos {})",
                    oid.to_hex(),
                    c_pos,
                    p.to_hex(),
                    p_pos
                );
            }
        }
    }
}

#[test]
fn test_stress_disconnected_dag_components() {
    let mut graph = MemoryCommitGraph::new();

    // 3 disconnected components:
    // Component 1: R1 -> C1_1 -> C1_2
    // Component 2: R2 -> C2_1 -> C2_2 -> C2_merge(C2_1, C2_2)
    // Component 3: R3 -> C3_1

    let r1 = make_commit(vec![], 100, "R1", &mut graph);
    let c1_1 = make_commit(vec![r1], 110, "C1_1", &mut graph);
    let c1_2 = make_commit(vec![c1_1], 120, "C1_2", &mut graph);

    let r2 = make_commit(vec![], 200, "R2", &mut graph);
    let c2_1 = make_commit(vec![r2], 210, "C2_1", &mut graph);
    let c2_2 = make_commit(vec![r2], 215, "C2_2", &mut graph);
    let c2_merge = make_commit(vec![c2_1, c2_2], 230, "C2_merge", &mut graph);

    let r3 = make_commit(vec![], 300, "R3", &mut graph);
    let c3_1 = make_commit(vec![r3], 310, "C3_1", &mut graph);

    // Cross-component ancestry should all be false
    assert!(!is_ancestor(&graph, &r1, &c2_merge).unwrap());
    assert!(!is_ancestor(&graph, &r2, &c1_2).unwrap());
    assert!(!is_ancestor(&graph, &r3, &c1_2).unwrap());
    assert!(!is_ancestor(&graph, &c1_2, &c3_1).unwrap());

    // Cross-component merge bases should be None / empty
    assert_eq!(merge_base(&graph, &c1_2, &c2_merge).unwrap(), None);
    assert_eq!(merge_base(&graph, &c2_merge, &c3_1).unwrap(), None);
    assert!(all_merge_bases(&graph, &c1_2, &c3_1).unwrap().is_empty());
    assert_eq!(
        merge_base_octopus(&graph, &[c1_2, c2_merge, c3_1]).unwrap(),
        None
    );

    // Multi-root topological sort across all 3 components
    let order = walk_commits(&graph, &[c1_2, c2_merge, c3_1]).unwrap();
    assert_eq!(order.len(), 9); // 3 (comp1) + 4 (comp2) + 2 (comp3) = 9 commits

    // In-component topological invariant holds across all components
    let mut pos_map = HashMap::new();
    for (pos, oid) in order.iter().enumerate() {
        pos_map.insert(*oid, pos);
    }

    assert!(pos_map[&c1_2] < pos_map[&c1_1]);
    assert!(pos_map[&c1_1] < pos_map[&r1]);

    assert!(pos_map[&c2_merge] < pos_map[&c2_1]);
    assert!(pos_map[&c2_merge] < pos_map[&c2_2]);
    assert!(pos_map[&c2_1] < pos_map[&r2]);
    assert!(pos_map[&c2_2] < pos_map[&r2]);

    assert!(pos_map[&c3_1] < pos_map[&r3]);
}

#[test]
fn test_stress_graph_on_real_cas_store() {
    let dir = tempdir().expect("tempdir");
    let store = ObjectStore::init(dir.path().join("objects")).expect("init store");

    const NUM_COMMITS: usize = 200;
    let mut commits = Vec::with_capacity(NUM_COMMITS);

    let root = make_commit_in_store(vec![], 100, "root commit in CAS", &store);
    commits.push(root);

    for i in 1..NUM_COMMITS {
        let parent = commits[i - 1];
        let c = make_commit_in_store(
            vec![parent],
            100 + i as i64,
            &format!("disk commit {}", i),
            &store,
        );
        commits.push(c);
    }

    let graph_adapter = StoreCommitGraph::new(&store);

    let tip = commits[NUM_COMMITS - 1];
    let mid = commits[NUM_COMMITS / 2];

    assert!(is_ancestor(&graph_adapter, &root, &tip).unwrap());
    assert!(is_ancestor(&graph_adapter, &mid, &tip).unwrap());
    assert_eq!(merge_base(&graph_adapter, &root, &tip).unwrap(), Some(root));
    assert_eq!(merge_base(&graph_adapter, &mid, &tip).unwrap(), Some(mid));

    let order = walk_commits(&graph_adapter, &[tip]).unwrap();
    assert_eq!(order.len(), NUM_COMMITS);
    assert_eq!(order[0], tip);
    assert_eq!(order[NUM_COMMITS - 1], root);
}
