use daft_core::cas::ObjectId;
use daft_core::error::RefError;
use daft_core::refs::{validate_ref_name, RefManager, ReferenceTarget};
use tempfile::tempdir;

#[test]
fn test_ref_write_and_read() {
    let dir = tempdir().unwrap();
    let dft_dir = dir.path().join(".dft");
    std::fs::create_dir_all(&dft_dir).unwrap();

    let mgr = RefManager::new(&dft_dir);
    let oid = ObjectId::hash(b"commit 1");

    mgr.write_ref("refs/heads/main", &ReferenceTarget::Direct(oid), None, None)
        .unwrap();

    let r = mgr.read_ref("refs/heads/main").unwrap();
    assert_eq!(r.name, "refs/heads/main");
    assert_eq!(r.target, ReferenceTarget::Direct(oid));
    assert!(r.is_direct());
}

#[test]
fn test_symbolic_head_resolution() {
    let dir = tempdir().unwrap();
    let dft_dir = dir.path().join(".dft");
    std::fs::create_dir_all(&dft_dir).unwrap();

    let mgr = RefManager::new(&dft_dir);
    let oid = ObjectId::hash(b"head commit");

    // Write branch
    mgr.write_ref(
        "refs/heads/feature",
        &ReferenceTarget::Direct(oid),
        None,
        None,
    )
    .unwrap();

    // Point HEAD to branch
    mgr.write_ref(
        "HEAD",
        &ReferenceTarget::Symbolic("refs/heads/feature".to_string()),
        None,
        None,
    )
    .unwrap();

    let resolved = mgr.resolve("HEAD").unwrap();
    assert_eq!(resolved, oid);
}

#[test]
fn test_unborn_branch_resolution() {
    let dir = tempdir().unwrap();
    let dft_dir = dir.path().join(".dft");
    std::fs::create_dir_all(&dft_dir).unwrap();

    let mgr = RefManager::new(&dft_dir);

    // Fresh repo HEAD points to unborn main
    mgr.write_ref(
        "HEAD",
        &ReferenceTarget::Symbolic("refs/heads/main".to_string()),
        None,
        None,
    )
    .unwrap();

    let res = mgr.resolve("HEAD");
    assert!(matches!(res, Err(RefError::UnbornBranch(b)) if b == "main"));
}

#[test]
fn test_symbolic_loop_detection() {
    let dir = tempdir().unwrap();
    let dft_dir = dir.path().join(".dft");
    std::fs::create_dir_all(&dft_dir).unwrap();

    let mgr = RefManager::new(&dft_dir);

    // A -> B -> A
    mgr.write_ref(
        "refs/heads/a",
        &ReferenceTarget::Symbolic("refs/heads/b".to_string()),
        None,
        None,
    )
    .unwrap();
    mgr.write_ref(
        "refs/heads/b",
        &ReferenceTarget::Symbolic("refs/heads/a".to_string()),
        None,
        None,
    )
    .unwrap();

    let res = mgr.resolve("refs/heads/a");
    assert!(matches!(res, Err(RefError::SymbolicRefLoop(_))));
}

#[test]
fn test_cas_compare_and_swap() {
    let dir = tempdir().unwrap();
    let dft_dir = dir.path().join(".dft");
    std::fs::create_dir_all(&dft_dir).unwrap();

    let mgr = RefManager::new(&dft_dir);
    let oid1 = ObjectId::hash(b"commit 1");
    let oid2 = ObjectId::hash(b"commit 2");
    let oid_wrong = ObjectId::hash(b"wrong commit");

    mgr.write_ref(
        "refs/heads/main",
        &ReferenceTarget::Direct(oid1),
        None,
        None,
    )
    .unwrap();

    // CAS failure when expected does not match
    let fail = mgr.write_ref(
        "refs/heads/main",
        &ReferenceTarget::Direct(oid2),
        Some(&ReferenceTarget::Direct(oid_wrong)),
        None,
    );
    assert!(matches!(fail, Err(RefError::CasMismatch { .. })));

    // Ref value remains oid1
    assert_eq!(
        mgr.read_ref("refs/heads/main").unwrap().target,
        ReferenceTarget::Direct(oid1)
    );

    // CAS success when expected matches
    mgr.write_ref(
        "refs/heads/main",
        &ReferenceTarget::Direct(oid2),
        Some(&ReferenceTarget::Direct(oid1)),
        None,
    )
    .unwrap();

    assert_eq!(
        mgr.read_ref("refs/heads/main").unwrap().target,
        ReferenceTarget::Direct(oid2)
    );
}

#[test]
fn test_ref_name_syntax_validation() {
    assert!(validate_ref_name("refs/heads/main").is_ok());
    assert!(validate_ref_name("refs/tags/v1.0").is_ok());
    assert!(validate_ref_name("refs/dimensions/agent-1").is_ok());

    assert!(validate_ref_name("").is_err());
    assert!(validate_ref_name("@").is_err());
    assert!(validate_ref_name("/refs/heads/main").is_err());
    assert!(validate_ref_name("refs/heads/main/").is_err());
    assert!(validate_ref_name("refs//heads").is_err());
    assert!(validate_ref_name("refs/heads/main.lock").is_err());
    assert!(validate_ref_name("refs/heads/..").is_err());
    assert!(validate_ref_name("refs/heads/a..b").is_err());
    assert!(validate_ref_name("refs/heads/@{1}").is_err());
    assert!(validate_ref_name("refs/heads/bad name").is_err());
    assert!(validate_ref_name("refs/heads/~branch").is_err());
    assert!(validate_ref_name("refs/heads/^branch").is_err());
    assert!(validate_ref_name("refs/heads/:branch").is_err());
}

#[test]
fn test_list_and_delete_refs() {
    let dir = tempdir().unwrap();
    let dft_dir = dir.path().join(".dft");
    std::fs::create_dir_all(&dft_dir).unwrap();

    let mgr = RefManager::new(&dft_dir);
    let oid = ObjectId::ZERO;

    mgr.write_ref("refs/heads/b", &ReferenceTarget::Direct(oid), None, None)
        .unwrap();
    mgr.write_ref("refs/heads/a", &ReferenceTarget::Direct(oid), None, None)
        .unwrap();
    mgr.write_ref("refs/tags/v1", &ReferenceTarget::Direct(oid), None, None)
        .unwrap();

    let branches = mgr.list_refs("refs/heads").unwrap();
    assert_eq!(branches.len(), 2);
    assert_eq!(branches[0].name, "refs/heads/a");
    assert_eq!(branches[1].name, "refs/heads/b");

    // Delete ref
    mgr.delete_ref("refs/heads/a", None).unwrap();
    assert!(mgr.read_ref("refs/heads/a").is_err());
    assert_eq!(mgr.list_refs("refs/heads").unwrap().len(), 1);
}
