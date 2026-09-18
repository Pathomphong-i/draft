use daft_awareness::entropy::EntropySubsystem;
use daft_awareness::foresee::{ConflictType, ForeseeSubsystem};
use daft_awareness::territory::glob::{matches_glob, patterns_overlap};
use daft_awareness::territory::TerritoryManager;
use daft_core::cas::{ObjectType, RawObject};
use daft_core::init::{init, InitOptions};
use daft_core::object::{Commit, FileMode, Signature, Tree, TreeEntry};
use daft_core::refs::ReferenceTarget;
use daft_core::Repository;
use daft_dimension::{DimensionManager, DimensionRepository};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::tempdir;

fn helper_create_commit(
    repo: &Repository,
    parent: Option<daft_core::cas::ObjectId>,
    files: &[(&str, &[u8])],
    msg: &str,
) -> daft_core::cas::ObjectId {
    let mut tree_entries = Vec::new();
    for (path, content) in files {
        let raw_blob = RawObject::new(ObjectType::Blob, content.to_vec());
        let blob_oid = repo.cas().write_raw(&raw_blob).unwrap();
        tree_entries.push(TreeEntry {
            mode: FileMode::REGULAR,
            name: path.to_string(),
            oid: blob_oid,
        });
    }
    let tree = Tree::from_entries(tree_entries).unwrap();
    let tree_bytes = tree.serialize();
    let raw_tree = RawObject::new(ObjectType::Tree, tree_bytes);
    let tree_oid = repo.cas().write_raw(&raw_tree).unwrap();

    let sig = Signature::now("Test User", "test@example.com");
    let commit = Commit::new(
        tree_oid,
        parent.into_iter().collect(),
        sig.clone(),
        sig,
        msg.to_string(),
    );
    let commit_bytes = commit.serialize();
    let raw_commit = RawObject::new(ObjectType::Commit, commit_bytes);
    repo.cas().write_raw(&raw_commit).unwrap()
}

// ============================================================================
// 1. Foresee Stress Tests
// ============================================================================

