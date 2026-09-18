//! Empirical Stress Tests for Daft Convergence Engine
//!
//! Tests:
//! 1. `converge`: N-way merge of 3+ dimensions (non-overlapping clean merge, strategy handling, strict preservation of source dimensions)
//! 2. `collapse`: multi-dimension distillation into atomic single-parent mainline commit
//! 3. `cascade`: sequential propagation along D1 -> D2 -> D3 and fail-fast abort on conflict
//! 4. `weave`: interleaved chronological commits across dimensions and topological causal order preservation
//! 5. `splice`: commit slice grafting across dimensions without merging full history

use daft_convergence::cascade::CascadeOutcome;
use daft_convergence::collapse::{CollapseOptions, CollapseOutcome};
use daft_convergence::converge::{ConvergeOptions, ConvergeOutcome};
use daft_convergence::strategy::MergeStrategy;
use daft_convergence::*;
use daft_core::cas::{ObjectId, ObjectType, RawObject};
use daft_core::diff::tree::flatten_tree;
use daft_core::init::{init, InitOptions};
use daft_core::merge::{build_hierarchical_tree, checkout_tree, get_signature};
use daft_core::object::{Commit, FileMode, Signature};
use daft_core::refs::ReferenceTarget;
use daft_core::Repository;
use daft_dimension::{DimensionManager, DimensionRepository};
use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

/// Helper to create a commit in a dimension or mainline with specific files and parent.
fn commit_in_dimension(
    repo: &Arc<Repository>,
    dim_name: &str,
    files: &[(&str, &str)],
    parents: Vec<ObjectId>,
    message: &str,
    timestamp_secs: Option<i64>,
) -> ObjectId {
    let dim_repo =
        DimensionRepository::for_dimension(Arc::clone(repo), dim_name).expect("open dim repo");

    let mut file_map: BTreeMap<String, (FileMode, ObjectId)> = BTreeMap::new();

    // If there is a parent, populate base files from the parent commit
    if let Some(parent_oid) = parents.first() {
        if let Ok(raw) = repo.cas().read_raw(parent_oid) {
            if let Ok(commit) = Commit::deserialize(&raw.data) {
                let _ = flatten_tree(repo.cas().as_ref(), &commit.tree, "", &mut file_map);
            }
        }
    }

    // Write new blobs and update file_map
    for (path, content) in files {
        let raw = RawObject::new(ObjectType::Blob, content.as_bytes().to_vec());
        let blob_oid = repo.cas().write_raw(&raw).expect("write blob");
        file_map.insert(path.to_string(), (FileMode::REGULAR, blob_oid));
    }

    // Build tree
    let tree_oid =
        build_hierarchical_tree(repo.cas().as_ref(), &file_map).expect("build hierarchical tree");

    let sig = if let Some(secs) = timestamp_secs {
        Signature::new("Tester", "tester@daft-vcs.org", secs, 0)
    } else {
        get_signature()
    };

    let commit = Commit::new(tree_oid, parents, sig.clone(), sig, message);
    let raw_commit = RawObject::new(ObjectType::Commit, commit.serialize());
    let commit_oid = repo.cas().write_raw(&raw_commit).expect("write commit");

    // Update dimension HEAD
    if dim_name == "mainline" {
        let _ = repo.refs().write_ref(
            "refs/heads/main",
            &ReferenceTarget::Direct(commit_oid),
            None,
            None,
        );
        let _ = repo.set_head(&ReferenceTarget::Direct(commit_oid));
        let dim_head = repo.dft_dir().join("dimensions/mainline/HEAD");
        if dim_head.exists() {
            let _ = fs::write(dim_head, format!("{}\n", commit_oid.to_hex()));
        }
    } else {
        dim_repo
            .set_head(&commit_oid.to_hex())
            .expect("set dim head");
    }

    // Materialize to workspace & index
    let ws = dim_repo.workdir();
    let _ = fs::create_dir_all(ws);
    let mut index = dim_repo.index().unwrap_or_default();
    let _ = checkout_tree(repo.cas().as_ref(), &tree_oid, ws, &mut index);
    let _ = dim_repo.write_index(&index);

    commit_oid
}

// ============================================================================
// 1. Stress-test `converge`
// ============================================================================

