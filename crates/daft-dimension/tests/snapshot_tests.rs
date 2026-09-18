//! Dimension Snapshots Integration Tests (Feature 42).
//!
//! Validates:
//! 1. Snapshot creation captures worktree files (staged, unstaged, untracked) into immutable CAS trees.
//! 2. Snapshot listing and filtering by dimension.
//! 3. Byte-for-byte restoration of corrupted or deleted working tree state.
//! 4. Safety guard preventing unforced restoration over dirty working tree.
//! 5. Snapshot deletion lifecycle.

use daft_core::init::{init, InitOptions};
use daft_core::Repository;
use daft_dimension::snapshot::SnapshotManager;
use daft_dimension::DimensionManager;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn test_snapshot_create_list_restore_lifecycle() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    init(&repo_root, &InitOptions::default()).unwrap();
    let repo = Arc::new(Repository::open(&repo_root.join(".dft")).unwrap());
    let manager = DimensionManager::new(repo.clone());
    manager.init().unwrap();

    // 1. Create files in mainline
    let f1 = repo_root.join("code.rs");
    fs::write(&f1, "pub fn v1() {}").unwrap();
    let f2 = repo_root.join("notes.txt");
    fs::write(&f2, "initial notes").unwrap();

    // 2. Create snapshot 'snap-1'
    let snap1 = SnapshotManager::create_snapshot(
        &repo,
        "mainline",
        "checkpoint-1",
        Some("Initial working state"),
    )
    .expect("create snapshot");

    assert_eq!(snap1.name, "checkpoint-1");
    assert_eq!(snap1.dimension_id, "mainline");
    assert_eq!(snap1.file_count, 2);

    // Verify snapshot file on disk
    let snap_dir = repo.dft_dir().join("snapshots/mainline");
    assert!(snap_dir
        .join(format!("{}.json", snap1.snapshot_id))
        .exists());

    // 3. Create second snapshot 'snap-2' with additional files
    let f3 = repo_root.join("extra.md");
    fs::write(&f3, "# Extra documentation").unwrap();
    let snap2 = SnapshotManager::create_snapshot(
        &repo,
        "mainline",
        "checkpoint-2",
        Some("Added documentation"),
    )
    .expect("create snapshot 2");
    assert_eq!(snap2.file_count, 3);

    // 4. List snapshots
    let list = SnapshotManager::list_snapshots(&repo, Some("mainline")).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].name, "checkpoint-2"); // Descending chronological order
    assert_eq!(list[1].name, "checkpoint-1");

    // 5. Corrupt worktree
    fs::write(&f1, "CORRUPTED CONTENT").unwrap();
    let _ = fs::remove_file(&f2);
    let _ = fs::remove_file(&f3);

    // 6. Restore 'checkpoint-1' with force
    let restored = SnapshotManager::restore_snapshot(&repo, "mainline", "checkpoint-1", true)
        .expect("restore snapshot");

    assert_eq!(restored.name, "checkpoint-1");
    assert_eq!(fs::read_to_string(&f1).unwrap(), "pub fn v1() {}");
    assert_eq!(fs::read_to_string(&f2).unwrap(), "initial notes");
    assert!(!f3.exists(), "extra.md should not exist in checkpoint-1");

    // 7. Delete snapshot 'checkpoint-2'
    let deleted = SnapshotManager::delete_snapshot(&repo, "mainline", "checkpoint-2").unwrap();
    assert_eq!(deleted.name, "checkpoint-2");

    let list_after = SnapshotManager::list_snapshots(&repo, Some("mainline")).unwrap();
    assert_eq!(list_after.len(), 1);
    assert_eq!(list_after[0].name, "checkpoint-1");
}

#[test]
fn test_snapshot_restore_dirty_guard() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    init(&repo_root, &InitOptions::default()).unwrap();
    let repo = Arc::new(Repository::open(&repo_root.join(".dft")).unwrap());
    let manager = DimensionManager::new(repo.clone());
    manager.init().unwrap();

    let f1 = repo_root.join("file.txt");
    fs::write(&f1, "v1").unwrap();

    // Stage file into index
    let blob = repo.cas().write_blob(b"v1").unwrap();
    let mut index = repo.index().unwrap();
    index.add_entry(
        daft_core::index::IndexEntry::new(
            "file.txt",
            blob,
            0o100644,
            daft_core::index::Stage::Normal,
            2,
        )
        .unwrap(),
    );
    index.write_to(&repo.index_path()).unwrap();

    SnapshotManager::create_snapshot(&repo, "mainline", "snap-base", None).unwrap();

    // Make working tree dirty
    fs::write(&f1, "v2-dirty").unwrap();

    // Attempt restore without force -> must fail
    let res = SnapshotManager::restore_snapshot(&repo, "mainline", "snap-base", false);
    assert!(
        res.is_err(),
        "Expected dirty workspace restore to be rejected without --force"
    );

    // Attempt restore with force -> must succeed
    let res_forced = SnapshotManager::restore_snapshot(&repo, "mainline", "snap-base", true);
    assert!(res_forced.is_ok());
    assert_eq!(fs::read_to_string(&f1).unwrap(), "v1");
}