#[test]
fn test_foresee_3way_line_collision_vs_clean_edits() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let base_lines = (1..=10)
        .map(|i| format!("line {}\n", i))
        .collect::<String>();

    let base_commit = helper_create_commit(
        &repo,
        None,
        &[("common.txt", base_lines.as_bytes())],
        "LCA base commit",
    );

    // Update mainline HEAD to base_commit
    repo.refs()
        .write_ref(
            "refs/heads/mainline",
            &ReferenceTarget::Direct(base_commit),
            None,
            None,
        )
        .unwrap();

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();

    // Create dim1 and dim2 branching from mainline base_commit
    dim_mgr
        .create_dimension("dim1", Some(&base_commit.to_hex()), None)
        .unwrap();
    dim_mgr
        .create_dimension("dim2", Some(&base_commit.to_hex()), None)
        .unwrap();

    // Verify initial state: identical dimensions have 0 conflicts
    let foresee = ForeseeSubsystem::new(Arc::clone(&repo));
    let initial_report = foresee.predict("dim1", "dim2").unwrap();
    assert!(!initial_report.has_conflicts);
    assert_eq!(initial_report.conflict_count, 0);
    assert_eq!(initial_report.clean_file_count, 1);

    // Scenario A: Clean non-overlapping replacements
    // dim1 edits line 2
    let dim1_lines = (1..=10)
        .map(|i| {
            if i == 2 {
                "line 2 modified by dim1\n".to_string()
            } else {
                format!("line {}\n", i)
            }
        })
        .collect::<String>();
    let dim1_commit = helper_create_commit(
        &repo,
        Some(base_commit),
        &[("common.txt", dim1_lines.as_bytes())],
        "dim1 edit line 2",
    );
    let dim1_repo = DimensionRepository::for_dimension(Arc::clone(&repo), "dim1").unwrap();
    dim1_repo.set_head(&dim1_commit.to_hex()).unwrap();

    // dim2 edits line 8
    let dim2_lines = (1..=10)
        .map(|i| {
            if i == 8 {
                "line 8 modified by dim2\n".to_string()
            } else {
                format!("line {}\n", i)
            }
        })
        .collect::<String>();
    let dim2_commit = helper_create_commit(
        &repo,
        Some(base_commit),
        &[("common.txt", dim2_lines.as_bytes())],
        "dim2 edit line 8",
    );
    let dim2_repo = DimensionRepository::for_dimension(Arc::clone(&repo), "dim2").unwrap();
    dim2_repo.set_head(&dim2_commit.to_hex()).unwrap();

    // Foresee prediction for clean non-overlapping edits
    let clean_report = foresee.predict("dim1", "dim2").unwrap();
    assert!(
        !clean_report.has_conflicts,
        "Clean non-overlapping edits must not have conflicts"
    );
    assert_eq!(clean_report.conflict_count, 0);
    assert_eq!(clean_report.clean_file_count, 1);
    assert_eq!(clean_report.severity_score, 0.0);

    // Verify non-destructive invariant: dim1 and dim2 HEAD commits unchanged
    let dim1_head_after = fs::read_to_string(dim1_repo.head_path()).unwrap();
    assert_eq!(dim1_head_after.trim(), dim1_commit.to_hex());

    // Scenario B: Direct 3-way line collision on line 5
    let dim1_collision_lines = (1..=10)
        .map(|i| {
            if i == 5 {
                "line 5 COLLISION DIM1\n".to_string()
            } else {
                format!("line {}\n", i)
            }
        })
        .collect::<String>();
    let dim1_collision_commit = helper_create_commit(
        &repo,
        Some(base_commit),
        &[("common.txt", dim1_collision_lines.as_bytes())],
        "dim1 edit line 5",
    );
    dim1_repo.set_head(&dim1_collision_commit.to_hex()).unwrap();

    let dim2_collision_lines = (1..=10)
        .map(|i| {
            if i == 5 {
                "line 5 COLLISION DIM2\n".to_string()
            } else {
                format!("line {}\n", i)
            }
        })
        .collect::<String>();
    let dim2_collision_commit = helper_create_commit(
        &repo,
        Some(base_commit),
        &[("common.txt", dim2_collision_lines.as_bytes())],
        "dim2 edit line 5",
    );
    dim2_repo.set_head(&dim2_collision_commit.to_hex()).unwrap();

    let collision_report = foresee.predict("dim1", "dim2").unwrap();
    assert!(
        collision_report.has_conflicts,
        "Synthetic 3-way line collision on line 5 must be flagged as conflict"
    );
    assert_eq!(collision_report.conflict_count, 1);
    assert_eq!(
        collision_report.conflicting_files,
        vec![PathBuf::from("common.txt")]
    );
    assert_eq!(
        collision_report.conflict_hunks[0].conflict_type,
        ConflictType::TextOverlap
    );
    assert_eq!(
        collision_report.conflict_hunks[0].ours_lines,
        vec!["line 5 COLLISION DIM1".to_string()]
    );
    assert_eq!(
        collision_report.conflict_hunks[0].theirs_lines,
        vec!["line 5 COLLISION DIM2".to_string()]
    );
    assert!(collision_report.severity_score >= 0.60);
}

#[test]
fn test_foresee_uncommitted_workspace_edits_without_commits() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let base_lines = (1..=10)
        .map(|i| format!("item {}\n", i))
        .collect::<String>();

    let base_commit = helper_create_commit(
        &repo,
        None,
        &[("data.txt", base_lines.as_bytes())],
        "Base commit",
    );
    repo.refs()
        .write_ref(
            "refs/heads/mainline",
            &ReferenceTarget::Direct(base_commit),
            None,
            None,
        )
        .unwrap();

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();
    dim_mgr
        .create_dimension("dim-uncommitted-1", Some(&base_commit.to_hex()), None)
        .unwrap();
    dim_mgr
        .create_dimension("dim-uncommitted-2", Some(&base_commit.to_hex()), None)
        .unwrap();

    // Write UNCOMMITTED changes directly into workspace trees
    let ws1 = dir
        .path()
        .join(".dft/dimensions/dim-uncommitted-1/workspace");
    let ws2 = dir
        .path()
        .join(".dft/dimensions/dim-uncommitted-2/workspace");
    fs::create_dir_all(&ws1).unwrap();
    fs::create_dir_all(&ws2).unwrap();

    // Dirty edit: collision on line 4
    let mut lines1 = (1..=10)
        .map(|i| format!("item {}\n", i))
        .collect::<Vec<_>>();
    lines1[3] = "item 4 UNCOMMITTED BY DIM 1\n".to_string();
    fs::write(ws1.join("data.txt"), lines1.concat()).unwrap();

    let mut lines2 = (1..=10)
        .map(|i| format!("item {}\n", i))
        .collect::<Vec<_>>();
    lines2[3] = "item 4 UNCOMMITTED BY DIM 2\n".to_string();
    fs::write(ws2.join("data.txt"), lines2.concat()).unwrap();

    let foresee = ForeseeSubsystem::new(Arc::clone(&repo));
    let report = foresee
        .predict("dim-uncommitted-1", "dim-uncommitted-2")
        .unwrap();

    assert!(
        report.has_conflicts,
        "Foresee must detect 3-way conflicts in uncommitted workspace files"
    );
    assert_eq!(report.conflict_count, 1);
    assert_eq!(report.conflicting_files, vec![PathBuf::from("data.txt")]);

    // Verify neither worktree was modified by foresee
    assert_eq!(
        fs::read_to_string(ws1.join("data.txt")).unwrap(),
        lines1.concat()
    );
    assert_eq!(
        fs::read_to_string(ws2.join("data.txt")).unwrap(),
        lines2.concat()
    );
}

