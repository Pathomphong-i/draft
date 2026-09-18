use chrono::Utc;
use daft_core::cas::{ObjectId, ObjectType, RawObject};
use daft_core::init::{init, InitOptions};
use daft_core::object::{Commit, FileMode, Signature};
use daft_core::refs::ReferenceTarget;
use daft_core::Repository;
use daft_dimension::{DimensionManager, DimensionRepository};
use daft_sync::{
    ConflictLogger, CronosDaemon, EntangleDirection, EntangleEngine, SyncStrategy, WalEngine,
    WalStep, WatchedDimension,
};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::Arc;
use tempfile::TempDir;

fn setup_repo() -> (TempDir, Arc<Repository>) {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    init(&repo_root, &InitOptions::default()).unwrap();
    let repo = Arc::new(Repository::open(&repo_root.join(".dft")).unwrap());
    let manager = DimensionManager::new(repo.clone());
    let _ = manager.init();
    (tmp, repo)
}

fn commit_to_dimension(
    repo: &Arc<Repository>,
    dim_name: &str,
    files: &[(&str, &str)],
    parents: Vec<ObjectId>,
    message: &str,
) -> ObjectId {
    let dim_repo =
        DimensionRepository::for_dimension(Arc::clone(repo), dim_name).expect("open dim repo");

    let mut file_map: BTreeMap<String, (FileMode, ObjectId)> = BTreeMap::new();

    if let Some(parent_oid) = parents.first() {
        if let Ok(raw) = repo.cas().read_raw(parent_oid) {
            if let Ok(commit) = Commit::deserialize(&raw.data) {
                let _ = daft_core::diff::tree::flatten_tree(
                    repo.cas().as_ref(),
                    &commit.tree,
                    "",
                    &mut file_map,
                );
            }
        }
    }

    for (path, content) in files {
        let raw = RawObject::new(ObjectType::Blob, content.as_bytes().to_vec());
        let blob_oid = repo.cas().write_raw(&raw).expect("write blob");
        file_map.insert(path.to_string(), (FileMode::REGULAR, blob_oid));
    }

    let tree_oid = daft_core::merge::build_hierarchical_tree(repo.cas().as_ref(), &file_map)
        .expect("build hierarchical tree");

    let sig = Signature::now("Tester", "tester@daft-vcs.org");
    let commit = Commit::new(tree_oid, parents, sig.clone(), sig, message);
    let raw_commit = RawObject::new(ObjectType::Commit, commit.serialize());
    let commit_oid = repo.cas().write_raw(&raw_commit).expect("write commit");

    if dim_name == "mainline" {
        let _ = repo.refs().write_ref(
            "refs/heads/main",
            &ReferenceTarget::Direct(commit_oid),
            None,
            None,
        );
        let _ = repo.set_head(&ReferenceTarget::Direct(commit_oid));
    } else {
        dim_repo
            .set_head(&commit_oid.to_hex())
            .expect("set dim head");
    }

    let ws = dim_repo.workdir();
    let _ = fs::create_dir_all(ws);
    let mut index = dim_repo.index().unwrap_or_default();
    let _ = daft_core::merge::checkout_tree(repo.cas().as_ref(), &tree_oid, ws, &mut index);
    let _ = dim_repo.write_index(&index);

    commit_oid
}

// ============================================================================
// 1. Circular Entanglement Topology Stress (A -> B -> C -> A)
// ============================================================================

#[test]
fn test_adversarial_circular_entanglement_echo_suppression() {
    let (_tmp, repo) = setup_repo();
    let dim_mgr = DimensionManager::new(Arc::clone(&repo));

    dim_mgr.create_dimension("dim-a", None, None).unwrap();
    dim_mgr.create_dimension("dim-b", None, None).unwrap();
    dim_mgr.create_dimension("dim-c", None, None).unwrap();

    let engine = EntangleEngine::new(Arc::clone(&repo));

    // Construct circular topology: dim-a -> dim-b, dim-b -> dim-c, dim-c -> dim-a
    engine
        .link(
            "dim-a",
            "dim-b",
            Some("shared/**"),
            EntangleDirection::Unidirectional,
        )
        .unwrap();
    engine
        .link(
            "dim-b",
            "dim-c",
            Some("shared/**"),
            EntangleDirection::Unidirectional,
        )
        .unwrap();
    engine
        .link(
            "dim-c",
            "dim-a",
            Some("shared/**"),
            EntangleDirection::Unidirectional,
        )
        .unwrap();

    let rules = engine.list().unwrap();
    assert_eq!(rules.len(), 3, "All 3 circular links must be active");

    // Write initial payload in dim-a
    let a_ws = repo.dft_dir().join("dimensions/dim-a/workspace");
    fs::create_dir_all(a_ws.join("shared")).unwrap();
    let payload = "{\"version\": 1, \"topology\": \"circular\"}";
    fs::write(a_ws.join("shared/state.json"), payload).unwrap();

    // First round of propagation: should traverse A -> B -> C
    // And when C -> A runs, A already has the exact same content!
    let results = engine.propagate_all().unwrap();
    assert_eq!(results.len(), 3);

    // Verify content propagated to B and C
    let b_file = repo
        .dft_dir()
        .join("dimensions/dim-b/workspace/shared/state.json");
    let c_file = repo
        .dft_dir()
        .join("dimensions/dim-c/workspace/shared/state.json");
    assert!(b_file.exists(), "dim-b must receive state.json");
    assert!(c_file.exists(), "dim-c must receive state.json");
    assert_eq!(fs::read_to_string(&b_file).unwrap(), payload);
    assert_eq!(fs::read_to_string(&c_file).unwrap(), payload);

    // Verify that C -> A did NOT re-propagate to A (echo suppression Tier 1)
    let c_to_a_res = results.iter().find(|r| r.source_dim == "dim-c").unwrap();
    assert!(
        c_to_a_res
            .files_skipped
            .contains(&"shared/state.json".to_string()),
        "C -> A must skip propagation because A already has identical content"
    );

    // Second round of propagation without touching anything:
    // ALL 3 links must skip propagation (0 files propagated)
    let round2 = engine.propagate_all().unwrap();
    for res in round2 {
        assert!(
            res.files_propagated.is_empty(),
            "Rule {} -> {} must propagate 0 files in steady state",
            res.source_dim,
            res.target_dim
        );
        assert!(
            res.files_skipped.contains(&"shared/state.json".to_string()),
            "File must be skipped under echo cancellation"
        );
    }

    // Verify audit log has bounded entries and no runaway explosion
    let logs = engine.read_log(None).unwrap();
    // Round 1 propagated A->B and B->C (2 events). C->A was skipped (0 events).
    // Round 2 skipped all (0 events).
    assert_eq!(
        logs.len(),
        2,
        "Audit log must record exactly 2 propagation events (A->B, B->C), strictly bounded"
    );
}