#[test]
fn test_converge_n_way_clean_and_source_preservation() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();

    // 1. Mainline base commit
    let base_oid = commit_in_dimension(
        &repo,
        "mainline",
        &[("shared.txt", "common base content\n")],
        Vec::new(),
        "Initial commit on mainline",
        Some(1000),
    );

    // 2. Create 4 parallel dimensions
    let dims = vec![
        "dim-alpha".to_string(),
        "dim-beta".to_string(),
        "dim-gamma".to_string(),
        "dim-delta".to_string(),
    ];
    let mut source_head_oids = Vec::new();

    for (i, dim) in dims.iter().enumerate() {
        dim_mgr.create_dimension(dim, None, None).unwrap();
        let fname = format!("feature_{}.txt", dim);
        let content = format!("content from {} version {}\n", dim, i);
        let head = commit_in_dimension(
            &repo,
            dim,
            &[(&fname, &content)],
            vec![base_oid],
            &format!("commit in {}", dim),
            Some(1100 + i as i64),
        );
        source_head_oids.push(head);
    }

    // Record source dimension states before convergence
    let mut pre_converge_files: Vec<Vec<String>> = Vec::new();
    for dim in &dims {
        let dim_repo = DimensionRepository::for_dimension(Arc::clone(&repo), dim).unwrap();
        assert_eq!(
            dim_repo.head_commit().unwrap(),
            source_head_oids[pre_converge_files.len()]
        );
        let mut files = BTreeMap::new();
        let raw = repo
            .cas()
            .read_raw(&dim_repo.head_commit().unwrap())
            .unwrap();
        let c = Commit::deserialize(&raw.data).unwrap();
        flatten_tree(repo.cas().as_ref(), &c.tree, "", &mut files).unwrap();
        pre_converge_files.push(files.keys().cloned().collect());
    }

    // 3. Converge all 4 dimensions into mainline
    let converge = ConvergeEngine::new(Arc::clone(&repo));
    let outcome = converge.converge(
        &dims,
        ConvergeOptions {
            into: Some("mainline".to_string()),
            strategy: MergeStrategy::ManualMarkers,
            message: Some("Converge 4 dimensions".to_string()),
            no_commit: false,
        },
    );

    match outcome {
        Ok(ConvergeOutcome::Clean {
            target_dimension,
            commit_oid,
            merged_dimensions,
        }) => {
            assert_eq!(target_dimension, "mainline");
            assert_eq!(merged_dimensions, dims);
            let c_oid = commit_oid.expect("commit must be created");

            // Verify octopus merge parents
            let raw = repo.cas().read_raw(&c_oid).unwrap();
            let commit = Commit::deserialize(&raw.data).unwrap();
            assert_eq!(commit.parents.len(), 5); // mainline base + 4 source dimensions
            assert!(commit.parents.contains(&base_oid));
            for sh in &source_head_oids {
                assert!(commit.parents.contains(sh));
            }

            // Verify all combined files exist in the converged tree
            let mut merged_files = BTreeMap::new();
            flatten_tree(repo.cas().as_ref(), &commit.tree, "", &mut merged_files).unwrap();
            assert!(merged_files.contains_key("shared.txt"));
            assert!(merged_files.contains_key("feature_dim-alpha.txt"));
            assert!(merged_files.contains_key("feature_dim-beta.txt"));
            assert!(merged_files.contains_key("feature_dim-gamma.txt"));
            assert!(merged_files.contains_key("feature_dim-delta.txt"));

            // Verify workspace disk content
            let mainline_ws = dir.path().to_path_buf();
            assert!(mainline_ws.join("shared.txt").exists());
            assert!(mainline_ws.join("feature_dim-alpha.txt").exists());
            assert!(mainline_ws.join("feature_dim-beta.txt").exists());
            assert!(mainline_ws.join("feature_dim-gamma.txt").exists());
            assert!(mainline_ws.join("feature_dim-delta.txt").exists());
        }
        other => panic!("Expected Clean converge outcome, got: {:?}", other),
    }

    // 4. STRICT PRESERVATION OF SOURCE DIMENSIONS
    // Verify each source dimension is completely unmodified (HEAD and files)
    for (i, dim) in dims.iter().enumerate() {
        let dim_repo = DimensionRepository::for_dimension(Arc::clone(&repo), dim).unwrap();
        assert_eq!(
            dim_repo.head_commit().unwrap(),
            source_head_oids[i],
            "Dimension {} HEAD must remain strictly unchanged",
            dim
        );

        let mut post_files = BTreeMap::new();
        let raw = repo.cas().read_raw(&source_head_oids[i]).unwrap();
        let c = Commit::deserialize(&raw.data).unwrap();
        flatten_tree(repo.cas().as_ref(), &c.tree, "", &mut post_files).unwrap();
        let file_keys: Vec<String> = post_files.keys().cloned().collect();
        assert_eq!(
            file_keys, pre_converge_files[i],
            "Dimension {} files must remain strictly unchanged",
            dim
        );

        // Verify other dimensions' files were NOT leaked into this dimension's workspace
        let ws = dim_repo.workdir();
        for other_dim in &dims {
            if other_dim != dim {
                let leaked_file = ws.join(format!("feature_{}.txt", other_dim));
                assert!(
                    !leaked_file.exists(),
                    "Dimension {} must not contain leaked file from {}",
                    dim,
                    other_dim
                );
            }
        }
    }
}