#[test]
fn test_foresee_pure_insertions_disjoint_lines_bug() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let base_lines = (1..=10)
        .map(|i| format!("line {}\n", i))
        .collect::<String>();

    let base_commit = helper_create_commit(
        &repo,
        None,
        &[("insert.txt", base_lines.as_bytes())],
        "Base commit",
    );
    repo.refs()
        .write_ref(
            "refs/heads/mainline",
            &ReferenceTarget::Direct(base_commit),
            None,
            None,
        )
        .unwrap();

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();
    dim_mgr
        .create_dimension("dim-ins-1", Some(&base_commit.to_hex()), None)
        .unwrap();
    dim_mgr
        .create_dimension("dim-ins-2", Some(&base_commit.to_hex()), None)
        .unwrap();

    // dim-ins-1 inserts a line after line 2
    let mut ins1 = Vec::new();
    for i in 1..=10 {
        ins1.push(format!("line {}\n", i));
        if i == 2 {
            ins1.push("line 2.5 inserted by dim 1\n".to_string());
        }
    }
    let c1 = helper_create_commit(
        &repo,
        Some(base_commit),
        &[("insert.txt", ins1.concat().as_bytes())],
        "insert at line 2",
    );
    let dim1_repo = DimensionRepository::for_dimension(Arc::clone(&repo), "dim-ins-1").unwrap();
    dim1_repo.set_head(&c1.to_hex()).unwrap();

    // dim-ins-2 inserts a line after line 8
    let mut ins2 = Vec::new();
    for i in 1..=10 {
        ins2.push(format!("line {}\n", i));
        if i == 8 {
            ins2.push("line 8.5 inserted by dim 2\n".to_string());
        }
    }
    let c2 = helper_create_commit(
        &repo,
        Some(base_commit),
        &[("insert.txt", ins2.concat().as_bytes())],
        "insert at line 8",
    );
    let dim2_repo = DimensionRepository::for_dimension(Arc::clone(&repo), "dim-ins-2").unwrap();
    dim2_repo.set_head(&c2.to_hex()).unwrap();

    let foresee = ForeseeSubsystem::new(Arc::clone(&repo));
    let report = foresee.predict("dim-ins-1", "dim-ins-2").unwrap();

    // EMPIRICAL BUG REPRODUCTION:
    // Disjoint insertions after line 2 vs line 8 are erroneously reported as conflict on line 1..2!
    if report.has_conflicts {
        eprintln!(
            "REPRODUCED FORESEE BUG: Disjoint line insertions falsely collided: {:?}",
            report.conflict_hunks
        );
    }
    assert!(
        !report.has_conflicts,
        "Specification violation: Disjoint insertions (line 2 vs line 8) must NOT be reported as conflicts!"
    );
    assert_eq!(report.conflict_count, 0);

    // Adversarial Challenge A: Same-line insertion with conflicting text MUST be flagged
    dim_mgr
        .create_dimension("dim-ins-c1", Some(&base_commit.to_hex()), None)
        .unwrap();
    dim_mgr
        .create_dimension("dim-ins-c2", Some(&base_commit.to_hex()), None)
        .unwrap();

    let mut ins_c1 = Vec::new();
    let mut ins_c2 = Vec::new();
    for i in 1..=10 {
        ins_c1.push(format!("line {}\n", i));
        ins_c2.push(format!("line {}\n", i));
        if i == 2 {
            ins_c1.push("line 2.5 conflicting version Alpha\n".to_string());
            ins_c2.push("line 2.5 conflicting version Beta\n".to_string());
        }
    }
    let cc1 = helper_create_commit(
        &repo,
        Some(base_commit),
        &[("insert.txt", ins_c1.concat().as_bytes())],
        "conflicting insert at line 2 alpha",
    );
    let cc2 = helper_create_commit(
        &repo,
        Some(base_commit),
        &[("insert.txt", ins_c2.concat().as_bytes())],
        "conflicting insert at line 2 beta",
    );
    let dim_c1_repo = DimensionRepository::for_dimension(Arc::clone(&repo), "dim-ins-c1").unwrap();
    dim_c1_repo.set_head(&cc1.to_hex()).unwrap();
    let dim_c2_repo = DimensionRepository::for_dimension(Arc::clone(&repo), "dim-ins-c2").unwrap();
    dim_c2_repo.set_head(&cc2.to_hex()).unwrap();

    let conflict_report = foresee.predict("dim-ins-c1", "dim-ins-c2").unwrap();
    assert!(
        conflict_report.has_conflicts,
        "Same-line insertions with divergent text MUST produce a conflict!"
    );
    assert_eq!(conflict_report.conflict_count, 1);
    assert_eq!(
        conflict_report.conflict_hunks[0].conflict_type,
        ConflictType::TextOverlap
    );

    // Adversarial Challenge B: Same-line insertion with IDENTICAL text converges cleanly
    dim_mgr
        .create_dimension("dim-ins-id1", Some(&base_commit.to_hex()), None)
        .unwrap();
    dim_mgr
        .create_dimension("dim-ins-id2", Some(&base_commit.to_hex()), None)
        .unwrap();

    let id_c = helper_create_commit(
        &repo,
        Some(base_commit),
        &[("insert.txt", ins_c1.concat().as_bytes())],
        "identical insert at line 2",
    );
    let dim_id1_repo =
        DimensionRepository::for_dimension(Arc::clone(&repo), "dim-ins-id1").unwrap();
    dim_id1_repo.set_head(&id_c.to_hex()).unwrap();
    let dim_id2_repo =
        DimensionRepository::for_dimension(Arc::clone(&repo), "dim-ins-id2").unwrap();
    dim_id2_repo.set_head(&id_c.to_hex()).unwrap();

    let id_report = foresee.predict("dim-ins-id1", "dim-ins-id2").unwrap();
    assert!(
        !id_report.has_conflicts,
        "Same-line insertions with identical text must NOT conflict!"
    );

    // Adversarial Challenge C: Boundary insertions (start of file line 0 vs end of file line 10)
    dim_mgr
        .create_dimension("dim-ins-head", Some(&base_commit.to_hex()), None)
        .unwrap();
    dim_mgr
        .create_dimension("dim-ins-tail", Some(&base_commit.to_hex()), None)
        .unwrap();

    let head_text = format!("line 0.0 header\n{}", base_lines);
    let tail_text = format!("{}line 10.5 footer\n", base_lines);

    let head_c = helper_create_commit(
        &repo,
        Some(base_commit),
        &[("insert.txt", head_text.as_bytes())],
        "head insert",
    );
    let tail_c = helper_create_commit(
        &repo,
        Some(base_commit),
        &[("insert.txt", tail_text.as_bytes())],
        "tail insert",
    );
    let dim_head_repo =
        DimensionRepository::for_dimension(Arc::clone(&repo), "dim-ins-head").unwrap();
    dim_head_repo.set_head(&head_c.to_hex()).unwrap();
    let dim_tail_repo =
        DimensionRepository::for_dimension(Arc::clone(&repo), "dim-ins-tail").unwrap();
    dim_tail_repo.set_head(&tail_c.to_hex()).unwrap();

    let boundary_report = foresee.predict("dim-ins-head", "dim-ins-tail").unwrap();
    assert!(
        !boundary_report.has_conflicts,
        "Extreme boundary insertions (header vs footer) must NOT conflict!"
    );
    assert_eq!(boundary_report.conflict_count, 0);
}