// ============================================================================
// 2. Bilateral (A <-> B) Ping-Pong Echo Suppression Stress
// ============================================================================

#[test]
fn test_adversarial_bilateral_pingpong_suppression() {
    let (_tmp, repo) = setup_repo();
    let dim_mgr = DimensionManager::new(Arc::clone(&repo));

    dim_mgr.create_dimension("node-1", None, None).unwrap();
    dim_mgr.create_dimension("node-2", None, None).unwrap();

    let engine = EntangleEngine::new(Arc::clone(&repo));

    // Bilateral link
    engine
        .link(
            "node-1",
            "node-2",
            Some("sync/**"),
            EntangleDirection::Bidirectional,
        )
        .unwrap();

    let n1_ws = repo.dft_dir().join("dimensions/node-1/workspace");
    fs::create_dir_all(n1_ws.join("sync")).unwrap();
    fs::write(n1_ws.join("sync/packet.bin"), b"ping-pong-payload").unwrap();

    // Propagate node-1 -> node-2 and node-2 -> node-1
    let pass1 = engine.propagate_all().unwrap();
    assert_eq!(pass1.len(), 2);
    // Direction 1: node-1 -> node-2 propagated packet.bin
    assert_eq!(pass1[0].files_propagated, vec!["sync/packet.bin"]);
    // Direction 2: node-2 -> node-1 skipped packet.bin (echo cancellation)
    assert_eq!(pass1[1].files_skipped, vec!["sync/packet.bin"]);

    // Now re-touch the file in node-2 with the exact same content (updating timestamp only)
    let n2_file = repo
        .dft_dir()
        .join("dimensions/node-2/workspace/sync/packet.bin");
    fs::write(&n2_file, b"ping-pong-payload").unwrap();

    let pass2 = engine.propagate_all().unwrap();
    // Both directions must skip because CAS content hash matches
    for res in pass2 {
        assert!(
            res.files_propagated.is_empty(),
            "Re-touching with identical content must NOT trigger propagation"
        );
        assert_eq!(res.files_skipped, vec!["sync/packet.bin"]);
    }
}

// ============================================================================
// 3. Glob Filtering Across Additions and Subdirectories
// ============================================================================

#[test]
fn test_adversarial_glob_filter_path_isolation() {
    let (_tmp, repo) = setup_repo();
    let dim_mgr = DimensionManager::new(Arc::clone(&repo));

    dim_mgr.create_dimension("producer", None, None).unwrap();
    dim_mgr.create_dimension("consumer", None, None).unwrap();

    let engine = EntangleEngine::new(Arc::clone(&repo));

    // Filter allows schemas/**/*.json and models/*.rs
    engine
        .link(
            "producer",
            "consumer",
            Some("schemas/**/*.json, models/*.rs"),
            EntangleDirection::Unidirectional,
        )
        .unwrap();

    let prod_ws = repo.dft_dir().join("dimensions/producer/workspace");
    fs::create_dir_all(prod_ws.join("schemas/core/v1")).unwrap();
    fs::create_dir_all(prod_ws.join("models/nested")).unwrap();
    fs::create_dir_all(prod_ws.join("docs")).unwrap();
    fs::create_dir_all(prod_ws.join("schemas/data")).unwrap();

    // Files that MATCH
    fs::write(
        prod_ws.join("schemas/core/v1/user.json"),
        "{\"type\": \"user\"}",
    )
    .unwrap();
    fs::write(prod_ws.join("models/order.rs"), "pub struct Order;").unwrap();

    // Files that DO NOT match
    fs::write(prod_ws.join("docs/readme.md"), "# Documentation").unwrap();
    fs::write(prod_ws.join("schemas/data/sample.csv"), "id,name\n1,alice").unwrap();
    fs::write(prod_ws.join("models/nested/deep.rs"), "pub struct Deep;").unwrap(); // models/*.rs does not match nested

    let res = engine.propagate_all().unwrap();
    let cons_ws = repo.dft_dir().join("dimensions/consumer/workspace");

    // Matched files must exist in consumer
    assert!(cons_ws.join("schemas/core/v1/user.json").exists());
    assert!(cons_ws.join("models/order.rs").exists());

    // Non-matched files must NOT exist in consumer
    assert!(!cons_ws.join("docs/readme.md").exists());
    assert!(!cons_ws.join("schemas/data/sample.csv").exists());
    assert!(!cons_ws.join("models/nested/deep.rs").exists());

    assert_eq!(res[0].files_propagated.len(), 2);
}

// ============================================================================
// 4. Deletion Resuscitation Finding (Entanglement Edge Case)
// ============================================================================