#[test]
fn test_converge_strategy_handling_ours_theirs_manual() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();

    let base_oid = commit_in_dimension(
        &repo,
        "mainline",
        &[("conflict.txt", "base version\n")],
        Vec::new(),
        "Base",
        Some(1000),
    );

    dim_mgr.create_dimension("dim-one", None, None).unwrap();
    dim_mgr.create_dimension("dim-two", None, None).unwrap();

    let _h_one = commit_in_dimension(
        &repo,
        "dim-one",
        &[("conflict.txt", "version from dim-one\n")],
        vec![base_oid],
        "Commit dim-one",
        Some(1010),
    );

    let _h_two = commit_in_dimension(
        &repo,
        "dim-two",
        &[("conflict.txt", "version from dim-two\n")],
        vec![base_oid],
        "Commit dim-two",
        Some(1020),
    );

    // Target has its own modification
    let _tgt_head = commit_in_dimension(
        &repo,
        "mainline",
        &[("conflict.txt", "version from target mainline\n")],
        vec![base_oid],
        "Mainline mod",
        Some(1030),
    );

    let converge = ConvergeEngine::new(Arc::clone(&repo));

    // 1. Test MergeStrategy::ManualMarkers -> Expect Conflict with formatted markers
    let outcome_manual = converge
        .converge(
            &["dim-one".to_string(), "dim-two".to_string()],
            ConvergeOptions {
                into: Some("mainline".to_string()),
                strategy: MergeStrategy::ManualMarkers,
                message: None,
                no_commit: false,
            },
        )
        .unwrap();

    match outcome_manual {
        ConvergeOutcome::Conflict {
            target_dimension,
            conflicting_files,
        } => {
            assert_eq!(target_dimension, "mainline");
            assert_eq!(conflicting_files, vec!["conflict.txt"]);

            let marker_content = fs::read_to_string(dir.path().join("conflict.txt")).unwrap();
            assert!(
                marker_content.contains("<<<<<<< ours (mainline)"),
                "Marker must contain ours header"
            );
            assert!(
                marker_content.contains("version from target mainline"),
                "Marker must contain target content"
            );
            assert!(
                marker_content.contains("||||||| base"),
                "Marker must contain base separator"
            );
            assert!(
                marker_content.contains("base version"),
                "Marker must contain base content"
            );
            assert!(
                marker_content.contains("======="),
                "Marker must contain conflict separator"
            );
            assert!(
                marker_content.contains("// [dim-one]"),
                "Marker must label dim-one section"
            );
            assert!(
                marker_content.contains("version from dim-one"),
                "Marker must contain dim-one content"
            );
            assert!(
                marker_content.contains("// [dim-two]"),
                "Marker must label dim-two section"
            );
            assert!(
                marker_content.contains("version from dim-two"),
                "Marker must contain dim-two content"
            );
            assert!(
                marker_content.contains(">>>>>>> dim-one,dim-two"),
                "Marker must end with dimensions footer"
            );
        }
        other => panic!("Expected Conflict for ManualMarkers, got {:?}", other),
    }

    // 2. Test MergeStrategy::Ours -> Resolves cleanly keeping target mainline
    let outcome_ours = converge
        .converge(
            &["dim-one".to_string(), "dim-two".to_string()],
            ConvergeOptions {
                into: Some("mainline".to_string()),
                strategy: MergeStrategy::Ours,
                message: Some("Converge ours".to_string()),
                no_commit: false,
            },
        )
        .unwrap();

    match outcome_ours {
        ConvergeOutcome::Clean { .. } => {
            let disk_content = fs::read_to_string(dir.path().join("conflict.txt")).unwrap();
            assert_eq!(disk_content, "version from target mainline\n");
        }
        other => panic!("Expected Clean for Ours, got {:?}", other),
    }

    // 3. Test MergeStrategy::Theirs -> Resolves cleanly with source version
    let outcome_theirs = converge
        .converge(
            &["dim-one".to_string(), "dim-two".to_string()],
            ConvergeOptions {
                into: Some("mainline".to_string()),
                strategy: MergeStrategy::Theirs,
                message: Some("Converge theirs".to_string()),
                no_commit: false,
            },
        )
        .unwrap();

    match outcome_theirs {
        ConvergeOutcome::Clean { .. } => {
            let disk_content = fs::read_to_string(dir.path().join("conflict.txt")).unwrap();
            assert_eq!(disk_content, "version from dim-two\n");
        }
        other => panic!("Expected Clean for Theirs, got {:?}", other),
    }
}

// ============================================================================
// 2. Stress-test `collapse`
// ============================================================================

