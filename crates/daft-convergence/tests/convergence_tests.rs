use daft_convergence::lock_order::MultiDimensionLockGuard;
use daft_convergence::strategy::MergeStrategy;
use daft_convergence::*;
use daft_core::init::{init, InitOptions};
use daft_dimension::DimensionManager;
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn test_lock_order_lexicographical() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);
    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();
    dim_mgr.create_dimension("zebra", None, None).unwrap();
    dim_mgr.create_dimension("alpha", None, None).unwrap();
    dim_mgr.create_dimension("beta", None, None).unwrap();

    let guard = MultiDimensionLockGuard::acquire(
        &dim_mgr,
        vec!["zebra".to_string(), "alpha".to_string(), "beta".to_string()],
    );
    assert!(guard.is_ok());
}

#[test]
fn test_merge_strategy_parsing() {
    use std::str::FromStr;
    assert_eq!(
        MergeStrategy::from_str("manual").unwrap(),
        MergeStrategy::ManualMarkers
    );
    assert_eq!(
        MergeStrategy::from_str("ours").unwrap(),
        MergeStrategy::Ours
    );
    assert_eq!(
        MergeStrategy::from_str("theirs").unwrap(),
        MergeStrategy::Theirs
    );
    assert_eq!(
        MergeStrategy::from_str("union").unwrap(),
        MergeStrategy::Union
    );
    assert!(MergeStrategy::from_str("invalid").is_err());
}

#[test]
fn test_converge_engine_lifecycle() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();
    dim_mgr.create_dimension("dim-a", None, None).unwrap();
    dim_mgr.create_dimension("dim-b", None, None).unwrap();

    let converge = ConvergeEngine::new(Arc::clone(&repo));
    let outcome = converge.converge(
        &["dim-a".to_string(), "dim-b".to_string()],
        ConvergeOptions {
            into: Some("mainline".to_string()),
            strategy: MergeStrategy::ManualMarkers,
            message: Some("Test converge".to_string()),
            no_commit: false,
        },
    );
    assert!(outcome.is_ok());
}

#[test]
fn test_collapse_engine_lifecycle() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();
    dim_mgr.create_dimension("worker-1", None, None).unwrap();

    let collapse = CollapseEngine::new(Arc::clone(&repo));
    let outcome = collapse.collapse(
        &["worker-1".to_string()],
        CollapseOptions {
            into: Some("mainline".to_string()),
            strategy: Some(MergeStrategy::Ours),
            message: Some("Test collapse".to_string()),
        },
    );
    assert!(outcome.is_ok());
}

#[test]
fn test_cascade_engine_lifecycle() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();
    dim_mgr.create_dimension("src-dim", None, None).unwrap();
    dim_mgr.create_dimension("dst-dim", None, None).unwrap();

    let cascade = CascadeEngine::new(Arc::clone(&repo));
    let outcome = cascade.cascade("src-dim", Some("dst-dim"), true);
    assert!(outcome.is_ok());
}

#[test]
fn test_weave_engine_lifecycle() {
    let dir = tempdir().unwrap();
    let repo = init(dir.path(), &InitOptions::default()).unwrap();
    let repo = Arc::new(repo);

    let empty_tree = daft_core::cas::RawObject::new(daft_core::cas::ObjectType::Tree, Vec::new());
    let tree_oid = repo.cas().write_raw(&empty_tree).unwrap();
    let sig = daft_core::merge::get_signature();
    let commit = daft_core::object::Commit::new(tree_oid, Vec::new(), sig.clone(), sig, "init");
    let raw_c =
        daft_core::cas::RawObject::new(daft_core::cas::ObjectType::Commit, commit.serialize());
    let c_oid = repo.cas().write_raw(&raw_c).unwrap();
    repo.refs()
        .write_ref(
            "refs/heads/main",
            &daft_core::refs::ReferenceTarget::Direct(c_oid),
            None,
            None,
        )
        .unwrap();

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.init().unwrap();
    dim_mgr.create_dimension("stream-1", None, None).unwrap();
    dim_mgr.create_dimension("stream-2", None, None).unwrap();

    let weave = WeaveEngine::new(Arc::clone(&repo));
    let outcome = weave.weave("stream-1", "stream-2", Some("mainline"), None);
    assert!(outcome.is_ok());
}