#[test]
fn test_adversarial_entangle_deletion_behavior() {
    let (_tmp, repo) = setup_repo();
    let dim_mgr = DimensionManager::new(Arc::clone(&repo));

    dim_mgr.create_dimension("dim-left", None, None).unwrap();
    dim_mgr.create_dimension("dim-right", None, None).unwrap();

    let engine = EntangleEngine::new(Arc::clone(&repo));
    engine
        .link(
            "dim-left",
            "dim-right",
            None,
            EntangleDirection::Bidirectional,
        )
        .unwrap();

    let left_ws = repo.dft_dir().join("dimensions/dim-left/workspace");
    let right_ws = repo.dft_dir().join("dimensions/dim-right/workspace");

    fs::write(left_ws.join("shared_contract.txt"), "important contract").unwrap();
    engine.propagate_all().unwrap();

    assert!(right_ws.join("shared_contract.txt").exists());

    // Adversarial step: delete file in dim-left
    fs::remove_file(left_ws.join("shared_contract.txt")).unwrap();
    assert!(!left_ws.join("shared_contract.txt").exists());

    // Propagate again
    let _res = engine.propagate_all().unwrap();

    // Remediation verification:
    // Deletion is cleanly reconciled without zombie resurrection
    assert!(
        !left_ws.join("shared_contract.txt").exists(),
        "File must not be resurrected in dim-left"
    );
    assert!(
        !right_ws.join("shared_contract.txt").exists(),
        "Deletion must cleanly propagate to dim-right"
    );
}

// ============================================================================
// 5. Simultaneous Cross-Dimension Modifications & Blind Overwrite
// ============================================================================

#[test]
fn test_adversarial_simultaneous_modifications_and_conflict_gap() {
    let (_tmp, repo) = setup_repo();
    let dim_mgr = DimensionManager::new(Arc::clone(&repo));

    dim_mgr.create_dimension("branch-x", None, None).unwrap();
    dim_mgr.create_dimension("branch-y", None, None).unwrap();

    let engine = EntangleEngine::new(Arc::clone(&repo));
    engine
        .link(
            "branch-x",
            "branch-y",
            None,
            EntangleDirection::Bidirectional,
        )
        .unwrap();

    let x_ws = repo.dft_dir().join("dimensions/branch-x/workspace");
    let y_ws = repo.dft_dir().join("dimensions/branch-y/workspace");

    // Both dimensions concurrently write divergent content to conflict.txt
    fs::write(x_ws.join("conflict.txt"), "divergent edit by branch-x").unwrap();
    fs::write(y_ws.join("conflict.txt"), "divergent edit by branch-y").unwrap();

    // Propagate
    let res = engine.propagate_all().unwrap();

    // Verify conflict detected in result
    let has_conflict = res
        .iter()
        .any(|r| r.conflicts.contains(&"conflict.txt".to_string()));
    assert!(
        has_conflict,
        "Conflict must be reported in PropagationResult"
    );

    // Verify conflict recorded in Cronos sync.log
    let cronos_dir = repo.dft_dir().join("cronos");
    let conflict_records = ConflictLogger::read_entries(&cronos_dir, None).unwrap();
    assert!(
        !conflict_records.is_empty(),
        "ConflictLogger must record the conflict incident in sync.log"
    );
    assert_eq!(conflict_records[0].action, "conflict_detected");

    // Verify conflict recorded in entangle log.jsonl
    let entangle_logs = engine.read_log(None).unwrap();
    assert!(
        entangle_logs.iter().any(|e| e.action == "conflict"),
        "Audit log in log.jsonl must record conflict event"
    );

    // Verify target file content preserves edits via conflict markers or backup
    let y_content = fs::read_to_string(y_ws.join("conflict.txt")).unwrap();
    assert!(
        y_content.contains("branch-y") || y_ws.join("conflict.txt.conflict.branch-x").exists(),
        "Target modifications must be preserved via conflict markers or backup"
    );
}

// ============================================================================
// 6. Cronos WAL Crash Resilience & Recovery
// ============================================================================

#[test]
fn test_adversarial_cronos_wal_crash_recovery_scenarios() {
    let (_tmp, repo) = setup_repo();
    let cronos_dir = repo.dft_dir().join("cronos");
    let wal = WalEngine::new(&cronos_dir);

    // Scenario 1: Sudden crash after TxBegin (zero mutations committed)
    let tx1 = "tx-crash-early";
    wal.append(
        tx1,
        WalStep::TxBegin {
            source_dim: "dim-1".into(),
            target_dim: "dim-2".into(),
            strategy: SyncStrategy::Merge,
            source_head: "h1".into(),
            target_head: "h2".into(),
        },
    )
    .unwrap();

    // Scenario 2: Sudden crash after partial mutation (HeadUpdated but NOT WorkspaceUpdated)
    let tx2 = "tx-crash-mid-mutation";
    wal.append(
        tx2,
        WalStep::TxBegin {
            source_dim: "dim-1".into(),
            target_dim: "dim-2".into(),
            strategy: SyncStrategy::Merge,
            source_head: "h1".into(),
            target_head: "h2".into(),
        },
    )
    .unwrap();
    wal.append(
        tx2,
        WalStep::HeadUpdated {
            target_dim: "dim-2".into(),
            old_oid: "h2".into(),
            new_oid: "h3".into(),
        },
    )
    .unwrap();

    // Scenario 3: Complete mutation but crash right before TxCommit was written
    let tx3 = "tx-crash-pre-commit";
    wal.append(
        tx3,
        WalStep::TxBegin {
            source_dim: "dim-1".into(),
            target_dim: "dim-2".into(),
            strategy: SyncStrategy::Merge,
            source_head: "h1".into(),
            target_head: "h2".into(),
        },
    )
    .unwrap();
    wal.append(
        tx3,
        WalStep::HeadUpdated {
            target_dim: "dim-2".into(),
            old_oid: "h2".into(),
            new_oid: "h4".into(),
        },
    )
    .unwrap();
    wal.append(tx3, WalStep::WorkspaceUpdated { files_updated: 5 })
        .unwrap();

    // Scenario 4: Already committed transaction (should be untouched)
    let tx4 = "tx-already-committed";
    wal.append(
        tx4,
        WalStep::TxBegin {
            source_dim: "dim-1".into(),
            target_dim: "dim-2".into(),
            strategy: SyncStrategy::Merge,
            source_head: "h1".into(),
            target_head: "h2".into(),
        },
    )
    .unwrap();
    wal.append(
        tx4,
        WalStep::TxCommit {
            completion_time: Utc::now(),
        },
    )
    .unwrap();

    // Execute Recovery
    let report = wal.recover().unwrap();

    assert_eq!(
        report.transactions_analyzed, 4,
        "Must analyze all 4 distinct transactions"
    );
    assert_eq!(
        report.recovered_transactions, 1,
        "Only tx3 (complete mutations) should be recovered with TxCommit"
    );
    assert_eq!(
        report.rolled_back, 2,
        "tx1 and tx2 must be aborted/rolled back"
    );

    // Verify WAL history is valid and can be read without errors
    let all_records = wal.read_all().unwrap();
    assert_eq!(all_records.len(), 8 + 3); // 8 initial records + 1 commit (tx3) + 2 aborts (tx1, tx2) = 11 records

    // Verify idempotent re-recovery: second recovery must do nothing
    let report2 = wal.recover().unwrap();
    assert_eq!(report2.transactions_analyzed, 4);
    assert_eq!(report2.recovered_transactions, 0);
    assert_eq!(report2.rolled_back, 0);
}