#[test]
fn test_collapse_multi_dimension_distillation_atomic_single_parent() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();

    // Base commit on mainline
    let base_oid = commit_in_dimension(
        &repo,
        "mainline",
        &[("root.txt", "mainline initial root\n")],
        Vec::new(),
        "Initial mainline",
        Some(1000),
    );

    // Create 3 parallel dimensions with deep histories
    let d1 = "worker-alpha";
    let d2 = "worker-beta";
    let d3 = "worker-gamma";
    dim_mgr.create_dimension(d1, None, None).unwrap();
    dim_mgr.create_dimension(d2, None, None).unwrap();
    dim_mgr.create_dimension(d3, None, None).unwrap();

    // d1: 2 commits
    let c1_1 = commit_in_dimension(
        &repo,
        d1,
        &[("alpha_1.txt", "alpha 1")],
        vec![base_oid],
        "d1 commit 1",
        Some(1010),
    );
    let _c1_2 = commit_in_dimension(
        &repo,
        d1,
        &[("alpha_2.txt", "alpha 2")],
        vec![c1_1],
        "d1 commit 2",
        Some(1020),
    );

    // d2: 2 commits
    let c2_1 = commit_in_dimension(
        &repo,
        d2,
        &[("beta_1.txt", "beta 1")],
        vec![base_oid],
        "d2 commit 1",
        Some(1015),
    );
    let _c2_2 = commit_in_dimension(
        &repo,
        d2,
        &[("beta_2.txt", "beta 2")],
        vec![c2_1],
        "d2 commit 2",
        Some(1025),
    );

    // d3: 2 commits
    let c3_1 = commit_in_dimension(
        &repo,
        d3,
        &[("gamma_1.txt", "gamma 1")],
        vec![base_oid],
        "d3 commit 1",
        Some(1018),
    );
    let _c3_2 = commit_in_dimension(
        &repo,
        d3,
        &[("gamma_2.txt", "gamma 2")],
        vec![c3_1],
        "d3 commit 2",
        Some(1028),
    );

    // Also leave an uncommitted in-flight file in d1 workspace
    let d1_repo = DimensionRepository::for_dimension(Arc::clone(&repo), d1).unwrap();
    fs::write(
        d1_repo.workdir().join("uncommitted_in_flight.txt"),
        "live uncommitted work\n",
    )
    .unwrap();

    // Collapse all dimensions into mainline
    let collapse = CollapseEngine::new(Arc::clone(&repo));
    let outcome = collapse
        .collapse(
            &[d1.to_string(), d2.to_string(), d3.to_string()],
            CollapseOptions {
                into: Some("mainline".to_string()),
                strategy: Some(MergeStrategy::Ours),
                message: Some("Wavefunction collapse into mainline".to_string()),
            },
        )
        .unwrap();

    match outcome {
        CollapseOutcome::Clean {
            target_dimension,
            commit_oid,
            collapsed_dimensions,
        } => {
            assert_eq!(target_dimension, "mainline");
            assert_eq!(collapsed_dimensions.len(), 3);

            // VERIFY ATOMIC SINGLE-PARENT DISTILLATION
            let raw = repo.cas().read_raw(&commit_oid).unwrap();
            let commit = Commit::deserialize(&raw.data).unwrap();

            assert_eq!(
                commit.parents.len(),
                1,
                "Collapse MUST produce an atomic, single-parent commit (squashed distillation)"
            );
            assert_eq!(
                commit.parents[0], base_oid,
                "Single parent MUST be the previous target HEAD"
            );

            // VERIFY ALL COMBINED FILES ARE PRESENT IN TREE
            let mut tree_files = BTreeMap::new();
            flatten_tree(repo.cas().as_ref(), &commit.tree, "", &mut tree_files).unwrap();

            assert!(tree_files.contains_key("root.txt"));
            assert!(tree_files.contains_key("alpha_1.txt"));
            assert!(tree_files.contains_key("alpha_2.txt"));
            assert!(tree_files.contains_key("beta_1.txt"));
            assert!(tree_files.contains_key("beta_2.txt"));
            assert!(tree_files.contains_key("gamma_1.txt"));
            assert!(tree_files.contains_key("gamma_2.txt"));
            assert!(
                tree_files.contains_key("uncommitted_in_flight.txt"),
                "Uncommitted workspace files from dimensions must be captured in distillation"
            );

            // VERIFY DISK WORKSPACE
            assert!(dir.path().join("root.txt").exists());
            assert!(dir.path().join("alpha_1.txt").exists());
            assert!(dir.path().join("alpha_2.txt").exists());
            assert!(dir.path().join("beta_1.txt").exists());
            assert!(dir.path().join("beta_2.txt").exists());
            assert!(dir.path().join("gamma_1.txt").exists());
            assert!(dir.path().join("gamma_2.txt").exists());
            assert!(dir.path().join("uncommitted_in_flight.txt").exists());
        }
        other => panic!("Expected Clean collapse outcome, got {:?}", other),
    }

    // Running collapse again with no new changes should be NoOp
    let noop_outcome = collapse
        .collapse(
            &[d1.to_string(), d2.to_string(), d3.to_string()],
            CollapseOptions {
                into: Some("mainline".to_string()),
                strategy: Some(MergeStrategy::Ours),
                message: None,
            },
        )
        .unwrap();

    match noop_outcome {
        CollapseOutcome::NoOp { target_dimension } => {
            assert_eq!(target_dimension, "mainline");
        }
        other => panic!("Expected NoOp for identical re-collapse, got {:?}", other),
    }
}

// ============================================================================
// 3. Stress-test `cascade`
// ============================================================================