// ============================================================================
// 2. Entropy Mathematical Bounds Stress Tests
// ============================================================================

#[test]
fn test_entropy_mathematical_bounds_committed() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let base_lines = "alpha\nbeta\ngamma\ndelta\nepsilon\n";
    let base_commit = helper_create_commit(
        &repo,
        None,
        &[("file.txt", base_lines.as_bytes())],
        "LCA base commit",
    );
    repo.refs()
        .write_ref(
            "refs/heads/mainline",
            &ReferenceTarget::Direct(base_commit),
            None,
            None,
        )
        .unwrap();

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();
    dim_mgr
        .create_dimension("dim-a", Some(&base_commit.to_hex()), None)
        .unwrap();
    dim_mgr
        .create_dimension("dim-b", Some(&base_commit.to_hex()), None)
        .unwrap();

    let entropy_sys = EntropySubsystem::new(Arc::clone(&repo));

    // Bound 1: Identical committed dimension states == 0.0
    let id_metrics = entropy_sys.calculate("dim-a", "dim-b").unwrap();
    assert_eq!(
        id_metrics.total_entropy, 0.0,
        "Identical committed dimension states must have total_entropy = 0.0, got {}",
        id_metrics.total_entropy
    );

    // Bound 2: Clean disjoint edits in [0.0, 0.20]
    let c_a = helper_create_commit(
        &repo,
        Some(base_commit),
        &[
            ("file.txt", base_lines.as_bytes()),
            ("file_a.txt", b"disjoint content a\n"),
        ],
        "commit in dim-a",
    );
    let dim_a_repo = DimensionRepository::for_dimension(Arc::clone(&repo), "dim-a").unwrap();
    dim_a_repo.set_head(&c_a.to_hex()).unwrap();

    let c_b = helper_create_commit(
        &repo,
        Some(base_commit),
        &[
            ("file.txt", base_lines.as_bytes()),
            ("file_b.txt", b"disjoint content b\n"),
        ],
        "commit in dim-b",
    );
    let dim_b_repo = DimensionRepository::for_dimension(Arc::clone(&repo), "dim-b").unwrap();
    dim_b_repo.set_head(&c_b.to_hex()).unwrap();

    let disjoint_metrics = entropy_sys.calculate("dim-a", "dim-b").unwrap();
    assert!(
        disjoint_metrics.total_entropy >= 0.0 && disjoint_metrics.total_entropy <= 0.20,
        "Clean disjoint edits must be in [0.0, 0.20], got {}",
        disjoint_metrics.total_entropy
    );
    assert_eq!(
        disjoint_metrics.hunk_collision_score, 0.0,
        "Disjoint edits must have C = 0.0"
    );
    assert_eq!(
        disjoint_metrics.file_overlap_score, 0.0,
        "Disjoint edits must have F = 0.0"
    );

    // Bound 3: Direct conflicting hunks >= 0.60
    let c_a_conf = helper_create_commit(
        &repo,
        Some(base_commit),
        &[("file.txt", b"alpha AAA\nbeta\ngamma\ndelta\nepsilon\n")],
        "conf a",
    );
    dim_a_repo.set_head(&c_a_conf.to_hex()).unwrap();

    let c_b_conf = helper_create_commit(
        &repo,
        Some(base_commit),
        &[("file.txt", b"alpha BBB\nbeta\ngamma\ndelta\nepsilon\n")],
        "conf b",
    );
    dim_b_repo.set_head(&c_b_conf.to_hex()).unwrap();

    let conf_metrics = entropy_sys.calculate("dim-a", "dim-b").unwrap();
    assert!(
        conf_metrics.total_entropy >= 0.60,
        "Direct conflicting hunks must produce total_entropy >= 0.60, got {}",
        conf_metrics.total_entropy
    );
    assert!(
        conf_metrics.hunk_collision_score >= 0.60,
        "Direct conflicting hunks must produce hunk_collision_score >= 0.60, got {}",
        conf_metrics.hunk_collision_score
    );
}