// ============================================================================
// 7. WAL Truncated Record / Partial Write Resilience
// ============================================================================

#[test]
fn test_adversarial_wal_corrupted_trailing_record() {
    let (_tmp, repo) = setup_repo();
    let cronos_dir = repo.dft_dir().join("cronos");
    let wal = WalEngine::new(&cronos_dir);

    // Write a valid record
    wal.append(
        "tx-valid",
        WalStep::TxBegin {
            source_dim: "a".into(),
            target_dim: "b".into(),
            strategy: SyncStrategy::Merge,
            source_head: "h1".into(),
            target_head: "h2".into(),
        },
    )
    .unwrap();

    // Corrupt WAL file by appending an incomplete/truncated JSON line (simulating power loss during write)
    let wal_path = cronos_dir.join("wal.log");
    let mut f = OpenOptions::new().append(true).open(&wal_path).unwrap();
    write!(
        f,
        "{{\"seq\":12345,\"tx_id\":\"tx-corrupt\",\"step\":{{\"type\":\"tx_begin\""
    )
    .unwrap(); // truncated!

    // Verify wal.read_all() does not panic and safely parses valid lines
    let records = wal.read_all().unwrap();
    assert_eq!(
        records.len(),
        1,
        "Should parse the valid record and skip the corrupt trailing fragment"
    );
    assert_eq!(records[0].tx_id, "tx-valid");
}

// ============================================================================
// 8. Cronos Daemon Advisory Lock & Concurrent Startup Gap Analysis
// ============================================================================

#[test]
fn test_adversarial_daemon_advisory_lock_and_zombie_state() {
    let (_tmp, repo) = setup_repo();
    let daemon = CronosDaemon::new(Arc::clone(&repo));

    // Verify start works
    let info = daemon.start(None, None).unwrap();
    assert_eq!(info.status, "running");

    // Lock file must exist while daemon is running
    let lock_file = repo.dft_dir().join("cronos/daemon.lock");
    assert!(
        lock_file.exists(),
        "daemon.lock must exist while daemon is running"
    );

    // Attempt concurrent start in same or second manager
    let daemon2 = CronosDaemon::new(Arc::clone(&repo));
    let err = daemon2.start(None, None);
    assert!(
        err.is_err(),
        "Second daemon startup must fail with DaemonAlreadyRunning"
    );

    // Stop daemon
    daemon.stop().unwrap();

    assert!(
        !lock_file.exists(),
        "daemon.lock must be cleaned up after daemon.stop()"
    );

    // Zombie state observation:
    // If state.json has 'running' but process has died:
    let state_file = repo.dft_dir().join("cronos/state.json");
    fs::write(&state_file, "{\"status\": \"running\", \"pid\": 99999999}").unwrap();

    let status = daemon.status().unwrap();
    assert!(
        !status.is_running,
        "is_running() must return false on dead pid even if state.json contains 'running'"
    );
    assert_eq!(
        status.pid, None,
        "PID is correctly None because pid 99999999 is dead"
    );
}

// ============================================================================
// 9. 5-Dimension Circular Ring Under Multi-File Load (50 Files)
// ============================================================================

#[test]
fn test_adversarial_5_dimension_ring_heavy_load() {
    let (_tmp, repo) = setup_repo();
    let dim_mgr = DimensionManager::new(Arc::clone(&repo));

    let dims = ["ring-1", "ring-2", "ring-3", "ring-4", "ring-5"];
    for d in &dims {
        dim_mgr.create_dimension(d, None, None).unwrap();
    }

    let engine = EntangleEngine::new(Arc::clone(&repo));

    // Connect ring: 1->2, 2->3, 3->4, 4->5, 5->1
    for i in 0..5 {
        let next = (i + 1) % 5;
        engine
            .link(
                dims[i],
                dims[next],
                Some("assets/**"),
                EntangleDirection::Unidirectional,
            )
            .unwrap();
    }

    // Populate 50 files in ring-1
    let r1_ws = repo.dft_dir().join("dimensions/ring-1/workspace");
    fs::create_dir_all(r1_ws.join("assets")).unwrap();
    for i in 0..50 {
        fs::write(
            r1_ws.join(format!("assets/asset_{:03}.dat", i)),
            format!("heavy-asset-content-{}", i),
        )
        .unwrap();
    }

    // Propagation pass around the ring
    // In pass 1, because rules are evaluated sequentially in topological order:
    // 1->2 propagates 50 files, which enables 2->3 to propagate 50 files, and so on!
    let pass1 = engine.propagate_all().unwrap();
    assert_eq!(pass1.len(), 5);
    assert_eq!(
        pass1[0].files_propagated.len(),
        50,
        "1->2 propagates 50 files"
    );
    assert_eq!(
        pass1[1].files_propagated.len(),
        50,
        "2->3 propagates 50 files"
    );
    assert_eq!(
        pass1[2].files_propagated.len(),
        50,
        "3->4 propagates 50 files"
    );
    assert_eq!(
        pass1[3].files_propagated.len(),
        50,
        "4->5 propagates 50 files"
    );
    // 5->1: ring-1 already has all 50 identical files, so echo suppression skips them
    assert_eq!(
        pass1[4].files_propagated.len(),
        0,
        "5->1 propagates 0 files"
    );
    assert_eq!(
        pass1[4].files_skipped.len(),
        50,
        "5->1 skips 50 files via echo suppression"
    );

    // Verify all 50 files exist in all 5 rings
    for d in &dims {
        let ws = repo.dft_dir().join(format!("dimensions/{}/workspace", d));
        for i in 0..50 {
            assert!(
                ws.join(format!("assets/asset_{:03}.dat", i)).exists(),
                "Asset {} must exist in dimension {}",
                i,
                d
            );
        }
    }

    // Steady state check: pass 6 must propagate 0 files across all 5 links
    let pass6 = engine.propagate_all().unwrap();
    for res in pass6 {
        assert!(
            res.files_propagated.is_empty(),
            "Steady state ring must not propagate any files"
        );
        assert_eq!(res.files_skipped.len(), 50);
    }
}