#[test]
fn test_cascade_sequential_propagation_and_fail_fast_abort() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();

    // 1. Mainline base
    let base_oid = commit_in_dimension(
        &repo,
        "mainline",
        &[("common.txt", "common base\n"), ("propagate.txt", "v1\n")],
        Vec::new(),
        "Base",
        Some(1000),
    );

    // 2. Setup fork chain: D1 -> D2 -> D3
    let d1 = "dim-level-1";
    let d2 = "dim-level-2";
    let d3 = "dim-level-3";

    dim_mgr.fork_dimension(d1, "mainline").unwrap();
    dim_mgr.fork_dimension(d2, d1).unwrap();
    dim_mgr.fork_dimension(d3, d2).unwrap();

    // Set initial heads to base_oid
    commit_in_dimension(&repo, d1, &[], vec![base_oid], "d1 init", Some(1005));
    commit_in_dimension(&repo, d2, &[], vec![base_oid], "d2 init", Some(1006));
    let _d3_initial_head =
        commit_in_dimension(&repo, d3, &[], vec![base_oid], "d3 init", Some(1007));

    let cascade = CascadeEngine::new(Arc::clone(&repo));

    // Verify resolve_chain detects D1 -> D2 -> D3
    let chain = cascade.resolve_chain(d1, None).unwrap();
    assert_eq!(chain, vec![d1, d2, d3]);

    // 3. Clean propagation test: modify in D1
    let d1_head_clean = commit_in_dimension(
        &repo,
        d1,
        &[
            ("propagate.txt", "v2 updated from D1\n"),
            ("new_d1.txt", "hello d1\n"),
        ],
        vec![base_oid],
        "D1 update",
        Some(1050),
    );

    let res_clean = cascade.cascade(d1, None, true).unwrap();
    match res_clean {
        CascadeOutcome::Success {
            propagated_chain,
            steps,
        } => {
            assert_eq!(propagated_chain, vec![d1, d2, d3]);
            assert_eq!(steps.len(), 2);
            assert_eq!(steps[0].from_dimension, d1);
            assert_eq!(steps[0].to_dimension, d2);
            assert_eq!(steps[0].status, "Success");
            assert_eq!(steps[1].from_dimension, d2);
            assert_eq!(steps[1].to_dimension, d3);
            assert_eq!(steps[1].status, "Success");

            // Verify D2 and D3 received the changes
            let d2_repo = DimensionRepository::for_dimension(Arc::clone(&repo), d2).unwrap();
            let d3_repo = DimensionRepository::for_dimension(Arc::clone(&repo), d3).unwrap();

            assert!(d2_repo.workdir().join("new_d1.txt").exists());
            assert!(d3_repo.workdir().join("new_d1.txt").exists());

            let d3_prop = fs::read_to_string(d3_repo.workdir().join("propagate.txt")).unwrap();
            assert_eq!(d3_prop, "v2 updated from D1\n");
        }
        other => panic!("Expected Success for clean cascade, got {:?}", other),
    }

    // 4. FAIL-FAST CLEAN ABORT TEST
    // Setup a conflicting edit between D1 and D2:
    // In D1: modify conflict_cascade.txt to "alpha"
    // In D2: modify conflict_cascade.txt to "beta"
    // D3: unmodified, records its current head
    let d2_repo = DimensionRepository::for_dimension(Arc::clone(&repo), d2).unwrap();
    let d3_repo = DimensionRepository::for_dimension(Arc::clone(&repo), d3).unwrap();

    let d2_curr_head = d2_repo.head_commit().unwrap();
    let d3_curr_head = d3_repo.head_commit().unwrap();

    let _d1_conflict_head = commit_in_dimension(
        &repo,
        d1,
        &[("conflict_cascade.txt", "alpha version in D1\n")],
        vec![d1_head_clean],
        "D1 conflict commit",
        Some(1100),
    );

    let _d2_conflict_head = commit_in_dimension(
        &repo,
        d2,
        &[("conflict_cascade.txt", "beta conflicting version in D2\n")],
        vec![d2_curr_head],
        "D2 conflict commit",
        Some(1105),
    );

    // Trigger cascade with abort_on_conflict = true
    let res_conflict = cascade.cascade(d1, None, true).unwrap();

    match res_conflict {
        CascadeOutcome::AbortedOnConflict {
            at_dimension,
            conflicting_files,
            completed_chain,
        } => {
            assert_eq!(
                at_dimension, d2,
                "Cascade must abort at the first conflicting dimension D2"
            );
            assert!(
                conflicting_files.contains(&"conflict_cascade.txt".to_string()),
                "Conflicting files must list the conflicted file"
            );
            assert_eq!(
                completed_chain,
                vec![d1],
                "Completed chain must only contain D1 (propagation stopped before D2 and D3)"
            );

            // STRICT FAIL-FAST VERIFICATION ON D3:
            // D3 must NOT have received any commits, its HEAD must match d3_curr_head
            let d3_post_head = d3_repo.head_commit().unwrap();
            assert_eq!(
                d3_post_head, d3_curr_head,
                "D3 HEAD must remain strictly unchanged after upstream cascade abort"
            );

            // D3 workspace must NOT contain conflict_cascade.txt
            assert!(
                !d3_repo.workdir().join("conflict_cascade.txt").exists(),
                "D3 workspace must not be touched when cascade aborts upstream"
            );
        }
        other => panic!("Expected AbortedOnConflict, got {:?}", other),
    }
}

// ============================================================================
// 4. Stress-test `weave`
// ============================================================================

