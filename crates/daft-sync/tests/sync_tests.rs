use daft_core::init::{init, InitOptions};
use daft_core::Repository;
use daft_dimension::DimensionManager;
use daft_sync::{
    CronosDaemon, EntangleDirection, EntangleEngine, SyncStrategy, WalEngine, WalStep,
};
use std::fs;
use std::sync::Arc;
use std::time::Duration;
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

#[test]
fn test_entangle_rules() {
    let (_tmp, repo) = setup_repo();
    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.create_dimension("dim-a", None, None).unwrap();
    dim_mgr.create_dimension("dim-b", None, None).unwrap();

    let engine = EntangleEngine::new(Arc::clone(&repo));

    // Link dim-a and dim-b
    let rule = engine
        .link(
            "dim-a",
            "dim-b",
            Some("shared/**"),
            EntangleDirection::Bidirectional,
        )
        .unwrap();
    assert_eq!(rule.dim1, "dim-a");
    assert_eq!(rule.dim2, "dim-b");
    assert_eq!(rule.paths.as_deref(), Some("shared/**"));

    // List rules
    let list = engine.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].dim1, "dim-a");

    // Sever rule
    let severed = engine.sever("dim-a", "dim-b").unwrap();
    assert!(severed);

    let list_after = engine.list().unwrap();
    assert_eq!(list_after.len(), 0);
}

#[test]
fn test_entangle_invalid_dimension_fails() {
    let (_tmp, repo) = setup_repo();
    let engine = EntangleEngine::new(Arc::clone(&repo));

    let res = engine.link(
        "mainline",
        "ghost-dim",
        None,
        EntangleDirection::Bidirectional,
    );
    assert!(res.is_err());
}

#[test]
fn test_entangle_propagation_and_echo() {
    let (_tmp, repo) = setup_repo();
    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.create_dimension("source-dim", None, None).unwrap();
    dim_mgr.create_dimension("target-dim", None, None).unwrap();

    let engine = EntangleEngine::new(Arc::clone(&repo));
    engine
        .link(
            "source-dim",
            "target-dim",
            Some("models/**"),
            EntangleDirection::Bidirectional,
        )
        .unwrap();

    // Write file in source workspace
    let src_ws = repo.dft_dir().join("dimensions/source-dim/workspace");
    fs::create_dir_all(src_ws.join("models")).unwrap();
    fs::write(
        src_ws.join("models/user.rs"),
        "pub struct User { pub id: u64 }",
    )
    .unwrap();

    // Propagate
    let res = engine.propagate_all().unwrap();
    assert!(!res.is_empty());

    let tgt_file = repo
        .dft_dir()
        .join("dimensions/target-dim/workspace/models/user.rs");
    assert!(tgt_file.exists());
    let content = fs::read_to_string(&tgt_file).unwrap();
    assert_eq!(content, "pub struct User { pub id: u64 }");

    // Re-propagate without change: should be skipped due to echo / hash match
    let res2 = engine.propagate_all().unwrap();
    for r in res2 {
        assert!(r.files_propagated.is_empty());
        assert!(!r.files_skipped.is_empty());
    }

    // Verify audit log
    let logs = engine.read_log(None).unwrap();
    assert!(!logs.is_empty());
    assert_eq!(logs[0].path, "models/user.rs");
}

#[test]
fn test_cronos_lifecycle_and_config() {
    let (_tmp, repo) = setup_repo();
    let daemon = CronosDaemon::new(Arc::clone(&repo));

    let status_before = daemon.status().unwrap();
    assert!(!status_before.is_running);

    let info = daemon
        .start(Some(Duration::from_secs(15)), Some(SyncStrategy::Theirs))
        .unwrap();
    assert_eq!(info.status, "running");

    let status_after = daemon.status().unwrap();
    assert!(status_after.is_running);
    assert_eq!(status_after.interval_secs, 15);
    assert_eq!(status_after.default_strategy, SyncStrategy::Theirs);

    // Pause and resume dimension
    daemon.pause("test-dim").unwrap();
    let cfg = daemon.get_config().unwrap();
    let watched = cfg
        .watched_dimensions
        .iter()
        .find(|w| w.dimension == "test-dim")
        .unwrap();
    assert!(watched.paused);

    daemon.resume("test-dim").unwrap();
    let cfg2 = daemon.get_config().unwrap();
    let watched2 = cfg2
        .watched_dimensions
        .iter()
        .find(|w| w.dimension == "test-dim")
        .unwrap();
    assert!(!watched2.paused);

    // Stop daemon
    let stop_res = daemon.stop().unwrap();
    assert!(stop_res.success);

    let status_stopped = daemon.status().unwrap();
    assert!(!status_stopped.is_running);
}

#[test]
fn test_cronos_wal_and_recovery() {
    let (_tmp, repo) = setup_repo();
    let cronos_dir = repo.dft_dir().join("cronos");
    let wal = WalEngine::new(&cronos_dir);

    let tx_id = "tx-test-123";
    wal.append(
        tx_id,
        WalStep::TxBegin {
            source_dim: "dim-1".into(),
            target_dim: "dim-2".into(),
            strategy: SyncStrategy::Merge,
            source_head: "oid1".into(),
            target_head: "oid2".into(),
        },
    )
    .unwrap();

    wal.append(
        tx_id,
        WalStep::HeadUpdated {
            target_dim: "dim-2".into(),
            old_oid: "oid2".into(),
            new_oid: "oid3".into(),
        },
    )
    .unwrap();

    wal.append(tx_id, WalStep::WorkspaceUpdated { files_updated: 3 })
        .unwrap();

    // Verify recovery detects completed mutation without commit record and commits it
    let report = wal.recover().unwrap();
    assert_eq!(report.transactions_analyzed, 1);
    assert_eq!(report.recovered_transactions, 1);

    // Verify WAL integrity
    let count = wal.verify().unwrap();
    assert_eq!(count, 4); // Begin, HeadUpdated, WorkspaceUpdated, TxCommit
}