// ============================================================================
// 10. Multi-Threaded Concurrent Propagation Stress
// ============================================================================

#[test]
fn test_adversarial_concurrent_propagation_threads() {
    let (_tmp, repo) = setup_repo();
    let dim_mgr = DimensionManager::new(Arc::clone(&repo));

    dim_mgr.create_dimension("conc-a", None, None).unwrap();
    dim_mgr.create_dimension("conc-b", None, None).unwrap();

    let engine = EntangleEngine::new(Arc::clone(&repo));
    engine
        .link("conc-a", "conc-b", None, EntangleDirection::Bidirectional)
        .unwrap();

    let a_ws = repo.dft_dir().join("dimensions/conc-a/workspace");
    for i in 0..20 {
        fs::write(a_ws.join(format!("item_{}.txt", i)), format!("val_{}", i)).unwrap();
    }

    // Launch 4 threads racing to call propagate_all simultaneously
    let mut handles = Vec::new();
    for _ in 0..4 {
        let repo_clone = Arc::clone(&repo);
        handles.push(std::thread::spawn(move || {
            let eng = EntangleEngine::new(repo_clone);
            eng.propagate_all()
        }));
    }

    for h in handles {
        let res = h.join().expect("Thread must not panic");
        assert!(
            res.is_ok(),
            "Concurrent propagation must succeed without I/O panic"
        );
    }

    // Verify consistency: conc-b has all 20 items intact
    let b_ws = repo.dft_dir().join("dimensions/conc-b/workspace");
    for i in 0..20 {
        assert!(b_ws.join(format!("item_{}.txt", i)).exists());
        assert_eq!(
            fs::read_to_string(b_ws.join(format!("item_{}.txt", i))).unwrap(),
            format!("val_{}", i)
        );
    }
}

// ============================================================================
// 11. Cronos Watched Dimension 3-Way Tree Conflict & Logging Stress
// ============================================================================

#[test]
fn test_adversarial_cronos_watched_dimension_3way_conflict_stress() {
    let (_tmp, repo) = setup_repo();
    let dim_mgr = DimensionManager::new(Arc::clone(&repo));

    dim_mgr.create_dimension("dim-src", None, None).unwrap();
    dim_mgr.create_dimension("dim-tgt", None, None).unwrap();

    // 1. Create common ancestor (LCA) commit on dim-tgt
    let base_oid = commit_to_dimension(
        &repo,
        "dim-tgt",
        &[("shared.txt", "common base line 1\ncommon base line 2\n")],
        vec![],
        "Initial base commit",
    );

    // Set dim-src to point to the same base commit
    let src_dim_repo = DimensionRepository::for_dimension(Arc::clone(&repo), "dim-src").unwrap();
    src_dim_repo.set_head(&base_oid.to_hex()).unwrap();
    let ws = src_dim_repo.workdir();
    let mut idx = src_dim_repo.index().unwrap_or_default();
    let raw = repo.cas().read_raw(&base_oid).unwrap();
    let base_commit = Commit::deserialize(&raw.data).unwrap();
    daft_core::merge::checkout_tree(repo.cas().as_ref(), &base_commit.tree, ws, &mut idx).unwrap();
    src_dim_repo.write_index(&idx).unwrap();

    // 2. Create divergent commit on dim-tgt: modify shared.txt
    let tgt_oid = commit_to_dimension(
        &repo,
        "dim-tgt",
        &[(
            "shared.txt",
            "common base line 1\nTARGET DIVERGENT CONTENT\n",
        )],
        vec![base_oid],
        "Target branch commit",
    );

    // 3. Create divergent commit on dim-src: modify shared.txt differently
    let src_oid = commit_to_dimension(
        &repo,
        "dim-src",
        &[(
            "shared.txt",
            "common base line 1\nSOURCE DIVERGENT CONTENT\n",
        )],
        vec![base_oid],
        "Source branch commit",
    );

    assert_ne!(src_oid, tgt_oid, "Heads must be divergent");

    // 4. Configure Cronos to watch dim-src -> dim-tgt with Merge strategy
    let daemon = CronosDaemon::new(Arc::clone(&repo));
    let mut cfg = daemon.get_config().unwrap();
    cfg.auto_entangle = false;
    cfg.watched_dimensions = vec![WatchedDimension {
        dimension: "dim-src".to_string(),
        target: "dim-tgt".to_string(),
        strategy: Some(SyncStrategy::Merge),
        interval_secs: None,
        paths: vec![],
        paused: false,
        last_synced_at: None,
        last_synced_commit: None,
    }];
    daemon.set_config(cfg).unwrap();

    // 5. Execute run_tick()
    let tick_result = daemon.run_tick().unwrap();

    // 6. Verify 3-way conflict results
    assert_eq!(
        tick_result.synced_dimensions, 0,
        "Conflicted sync must not count as successfully synced dimension"
    );
    assert!(
        tick_result.conflicts_detected > 0,
        "conflicts_detected must be strictly greater than 0, got {}",
        tick_result.conflicts_detected
    );
    assert_eq!(
        tick_result.conflicts_detected, 1,
        "Exactly 1 file conflict expected on shared.txt"
    );

    // 7. Verify sync.log contains the conflict via ConflictLogger
    let cronos_dir = repo.dft_dir().join("cronos");
    let sync_logs = ConflictLogger::read_entries(&cronos_dir, None).unwrap();
    assert!(
        !sync_logs.is_empty(),
        "sync.log must contain at least one log entry"
    );

    let conflict_entry = sync_logs.iter().find(|e| e.action == "conflict").unwrap();
    assert_eq!(conflict_entry.level, "WARN");
    assert_eq!(conflict_entry.source_dim, "dim-src");
    assert_eq!(conflict_entry.target_dim, "dim-tgt");
    assert_eq!(conflict_entry.conflicts.len(), 1);
    assert_eq!(conflict_entry.conflicts[0].path, "shared.txt");

    // 8. Verify WAL recorded TxBegin and TxAbort
    let wal = WalEngine::new(&cronos_dir);
    let wal_records = wal.read_all().unwrap();
    let has_abort = wal_records.iter().any(|r| match &r.step {
        WalStep::TxAbort { reason } => reason.contains("Merge conflicts detected"),
        _ => false,
    });
    assert!(has_abort, "WAL must record TxAbort on conflict");

    // 9. Verify target HEAD remains protected and unchanged
    let tgt_dim_repo = DimensionRepository::for_dimension(Arc::clone(&repo), "dim-tgt").unwrap();
    assert_eq!(
        tgt_dim_repo.head_commit(),
        Some(tgt_oid),
        "Target HEAD must not be corrupted or advanced when conflict occurs"
    );
}