#[test]
fn test_weave_interleaved_chronological_commits_and_causal_order() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();

    // Base commit C0 at T=1000
    let base_oid = commit_in_dimension(
        &repo,
        "mainline",
        &[("base.txt", "base timeline\n")],
        Vec::new(),
        "Base C0",
        Some(1000),
    );

    let dim_a = "stream-alpha";
    let dim_b = "stream-beta";
    dim_mgr.create_dimension(dim_a, None, None).unwrap();
    dim_mgr.create_dimension(dim_b, None, None).unwrap();

    // We build an interleaved chronology:
    // T=1100: A1 on stream-alpha
    // T=1200: B1 on stream-beta
    // T=1300: A2 on stream-alpha
    // T=1400: B2 on stream-beta
    let a1_oid = commit_in_dimension(
        &repo,
        dim_a,
        &[("a1.txt", "content a1\n")],
        vec![base_oid],
        "Feature A1",
        Some(1100),
    );

    let b1_oid = commit_in_dimension(
        &repo,
        dim_b,
        &[("b1.txt", "content b1\n")],
        vec![base_oid],
        "Feature B1",
        Some(1200),
    );

    let a2_oid = commit_in_dimension(
        &repo,
        dim_a,
        &[("a2.txt", "content a2\n")],
        vec![a1_oid],
        "Feature A2",
        Some(1300),
    );

    let b2_oid = commit_in_dimension(
        &repo,
        dim_b,
        &[("b2.txt", "content b2\n")],
        vec![b1_oid],
        "Feature B2",
        Some(1400),
    );

    let weave = WeaveEngine::new(Arc::clone(&repo));
    let outcome = weave
        .weave(dim_a, dim_b, Some("woven-timeline"), None)
        .expect("weave should succeed");

    assert_eq!(outcome.target_dimension, "woven-timeline");
    assert_eq!(outcome.commit_count, 4);
    assert_eq!(outcome.woven_commits.len(), 4);

    let woven_repo =
        DimensionRepository::for_dimension(Arc::clone(&repo), "woven-timeline").unwrap();
    assert_eq!(woven_repo.head_commit().unwrap(), outcome.woven_head);

    // Trace causal order from woven_head backwards
    // 4th commit (tip): corresponds to B2 (T=1400)
    let c4_raw = repo.cas().read_raw(&outcome.woven_head).unwrap();
    let c4 = Commit::deserialize(&c4_raw.data).unwrap();
    assert!(c4.message.contains("Feature B2"));
    assert!(c4
        .message
        .contains(&format!("woven from {}:{}", dim_b, b2_oid.to_hex())));
    assert_eq!(c4.parents.len(), 1);
    let p3 = c4.parents[0];

    // 3rd commit: corresponds to A2 (T=1300)
    let c3_raw = repo.cas().read_raw(&p3).unwrap();
    let c3 = Commit::deserialize(&c3_raw.data).unwrap();
    assert!(c3.message.contains("Feature A2"));
    assert!(c3
        .message
        .contains(&format!("woven from {}:{}", dim_a, a2_oid.to_hex())));
    assert_eq!(c3.parents.len(), 1);
    let p2 = c3.parents[0];

    // 2nd commit: corresponds to B1 (T=1200)
    let c2_raw = repo.cas().read_raw(&p2).unwrap();
    let c2 = Commit::deserialize(&c2_raw.data).unwrap();
    assert!(c2.message.contains("Feature B1"));
    assert!(c2
        .message
        .contains(&format!("woven from {}:{}", dim_b, b1_oid.to_hex())));
    assert_eq!(c2.parents.len(), 1);
    let p1 = c2.parents[0];

    // 1st commit: corresponds to A1 (T=1100)
    let c1_raw = repo.cas().read_raw(&p1).unwrap();
    let c1 = Commit::deserialize(&c1_raw.data).unwrap();
    assert!(c1.message.contains("Feature A1"));
    assert!(c1
        .message
        .contains(&format!("woven from {}:{}", dim_a, a1_oid.to_hex())));
    assert_eq!(c1.parents.len(), 1);
    assert_eq!(
        c1.parents[0], base_oid,
        "First woven commit parent must be base commit C0"
    );

    // Verify all files exist in woven target workspace
    let ws = woven_repo.workdir();
    assert!(ws.join("base.txt").exists());
    assert!(ws.join("a1.txt").exists());
    assert!(ws.join("b1.txt").exists());
    assert!(ws.join("a2.txt").exists());
    assert!(ws.join("b2.txt").exists());
}

// ============================================================================
// 5. Stress-test `splice`
// ============================================================================

#[test]
fn test_splice_commit_slice_grafting_without_merging_full_history() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();

    let donor = "donor-stream";
    let recipient = "recipient-stream";
    dim_mgr.create_dimension(donor, None, None).unwrap();
    dim_mgr.create_dimension(recipient, None, None).unwrap();

    // Recipient initial state
    let r0_oid = commit_in_dimension(
        &repo,
        recipient,
        &[("recipient_core.txt", "recipient base data\n")],
        Vec::new(),
        "Recipient R0",
        Some(1000),
    );

    // Donor state:
    // D1: noise commit to skip
    // D2: target feature part 1
    // D3: target feature part 2
    // D4: noise commit to skip
    let d1_oid = commit_in_dimension(
        &repo,
        donor,
        &[("noise_early.txt", "should not be spliced\n")],
        Vec::new(),
        "Donor D1 noise",
        Some(1010),
    );

    let d2_oid = commit_in_dimension(
        &repo,
        donor,
        &[("splice_feat_1.txt", "spliced feature part 1\n")],
        vec![d1_oid],
        "Donor D2 feature 1",
        Some(1020),
    );

    let d3_oid = commit_in_dimension(
        &repo,
        donor,
        &[("splice_feat_2.txt", "spliced feature part 2\n")],
        vec![d2_oid],
        "Donor D3 feature 2",
        Some(1030),
    );

    let _d4_oid = commit_in_dimension(
        &repo,
        donor,
        &[("noise_late.txt", "should also not be spliced\n")],
        vec![d3_oid],
        "Donor D4 noise",
        Some(1040),
    );

    // We want to splice ONLY range D1..D3 (i.e. D2 and D3) into recipient
    let splice = SpliceEngine::new(Arc::clone(&repo));
    let range_spec = format!("{}..{}", d1_oid.to_hex(), d3_oid.to_hex());

    let outcome = splice
        .splice(donor, recipient, &range_spec, None)
        .expect("splice should succeed");

    assert_eq!(outcome.donor_dim, donor);
    assert_eq!(outcome.target_dim, recipient);
    assert_eq!(outcome.spliced_commits.len(), 2);

    // Verify recipient HEAD commit
    let recipient_repo = DimensionRepository::for_dimension(Arc::clone(&repo), recipient).unwrap();
    assert_eq!(
        recipient_repo.head_commit().unwrap(),
        outcome.new_target_head
    );

    // Verify grafted lineage:
    // Top commit (D3 grafted) -> parent is D2 grafted -> parent is R0
    let top_raw = repo.cas().read_raw(&outcome.new_target_head).unwrap();
    let top_commit = Commit::deserialize(&top_raw.data).unwrap();
    assert!(top_commit.message.contains("Donor D3 feature 2"));
    assert!(top_commit
        .message
        .contains(&format!("spliced from {}:{}", donor, d3_oid.to_hex())));
    assert_eq!(top_commit.parents.len(), 1);

    let mid_oid = top_commit.parents[0];
    let mid_raw = repo.cas().read_raw(&mid_oid).unwrap();
    let mid_commit = Commit::deserialize(&mid_raw.data).unwrap();
    assert!(mid_commit.message.contains("Donor D2 feature 1"));
    assert!(mid_commit
        .message
        .contains(&format!("spliced from {}:{}", donor, d2_oid.to_hex())));
    assert_eq!(mid_commit.parents.len(), 1);
    assert_eq!(
        mid_commit.parents[0], r0_oid,
        "Grafted slice base parent must be recipient's original tip R0"
    );

    // Verify workspace contents:
    // Spliced files and recipient initial files exist; noise files DO NOT exist!
    let ws = recipient_repo.workdir();
    assert!(ws.join("recipient_core.txt").exists());
    assert!(ws.join("splice_feat_1.txt").exists());
    assert!(ws.join("splice_feat_2.txt").exists());
    assert!(
        !ws.join("noise_early.txt").exists(),
        "Early noise commit D1 was not in slice and must not exist in recipient"
    );
    assert!(
        !ws.join("noise_late.txt").exists(),
        "Late noise commit D4 was not in slice and must not exist in recipient"
    );
}