#[test]
fn test_entropy_uncommitted_identical_dimensions_bug() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let base_lines = "alpha\nbeta\ngamma\ndelta\nepsilon\n";
    let base_commit = helper_create_commit(
        &repo,
        None,
        &[("file.txt", base_lines.as_bytes())],
        "LCA base commit",
    );
    repo.refs()
        .write_ref(
            "refs/heads/mainline",
            &ReferenceTarget::Direct(base_commit),
            None,
            None,
        )
        .unwrap();

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();
    dim_mgr
        .create_dimension("dim-same-a", Some(&base_commit.to_hex()), None)
        .unwrap();
    dim_mgr
        .create_dimension("dim-same-b", Some(&base_commit.to_hex()), None)
        .unwrap();

    // Both dimensions make the EXACT SAME uncommitted workspace edits
    let ws_a = dir.path().join(".dft/dimensions/dim-same-a/workspace");
    let ws_b = dir.path().join(".dft/dimensions/dim-same-b/workspace");
    fs::create_dir_all(&ws_a).unwrap();
    fs::create_dir_all(&ws_b).unwrap();
    fs::write(ws_a.join("same.txt"), "exact same uncommitted content\n").unwrap();
    fs::write(ws_b.join("same.txt"), "exact same uncommitted content\n").unwrap();

    let entropy_sys = EntropySubsystem::new(Arc::clone(&repo));
    let metrics = entropy_sys.calculate("dim-same-a", "dim-same-b").unwrap();

    // EMPIRICAL BUG REPRODUCTION:
    // Dimensions in identical state have total_entropy > 0.25 because m_union was non-empty
    if metrics.total_entropy > 0.0 {
        eprintln!(
            "REPRODUCED ENTROPY BUG: Identical uncommitted states produced total_entropy = {} (expected 0.0)",
            metrics.total_entropy
        );
    }
    assert_eq!(
        metrics.total_entropy, 0.0,
        "Specification violation: Identical dimension states must have total_entropy = 0.0, got {}",
        metrics.total_entropy
    );

    // Adversarial Check A: Self-entropy identity H(D1, D1) = 0.0
    let self_metrics = entropy_sys.calculate("dim-same-a", "dim-same-a").unwrap();
    assert_eq!(
        self_metrics.total_entropy, 0.0,
        "Self-entropy H(D1, D1) must be exactly 0.0, got {}",
        self_metrics.total_entropy
    );

    // Adversarial Check B: Symmetry H(D1, D2) == H(D2, D1)
    let sym_metrics = entropy_sys.calculate("dim-same-b", "dim-same-a").unwrap();
    assert_eq!(
        metrics.total_entropy, sym_metrics.total_entropy,
        "Entropy must be symmetric: H(A, B) = {}, H(B, A) = {}",
        metrics.total_entropy, sym_metrics.total_entropy
    );

    // Adversarial Check C: Uncommitted conflicting edits MUST trigger H >= 0.60
    dim_mgr
        .create_dimension("dim-uconf-a", Some(&base_commit.to_hex()), None)
        .unwrap();
    dim_mgr
        .create_dimension("dim-uconf-b", Some(&base_commit.to_hex()), None)
        .unwrap();

    let ws_ca = dir.path().join(".dft/dimensions/dim-uconf-a/workspace");
    let ws_cb = dir.path().join(".dft/dimensions/dim-uconf-b/workspace");
    fs::create_dir_all(&ws_ca).unwrap();
    fs::create_dir_all(&ws_cb).unwrap();
    fs::write(
        ws_ca.join("file.txt"),
        "alpha modified by CA\nbeta\ngamma\ndelta\nepsilon\n",
    )
    .unwrap();
    fs::write(
        ws_cb.join("file.txt"),
        "alpha modified by CB\nbeta\ngamma\ndelta\nepsilon\n",
    )
    .unwrap();

    let uconf_metrics = entropy_sys.calculate("dim-uconf-a", "dim-uconf-b").unwrap();
    assert!(
        uconf_metrics.total_entropy >= 0.60,
        "Conflicting uncommitted modifications must trigger H >= 0.60, got {}",
        uconf_metrics.total_entropy
    );

    // Adversarial Check D: Uncommitted disjoint additions must have low entropy (0.0 < H <= 0.20)
    dim_mgr
        .create_dimension("dim-udisj-a", Some(&base_commit.to_hex()), None)
        .unwrap();
    dim_mgr
        .create_dimension("dim-udisj-b", Some(&base_commit.to_hex()), None)
        .unwrap();

    let ws_da = dir.path().join(".dft/dimensions/dim-udisj-a/workspace");
    let ws_db = dir.path().join(".dft/dimensions/dim-udisj-b/workspace");
    fs::create_dir_all(&ws_da).unwrap();
    fs::create_dir_all(&ws_db).unwrap();
    fs::write(ws_da.join("new_feature_a.txt"), "feature A contents\n").unwrap();
    fs::write(ws_db.join("new_feature_b.txt"), "feature B contents\n").unwrap();

    let udisj_metrics = entropy_sys.calculate("dim-udisj-a", "dim-udisj-b").unwrap();
    assert!(
        udisj_metrics.total_entropy > 0.0 && udisj_metrics.total_entropy <= 0.20,
        "Disjoint uncommitted files must produce low drift in (0.0, 0.20], got {}",
        udisj_metrics.total_entropy
    );
}