// ============================================================================
// 12. Cronos Watched Dimension Auto-Resolution & Clean 3-Way Merge
// ============================================================================

#[test]
fn test_adversarial_cronos_watched_dimension_theirs_resolution_stress() {
    let (_tmp, repo) = setup_repo();
    let dim_mgr = DimensionManager::new(Arc::clone(&repo));

    dim_mgr.create_dimension("dim-src", None, None).unwrap();
    dim_mgr.create_dimension("dim-tgt", None, None).unwrap();

    let base_oid = commit_to_dimension(
        &repo,
        "dim-tgt",
        &[("shared.txt", "base line\n")],
        vec![],
        "Initial base commit",
    );

    let src_dim_repo = DimensionRepository::for_dimension(Arc::clone(&repo), "dim-src").unwrap();
    src_dim_repo.set_head(&base_oid.to_hex()).unwrap();

    // Divergent commits
    commit_to_dimension(
        &repo,
        "dim-tgt",
        &[("shared.txt", "target line\n")],
        vec![base_oid],
        "Target commit",
    );
    let _src_oid = commit_to_dimension(
        &repo,
        "dim-src",
        &[("shared.txt", "theirs victorious line\n")],
        vec![base_oid],
        "Source commit",
    );

    // Configure Cronos with Theirs strategy
    let daemon = CronosDaemon::new(Arc::clone(&repo));
    let mut cfg = daemon.get_config().unwrap();
    cfg.auto_entangle = false;
    cfg.watched_dimensions = vec![WatchedDimension {
        dimension: "dim-src".to_string(),
        target: "dim-tgt".to_string(),
        strategy: Some(SyncStrategy::Theirs),
        interval_secs: None,
        paths: vec![],
        paused: false,
        last_synced_at: None,
        last_synced_commit: None,
    }];
    daemon.set_config(cfg).unwrap();

    let tick_result = daemon.run_tick().unwrap();
    assert_eq!(tick_result.synced_dimensions, 1);
    assert_eq!(tick_result.conflicts_detected, 1);

    // Verify target workspace now has the source version
    let tgt_dim_repo = DimensionRepository::for_dimension(Arc::clone(&repo), "dim-tgt").unwrap();
    let target_file = tgt_dim_repo.workdir().join("shared.txt");
    assert_eq!(
        fs::read_to_string(target_file).unwrap(),
        "theirs victorious line\n"
    );

    // Verify sync.log has auto_resolved entry
    let cronos_dir = repo.dft_dir().join("cronos");
    let sync_logs = ConflictLogger::read_entries(&cronos_dir, None).unwrap();
    let resolved_entry = sync_logs
        .iter()
        .find(|e| e.action == "auto_resolved")
        .unwrap();
    assert_eq!(resolved_entry.strategy, "theirs");
    assert_eq!(resolved_entry.conflicts.len(), 1);
}

// ============================================================================
// 13. POSIX Advisory Kernel Locking & Process Liveness Stress
// ============================================================================