#[test]
fn test_converge_delete_vs_modify_conflict() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();

    // Base commit with file_to_change.txt
    let base_oid = commit_in_dimension(
        &repo,
        "mainline",
        &[("file_to_change.txt", "original text\n")],
        Vec::new(),
        "Base",
        Some(1000),
    );

    let dim_mod = "dim-modifier";
    let dim_del = "dim-deleter";
    dim_mgr.create_dimension(dim_mod, None, None).unwrap();
    dim_mgr.create_dimension(dim_del, None, None).unwrap();

    // Modifier modifies file_to_change.txt
    commit_in_dimension(
        &repo,
        dim_mod,
        &[("file_to_change.txt", "modified text\n")],
        vec![base_oid],
        "Modify",
        Some(1010),
    );

    // Deleter deletes file_to_change.txt (empty tree)
    let empty_tree = daft_core::cas::RawObject::new(daft_core::cas::ObjectType::Tree, Vec::new());
    let tree_oid = repo.cas().write_raw(&empty_tree).unwrap();
    let sig = Signature::new("Tester", "tester@daft-vcs.org", 1020, 0);
    let commit = Commit::new(tree_oid, vec![base_oid], sig.clone(), sig, "Delete file");
    let raw_c =
        daft_core::cas::RawObject::new(daft_core::cas::ObjectType::Commit, commit.serialize());
    let del_oid = repo.cas().write_raw(&raw_c).unwrap();
    let dim_del_repo = DimensionRepository::for_dimension(Arc::clone(&repo), dim_del).unwrap();
    dim_del_repo.set_head(&del_oid.to_hex()).unwrap();

    let converge = ConvergeEngine::new(Arc::clone(&repo));
    let outcome = converge
        .converge(
            &[dim_mod.to_string(), dim_del.to_string()],
            ConvergeOptions {
                into: Some("mainline".to_string()),
                strategy: MergeStrategy::ManualMarkers,
                message: None,
                no_commit: false,
            },
        )
        .unwrap();

    // Conflict expected because one dimension modified and another deleted
    match outcome {
        ConvergeOutcome::Conflict {
            conflicting_files, ..
        } => {
            assert!(conflicting_files.contains(&"file_to_change.txt".to_string()));
        }
        other => panic!("Expected Conflict for modify vs delete, got {:?}", other),
    }
}

#[test]
fn test_cascade_multi_hop_with_uptodate_intermediate() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();

    let base_oid = commit_in_dimension(
        &repo,
        "mainline",
        &[("base.txt", "base\n")],
        Vec::new(),
        "Base",
        Some(1000),
    );

    // Chain: D1 -> D2 -> D3 -> D4
    let d1 = "cascade-d1";
    let d2 = "cascade-d2";
    let d3 = "cascade-d3";
    let d4 = "cascade-d4";

    dim_mgr.fork_dimension(d1, "mainline").unwrap();
    dim_mgr.fork_dimension(d2, d1).unwrap();
    dim_mgr.fork_dimension(d3, d2).unwrap();
    dim_mgr.fork_dimension(d4, d3).unwrap();

    // D1, D2, D3, D4 all start at base_oid
    commit_in_dimension(&repo, d1, &[], vec![base_oid], "init d1", Some(1001));
    commit_in_dimension(&repo, d2, &[], vec![base_oid], "init d2", Some(1002));
    commit_in_dimension(&repo, d3, &[], vec![base_oid], "init d3", Some(1003));
    commit_in_dimension(&repo, d4, &[], vec![base_oid], "init d4", Some(1004));

    // First propagate a change from D1 to D2 directly
    commit_in_dimension(
        &repo,
        d1,
        &[("file_hop.txt", "hop 1\n")],
        vec![base_oid],
        "d1 hop",
        Some(1010),
    );
    let cascade = CascadeEngine::new(Arc::clone(&repo));
    let res1 = cascade.cascade(d1, Some(d2), true).unwrap();
    match res1 {
        CascadeOutcome::Success { steps, .. } => {
            assert_eq!(steps.len(), 1);
            assert_eq!(steps[0].status, "Success");
        }
        other => panic!("Expected Success, got {:?}", other),
    }

    // Now D2 already has the change! Run full cascade from D1 to D4
    let res_full = cascade.cascade(d1, None, true).unwrap();
    match res_full {
        CascadeOutcome::Success {
            propagated_chain,
            steps,
        } => {
            assert_eq!(propagated_chain, vec![d1, d2, d3, d4]);
            // Step D1 -> D2 should be UpToDate
            assert_eq!(steps[0].status, "UpToDate");
            // Steps D2 -> D3 and D3 -> D4 should be Success
            assert_eq!(steps[1].status, "Success");
            assert_eq!(steps[2].status, "Success");

            // Verify D4 workspace has file_hop.txt
            let d4_repo = DimensionRepository::for_dimension(Arc::clone(&repo), d4).unwrap();
            assert!(d4_repo.workdir().join("file_hop.txt").exists());
        }
        other => panic!("Expected Success on multi-hop cascade, got {:?}", other),
    }
}