// ============================================================================
// 3. Territory Subsystem Tests
// ============================================================================

#[test]
fn test_territory_claims_and_glob_matching() {
    assert!(matches_glob("src/api/**", "src/api/v1/auth.rs"));
    assert!(matches_glob("src/api/**", "src/api/handler.rs"));
    assert!(!matches_glob("src/api/**", "src/core/auth.rs"));

    assert!(matches_glob("models/*.rs", "models/user.rs"));
    assert!(!matches_glob("models/*.rs", "models/nested/user.rs"));
    assert!(!matches_glob("models/*.rs", "src/models/user.rs"));

    assert!(patterns_overlap("src/api/**", "src/api/v1/auth.rs"));
    assert!(patterns_overlap("models/*.rs", "models/user.rs"));
    assert!(!patterns_overlap("src/api/**", "models/*.rs"));

    let dir = tempdir().unwrap();
    let dft_dir = dir.path().join(".dft");
    fs::create_dir_all(&dft_dir).unwrap();
    let tm = TerritoryManager::new(&dft_dir);

    // Claim src/api/** exclusively for dim-api
    let claim1 = tm
        .claim("src/api/**", "dim-api", Some("agent-api"), Some(3600), true)
        .unwrap();
    assert_eq!(claim1.path_glob, "src/api/**");

    // Overlapping claim should fail
    let conflict = tm.claim(
        "src/api/v1/auth.rs",
        "dim-other",
        Some("agent-other"),
        Some(3600),
        true,
    );
    assert!(conflict.is_err());

    // Disjoint claim should succeed
    let claim2 = tm
        .claim(
            "models/*.rs",
            "dim-models",
            Some("agent-mod"),
            Some(3600),
            true,
        )
        .unwrap();
    assert_eq!(claim2.path_glob, "models/*.rs");
}