#[test]
fn test_adversarial_posix_advisory_flock_concurrency_stress() {
    let (_tmp, repo) = setup_repo();
    let daemon = CronosDaemon::new(Arc::clone(&repo));

    let lock_path = repo.dft_dir().join("cronos/daemon.lock");
    fs::create_dir_all(repo.dft_dir().join("cronos")).unwrap();

    // 1. Manually open lock_path on a separate file descriptor and acquire exclusive lock
    let external_lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&lock_path)
        .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let ret = unsafe {
            libc::flock(
                external_lock_file.as_raw_fd(),
                libc::LOCK_EX | libc::LOCK_NB,
            )
        };
        assert_eq!(
            ret, 0,
            "External descriptor must successfully acquire initial exclusive lock"
        );

        // Now attempt to start the daemon: should fail because lock is already held
        let err = daemon.start(None, None);
        assert!(
            err.is_err(),
            "Daemon start must fail when flock is held by another descriptor/process"
        );

        // Release the external lock
        let unlock_ret = unsafe { libc::flock(external_lock_file.as_raw_fd(), libc::LOCK_UN) };
        assert_eq!(unlock_ret, 0, "Unlock must succeed");
    }

    // 2. Now start the daemon normally
    let info = daemon
        .start(None, None)
        .expect("Daemon start must succeed when lock is free");
    assert_eq!(info.status, "running");
    assert!(
        daemon.is_running(),
        "is_running() must return true while daemon is active"
    );

    // 3. While daemon is running, another descriptor attempting LOCK_EX | LOCK_NB must be rejected
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let competing_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        let ret = unsafe { libc::flock(competing_file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        assert_ne!(
            ret, 0,
            "Second descriptor must be rejected by flock(LOCK_EX | LOCK_NB)"
        );
    }

    // 4. Concurrent manager instance must also be rejected
    let daemon2 = CronosDaemon::new(Arc::clone(&repo));
    let err2 = daemon2.start(None, None);
    assert!(
        err2.is_err(),
        "Second daemon instance must fail with DaemonAlreadyRunning"
    );

    // 5. Stopping daemon cleanly releases lock and removes lock file
    let stop_res = daemon.stop().expect("Daemon stop must succeed");
    assert!(stop_res.success);
    assert!(
        !daemon.is_running(),
        "is_running() must return false after stop"
    );
    assert!(!lock_path.exists(), "daemon.lock must be removed on stop");

    // 6. After stop, another descriptor can re-acquire lock without error
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let recheck_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&lock_path)
            .unwrap();
        let ret = unsafe { libc::flock(recheck_file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        assert_eq!(ret, 0, "Lock can be acquired again after daemon stopped");
        let _ = unsafe { libc::flock(recheck_file.as_raw_fd(), libc::LOCK_UN) };
        let _ = fs::remove_file(&lock_path);
    }

    // 7. Zombie status elimination: dead PID in state.json must report is_running: false
    let state_file = repo.dft_dir().join("cronos/state.json");
    let pid_file = repo.dft_dir().join("cronos/daemon.pid");
    fs::write(&state_file, "{\"status\": \"running\", \"pid\": 88888888}").unwrap();
    fs::write(&pid_file, "88888888\n").unwrap();

    let status = daemon.status().unwrap();
    assert!(
        !status.is_running,
        "is_running must be false when PID is dead"
    );
    assert_eq!(status.pid, None, "PID must be None when process is dead");
}

// ============================================================================
// 14. Deep Entanglement Deletion Resuscitation & Re-Creation Stress
// ============================================================================

#[test]
fn test_adversarial_entanglement_deletion_resuscitation_deep_stress() {
    let (_tmp, repo) = setup_repo();
    let dim_mgr = DimensionManager::new(Arc::clone(&repo));

    dim_mgr.create_dimension("dim-alpha", None, None).unwrap();
    dim_mgr.create_dimension("dim-beta", None, None).unwrap();

    let engine = EntangleEngine::new(Arc::clone(&repo));
    engine
        .link(
            "dim-alpha",
            "dim-beta",
            None,
            EntangleDirection::Bidirectional,
        )
        .unwrap();

    let a_ws = repo.dft_dir().join("dimensions/dim-alpha/workspace");
    let b_ws = repo.dft_dir().join("dimensions/dim-beta/workspace");

    // 1. Create a suite of nested files in dim-alpha
    let files = [
        "docs/spec.md",
        "src/lib.rs",
        "src/utils.rs",
        "data/config.json",
    ];
    for f in &files {
        let p = a_ws.join(f);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, format!("content for {}", f)).unwrap();
    }

    // 2. Propagate dim-alpha -> dim-beta
    let res1 = engine.propagate_all().unwrap();
    assert_eq!(res1[0].files_propagated.len(), 4);
    for f in &files {
        assert!(b_ws.join(f).exists(), "File {} must exist in dim-beta", f);
    }

    // 3. Delete docs/spec.md and src/utils.rs in dim-alpha
    fs::remove_file(a_ws.join("docs/spec.md")).unwrap();
    fs::remove_file(a_ws.join("src/utils.rs")).unwrap();
    assert!(!a_ws.join("docs/spec.md").exists());
    assert!(!a_ws.join("src/utils.rs").exists());

    // 4. Run propagation (Pass 1 of deletion)
    let _res2 = engine.propagate_all().unwrap();

    // Assert neither deleted file was resurrected in dim-alpha!
    assert!(
        !a_ws.join("docs/spec.md").exists(),
        "docs/spec.md MUST NOT be resurrected in dim-alpha"
    );
    assert!(
        !a_ws.join("src/utils.rs").exists(),
        "src/utils.rs MUST NOT be resurrected in dim-alpha"
    );

    // Assert deletion propagated cleanly to dim-beta!
    assert!(
        !b_ws.join("docs/spec.md").exists(),
        "docs/spec.md MUST be deleted in dim-beta"
    );
    assert!(
        !b_ws.join("src/utils.rs").exists(),
        "src/utils.rs MUST be deleted in dim-beta"
    );

    // Non-deleted files must remain intact in both dimensions
    assert!(a_ws.join("src/lib.rs").exists());
    assert!(b_ws.join("src/lib.rs").exists());
    assert!(a_ws.join("data/config.json").exists());
    assert!(b_ws.join("data/config.json").exists());

    // 5. Run second steady-state propagation: 0 files propagated, no zombies
    let res3 = engine.propagate_all().unwrap();
    for r in res3 {
        assert!(
            r.files_propagated.is_empty(),
            "Steady state after deletion must propagate 0 files"
        );
    }
    assert!(!a_ws.join("docs/spec.md").exists());
    assert!(!b_ws.join("docs/spec.md").exists());

    // 6. Reverse direction deletion: delete src/lib.rs in dim-beta!
    fs::remove_file(b_ws.join("src/lib.rs")).unwrap();
    assert!(!b_ws.join("src/lib.rs").exists());

    let _res4 = engine.propagate_all().unwrap();
    assert!(
        !b_ws.join("src/lib.rs").exists(),
        "src/lib.rs MUST NOT be resurrected in dim-beta"
    );
    assert!(
        !a_ws.join("src/lib.rs").exists(),
        "src/lib.rs deletion in beta MUST propagate to alpha"
    );

    // 7. Re-creation test: re-create docs/spec.md in dim-alpha with new content
    fs::write(a_ws.join("docs/spec.md"), "# Brand New Spec v2\n").unwrap();
    let _res5 = engine.propagate_all().unwrap();
    assert!(
        b_ws.join("docs/spec.md").exists(),
        "Re-created file in alpha must cleanly propagate to beta"
    );
    assert_eq!(
        fs::read_to_string(b_ws.join("docs/spec.md")).unwrap(),
        "# Brand New Spec v2\n"
    );
}