#[test]
fn test_splice_single_commit() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();

    let donor = "donor-single";
    let recipient = "recipient-single";
    dim_mgr.create_dimension(donor, None, None).unwrap();
    dim_mgr.create_dimension(recipient, None, None).unwrap();

    let r_init = commit_in_dimension(
        &repo,
        recipient,
        &[("existing.txt", "keep me\n")],
        Vec::new(),
        "Recipient Init",
        Some(1000),
    );

    let d_single = commit_in_dimension(
        &repo,
        donor,
        &[("feature_solo.txt", "solo feature\n")],
        Vec::new(),
        "Donor Solo Feature",
        Some(1010),
    );

    let splice = SpliceEngine::new(Arc::clone(&repo));
    let outcome = splice
        .splice(donor, recipient, &d_single.to_hex(), None)
        .unwrap();

    assert_eq!(outcome.spliced_commits.len(), 1);
    let recipient_repo = DimensionRepository::for_dimension(Arc::clone(&repo), recipient).unwrap();
    assert_eq!(
        recipient_repo.head_commit().unwrap(),
        outcome.new_target_head
    );

    let raw = repo.cas().read_raw(&outcome.new_target_head).unwrap();
    let c = Commit::deserialize(&raw.data).unwrap();
    assert_eq!(c.parents, vec![r_init]);

    let ws = recipient_repo.workdir();
    assert!(ws.join("existing.txt").exists());
    assert!(ws.join("feature_solo.txt").exists());
}

#[test]
fn test_collapse_with_binary_and_union() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();

    // Commit a binary file (containing null bytes)
    let binary_data = vec![0u8, 159, 255, 0, 12, 34, 0];
    let raw_bin = RawObject::new(ObjectType::Blob, binary_data.clone());
    let bin_oid = repo.cas().write_raw(&raw_bin).unwrap();

    let mut file_map = BTreeMap::new();
    file_map.insert("image.bin".to_string(), (FileMode::REGULAR, bin_oid));
    file_map.insert(
        "notes.txt".to_string(),
        (
            FileMode::REGULAR,
            repo.cas()
                .write_raw(&RawObject::new(ObjectType::Blob, b"line 1\n".to_vec()))
                .unwrap(),
        ),
    );
    let tree_oid = build_hierarchical_tree(repo.cas().as_ref(), &file_map).unwrap();
    let sig = Signature::new("Tester", "test@test.org", 1000, 0);
    let c = Commit::new(tree_oid, Vec::new(), sig.clone(), sig, "Base with binary");
    let base_oid = repo
        .cas()
        .write_raw(&RawObject::new(ObjectType::Commit, c.serialize()))
        .unwrap();

    let _ = repo.refs().write_ref(
        "refs/heads/main",
        &ReferenceTarget::Direct(base_oid),
        None,
        None,
    );
    let _ = repo.set_head(&ReferenceTarget::Direct(base_oid));

    let d1 = "dim-bin-1";
    dim_mgr.create_dimension(d1, None, None).unwrap();
    commit_in_dimension(
        &repo,
        d1,
        &[("notes.txt", "line 1\nline 2 from d1\n")],
        vec![base_oid],
        "Notes from d1",
        Some(1010),
    );

    let collapse = CollapseEngine::new(Arc::clone(&repo));
    let outcome = collapse
        .collapse(
            &[d1.to_string()],
            CollapseOptions {
                into: Some("mainline".to_string()),
                strategy: Some(MergeStrategy::Union),
                message: Some("Collapse with binary file preserved".to_string()),
            },
        )
        .unwrap();

    match outcome {
        CollapseOutcome::Clean {
            target_dimension,
            commit_oid,
            ..
        } => {
            assert_eq!(target_dimension, "mainline");
            let raw = repo.cas().read_raw(&commit_oid).unwrap();
            let c = Commit::deserialize(&raw.data).unwrap();
            let mut final_files = BTreeMap::new();
            flatten_tree(repo.cas().as_ref(), &c.tree, "", &mut final_files).unwrap();

            // Verify binary file is intact with exact original blob OID
            assert_eq!(final_files.get("image.bin").unwrap().1, bin_oid);

            // Verify notes.txt has unioned content
            let notes_oid = final_files.get("notes.txt").unwrap().1;
            let notes_raw = repo.cas().read_raw(&notes_oid).unwrap();
            let notes_str = String::from_utf8_lossy(&notes_raw.data);
            assert!(notes_str.contains("line 1"));
            assert!(notes_str.contains("line 2 from d1"));
        }
        other => panic!("Expected Clean, got {:?}", other),
    }
}