#[test]
fn test_territory_ttl_expiration_and_audit_pruning() {
    let dir = tempdir().unwrap();
    let dft_dir = dir.path().join(".dft");
    fs::create_dir_all(&dft_dir).unwrap();
    let tm = TerritoryManager::new(&dft_dir);

    // Create a claim with 0s TTL (immediately expires)
    let _claim = tm
        .claim(
            "temp/expired.txt",
            "dim-temp",
            Some("agent-temp"),
            Some(0),
            true,
        )
        .unwrap();

    // Create an active claim with long TTL
    let _active = tm
        .claim(
            "active/valid.txt",
            "dim-act",
            Some("agent-act"),
            Some(3600),
            true,
        )
        .unwrap();

    std::thread::sleep(std::time::Duration::from_millis(50));

    // Audit without fix: should report expired claim, not pruned
    let report = tm.audit(false).unwrap();
    assert_eq!(report.expired_claims.len(), 1);
    assert_eq!(report.active_claims, 1);
    assert_eq!(report.total_claims, 2);
    assert!(report.fixed_pruned_claims.is_empty());

    // Audit with --fix: should prune expired claim
    let fix_report = tm.audit(true).unwrap();
    assert_eq!(fix_report.expired_claims.len(), 1);
    assert_eq!(fix_report.fixed_pruned_claims.len(), 1);

    // Next audit: expired claim is gone
    let clean_report = tm.audit(false).unwrap();
    assert_eq!(clean_report.expired_claims.len(), 0);
    assert_eq!(clean_report.active_claims, 1);
    assert_eq!(clean_report.total_claims, 1);
}

#[test]
fn test_territory_exclusionary_write_fences_subsystem() {
    let dir = tempdir().unwrap();
    let dft_dir = dir.path().join(".dft");
    fs::create_dir_all(&dft_dir).unwrap();
    let tm = TerritoryManager::new(&dft_dir);

    // Create a hard fence owned by dim-sec on "crates/security/**"
    let fence = tm
        .fence(
            "crates/security/**",
            Some("dim-sec"),
            Some("agent-sec"),
            true,
            Some(3600),
            Some("Security fence"),
        )
        .unwrap();
    assert!(fence.hard);

    // Owner dimension dim-sec is allowed to stage and commit
    let check_owner_stage =
        tm.check_stage_allowed("dim-sec", &[Path::new("crates/security/crypto.rs")]);
    assert!(
        check_owner_stage.is_ok(),
        "Owner dimension must be allowed to stage fenced files"
    );

    let check_owner_commit =
        tm.check_commit_allowed("dim-sec", &[Path::new("crates/security/crypto.rs")]);
    assert!(
        check_owner_commit.is_ok(),
        "Owner dimension must be allowed to commit fenced files"
    );

    // Non-owner dimension dim-rogue must be BLOCKED on stage and commit
    let check_rogue_stage =
        tm.check_stage_allowed("dim-rogue", &[Path::new("crates/security/crypto.rs")]);
    assert!(
        check_rogue_stage.is_err(),
        "Non-owner dimension must be BLOCKED from staging fenced files"
    );

    let check_rogue_commit =
        tm.check_commit_allowed("dim-rogue", &[Path::new("crates/security/crypto.rs")]);
    assert!(
        check_rogue_commit.is_err(),
        "Non-owner dimension must be BLOCKED from committing fenced files"
    );
}