// ============================================================================
// 15. Cronos Clean 3-Way Tree Convergence Across Non-Conflicting Dimensions
// ============================================================================

#[test]
fn test_adversarial_cronos_watched_dimension_clean_3way_convergence() {
    let (_tmp, repo) = setup_repo();
    let dim_mgr = DimensionManager::new(Arc::clone(&repo));

    dim_mgr.create_dimension("feat-left", None, None).unwrap();
    dim_mgr.create_dimension("feat-right", None, None).unwrap();

    // Base commit on feat-right
    let base_oid = commit_to_dimension(
        &repo,
        "feat-right",
        &[("base.txt", "common base content\n")],
        vec![],
        "Base commit",
    );

    let left_repo = DimensionRepository::for_dimension(Arc::clone(&repo), "feat-left").unwrap();
    left_repo.set_head(&base_oid.to_hex()).unwrap();

    // Disjoint divergent commits
    let right_oid = commit_to_dimension(
        &repo,
        "feat-right",
        &[("right_mod.txt", "right feature added\n")],
        vec![base_oid],
        "Right feature commit",
    );

    let left_oid = commit_to_dimension(
        &repo,
        "feat-left",
        &[("left_mod.txt", "left feature added\n")],
        vec![base_oid],
        "Left feature commit",
    );

    // Watch feat-left -> feat-right with Merge strategy
    let daemon = CronosDaemon::new(Arc::clone(&repo));
    let mut cfg = daemon.get_config().unwrap();
    cfg.auto_entangle = false;
    cfg.watched_dimensions = vec![WatchedDimension {
        dimension: "feat-left".to_string(),
        target: "feat-right".to_string(),
        strategy: Some(SyncStrategy::Merge),
        interval_secs: None,
        paths: vec![],
        paused: false,
        last_synced_at: None,
        last_synced_commit: None,
    }];
    daemon.set_config(cfg).unwrap();

    let tick_res = daemon.run_tick().unwrap();
    assert_eq!(tick_res.synced_dimensions, 1);
    assert_eq!(tick_res.conflicts_detected, 0);

    // Verify feat-right HEAD advanced to a 2-parent merge commit
    let right_repo = DimensionRepository::for_dimension(Arc::clone(&repo), "feat-right").unwrap();
    let new_head = right_repo.head_commit().unwrap();
    assert_ne!(new_head, right_oid);
    assert_ne!(new_head, left_oid);

    let raw_merge = repo.cas().read_raw(&new_head).unwrap();
    let merge_commit = Commit::deserialize(&raw_merge.data).unwrap();
    assert_eq!(merge_commit.parents, vec![right_oid, left_oid]);

    // Verify workspace contains all 3 files
    let ws = right_repo.workdir();
    assert!(ws.join("base.txt").exists());
    assert!(ws.join("right_mod.txt").exists());
    assert!(ws.join("left_mod.txt").exists());

    // Verify WAL has complete committed lifecycle
    let cronos_dir = repo.dft_dir().join("cronos");
    let wal = WalEngine::new(&cronos_dir);
    let records = wal.read_all().unwrap();
    assert!(records
        .iter()
        .any(|r| matches!(r.step, WalStep::TreePrepared { .. })));
    assert!(records
        .iter()
        .any(|r| matches!(r.step, WalStep::CommitCreated { .. })));
    assert!(records
        .iter()
        .any(|r| matches!(r.step, WalStep::HeadUpdated { .. })));
    assert!(records
        .iter()
        .any(|r| matches!(r.step, WalStep::WorkspaceUpdated { .. })));
    assert!(records
        .iter()
        .any(|r| matches!(r.step, WalStep::TxCommit { .. })));
}

// ============================================================================
// 16. POSIX Advisory Lock Inter-Thread / Multi-Handle Race Stress
// ============================================================================

#[test]
fn test_adversarial_daemon_concurrent_flock_thread_race() {
    let (_tmp, repo) = setup_repo();
    let daemon = CronosDaemon::new(Arc::clone(&repo));

    daemon
        .start(None, None)
        .expect("Primary daemon start must succeed");
    assert!(daemon.is_running());

    let lock_path = repo.dft_dir().join("cronos/daemon.lock");
    assert!(lock_path.exists());

    // Spawn 8 threads attempting to start the daemon concurrently or acquire flock
    let mut handles = Vec::new();
    for _ in 0..8 {
        let lp = lock_path.clone();
        let repo_clone = Arc::clone(&repo);
        handles.push(std::thread::spawn(move || {
            let d = CronosDaemon::new(repo_clone);
            let start_err = d.start(None, None);
            assert!(start_err.is_err(), "Concurrent start must be rejected");

            #[cfg(unix)]
            {
                use std::os::unix::io::AsRawFd;
                if let Ok(f) = OpenOptions::new().read(true).write(true).open(&lp) {
                    let ret = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
                    assert_ne!(
                        ret, 0,
                        "Concurrent flock acquisition must be rejected by kernel"
                    );
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Now stop the daemon
    daemon.stop().expect("Daemon stop must succeed");
    assert!(!daemon.is_running());
    assert!(!lock_path.exists());
}
