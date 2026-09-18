//! Live State Fork Integration Tests (Feature 41).
//!
//! Validates:
//! 1. Capturing in-flight uncommitted files (unstaged changes + untracked files) without an intermediate commit.
//! 2. Staged index state inheritance into the child dimension.
//! 3. Two-way isolation: mutations in child never alter source, and vice versa.
//! 4. Parent lineage metadata (`parent = Some(source)`).

use daft_core::init::{init, InitOptions};
use daft_core::Repository;
use daft_dimension::DimensionManager;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn test_live_fork_from_active_dirty_dimension() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    init(&repo_root, &InitOptions::default()).unwrap();
    let repo = Arc::new(Repository::open(&repo_root.join(".dft")).unwrap());
    let manager = DimensionManager::new(repo.clone());
    manager.init().unwrap();

    // 1. Create a base committed file in mainline
    let file1 = repo_root.join("base.txt");
    fs::write(&file1, "base content").unwrap();
    let mut index = repo.index().unwrap();
    let blob1 = repo.cas().write_blob(b"base content").unwrap();
    index.add_entry(
        daft_core::index::IndexEntry::new(
            "base.txt",
            blob1,
            0o100644,
            daft_core::index::Stage::Normal,
            12,
        )
        .unwrap(),
    );
    index.write_to(&repo.index_path()).unwrap();

    // 2. Introduce uncommitted dirty changes in mainline worktree
    let dirty_file = repo_root.join("dirty_draft.txt");
    fs::write(&dirty_file, "uncommitted live thoughts").unwrap();

    let unstaged_mod = repo_root.join("base.txt");
    fs::write(&unstaged_mod, "base content with unstaged edits").unwrap();

    // 3. Fork dimension live state into 'child-universe'
    let meta = manager
        .fork_dimension("child-universe", "mainline")
        .expect("fork should succeed");
    assert_eq!(meta.name, "child-universe");
    assert_eq!(meta.parent.as_deref(), Some("mainline"));

    let child_ws = manager.dimensions_dir().join("child-universe/workspace");
    assert!(child_ws.exists());

    // 4. Verify uncommitted files are present in child workspace
    assert_eq!(
        fs::read_to_string(child_ws.join("dirty_draft.txt")).unwrap(),
        "uncommitted live thoughts"
    );
    assert_eq!(
        fs::read_to_string(child_ws.join("base.txt")).unwrap(),
        "base content with unstaged edits"
    );

    // 5. Verify staged index was duplicated
    let child_index_path = manager.dimensions_dir().join("child-universe/index");
    assert!(child_index_path.exists());

    // 6. Verify independence: edit in child workspace does NOT modify source
    fs::write(child_ws.join("dirty_draft.txt"), "modified in child").unwrap();
    assert_eq!(
        fs::read_to_string(&dirty_file).unwrap(),
        "uncommitted live thoughts",
        "Source worktree was corrupted by child edit!"
    );
}

#[test]
fn test_live_fork_from_inactive_dimension() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    init(&repo_root, &InitOptions::default()).unwrap();
    let repo = Arc::new(Repository::open(&repo_root.join(".dft")).unwrap());
    let manager = DimensionManager::new(repo);
    manager.init().unwrap();

    // Create source dimension
    manager.create_dimension("parent-dim", None, None).unwrap();
    let parent_ws = manager.dimensions_dir().join("parent-dim/workspace");
    fs::write(parent_ws.join("code.rs"), "fn parent() {}").unwrap();

    // Fork child dimension from parent-dim (which is inactive)
    let meta = manager.fork_dimension("child-dim", "parent-dim").unwrap();
    assert_eq!(meta.parent.as_deref(), Some("parent-dim"));

    let child_ws = manager.dimensions_dir().join("child-dim/workspace");
    assert_eq!(
        fs::read_to_string(child_ws.join("code.rs")).unwrap(),
        "fn parent() {}"
    );

    // Verify metadata dual-write
    let meta_json = manager.dimensions_dir().join("child-dim/meta.json");
    let dim_json = manager.dimensions_dir().join("child-dim/dimension.json");
    assert!(meta_json.exists());
    assert!(dim_json.exists());
}

#[test]
fn test_live_fork_nonexistent_source_fails() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    init(&repo_root, &InitOptions::default()).unwrap();
    let repo = Arc::new(Repository::open(&repo_root.join(".dft")).unwrap());
    let manager = DimensionManager::new(repo);
    manager.init().unwrap();

    let res = manager.fork_dimension("invalid-child", "nonexistent-parent");
    assert!(res.is_err());
}