#[test]
fn test_cli_exclusionary_write_fences_empirical() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Locate dft binary
    let candidates = [
        PathBuf::from("../../target/debug/dft"),
        PathBuf::from("../target/debug/dft"),
        PathBuf::from("target/debug/dft"),
    ];
    let dft_bin = candidates
        .into_iter()
        .find(|p| p.exists())
        .map(|p| p.canonicalize().unwrap());
    if dft_bin.is_none() {
        eprintln!("dft binary not found, skipping CLI invocation test");
        return;
    }
    let bin = dft_bin.unwrap();

    let run_dft = |args: &[&str]| -> (bool, String, String) {
        let out = std::process::Command::new(&bin)
            .args(args)
            .current_dir(root)
            .env("HOME", root)
            .env("DFT_CONFIG_DIR", root.join(".config"))
            .env("DFT_AUTHOR_NAME", "Test Agent")
            .env("DFT_AUTHOR_EMAIL", "agent@test.com")
            .output()
            .expect("Failed to execute dft");
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    };

    let (ok, _, _) = run_dft(&["init"]);
    assert!(ok);

    let (ok, _, _) = run_dft(&["dimension", "create", "owner_dim"]);
    assert!(ok);
    let (ok, _, _) = run_dft(&["dimension", "create", "rogue_dim"]);
    assert!(ok);

    // Enter owner_dim and fence src/api/**
    let (ok, _, _) = run_dft(&["dimension", "enter", "owner_dim"]);
    assert!(ok);
    let (ok, out, err) = run_dft(&["fence", "src/api/**", "--hard"]);
    assert!(ok, "fence creation failed: {} {}", out, err);

    // Switch to rogue_dim (non-owner)
    let (ok, _, _) = run_dft(&["dimension", "enter", "rogue_dim"]);
    assert!(ok);

    let api_file = root.join("src/api/auth.rs");
    fs::create_dir_all(api_file.parent().unwrap()).unwrap();
    fs::write(&api_file, "pub fn rogue_code() {}").unwrap();

    // Check dft add for non-owner: should strictly block non-owners from adding fenced files
    let (add_ok, add_out, add_err) = run_dft(&["add", "src/api/auth.rs"]);
    if add_ok {
        eprintln!("REPRODUCED CLI FENCE BUG: 'dft add' did NOT block non-owner on fenced file src/api/auth.rs!");
    }
    assert!(
        !add_ok,
        "Specification violation: 'dft add' must block non-owner on fenced path, but succeeded! out: {} err: {}",
        add_out, add_err
    );

    // Switch back to owner_dim: owner should be permitted to add and commit
    let (ok, _, _) = run_dft(&["dimension", "enter", "owner_dim"]);
    assert!(ok);
    fs::create_dir_all(api_file.parent().unwrap()).unwrap();
    fs::write(&api_file, "pub fn owner_code() {}").unwrap();

    let (owner_add_ok, owner_add_out, owner_add_err) = run_dft(&["add", "src/api/auth.rs"]);
    assert!(
        owner_add_ok,
        "Specification violation: 'dft add' must allow fence owner, but failed: {} {}",
        owner_add_out, owner_add_err
    );

    let (owner_commit_ok, owner_commit_out, owner_commit_err) =
        run_dft(&["commit", "-m", "owner auth commit"]);
    assert!(
        owner_commit_ok,
        "Specification violation: 'dft commit' must permit fence owner, but failed: {} {}",
        owner_commit_out, owner_commit_err
    );

    // Adversarial Check: Non-owner cannot bypass fence via 'dft add .'
    let (ok, _, _) = run_dft(&["dimension", "enter", "rogue_dim"]);
    assert!(ok);
    fs::create_dir_all(api_file.parent().unwrap()).unwrap();
    fs::write(&api_file, "pub fn rogue_bypass_attempt() {}").unwrap();

    let (dot_add_ok, dot_add_out, dot_add_err) = run_dft(&["add", "."]);
    assert!(
        !dot_add_ok,
        "Specification violation: 'dft add .' must block non-owner on fenced path, but succeeded! out: {} err: {}",
        dot_add_out, dot_add_err
    );

    // Adversarial Check: Non-owner is ALLOWED to add and commit unfenced files
    let unfenced = root.join("src/unfenced.txt");
    fs::write(&unfenced, "unfenced content").unwrap();
    let (unf_add_ok, unf_add_out, unf_add_err) = run_dft(&["add", "src/unfenced.txt"]);
    assert!(
        unf_add_ok,
        "Non-owner must be allowed to add unfenced files, but failed: {} {}",
        unf_add_out, unf_add_err
    );
    let (unf_commit_ok, unf_commit_out, unf_commit_err) =
        run_dft(&["commit", "-m", "rogue unfenced commit"]);
    assert!(
        unf_commit_ok,
        "Non-owner must be allowed to commit unfenced files, but failed: {} {}",
        unf_commit_out, unf_commit_err
    );
}
