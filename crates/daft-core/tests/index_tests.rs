use daft_core::cas::ObjectId;
use daft_core::error::IndexError;
use daft_core::index::{Index, IndexEntry, IndexLock, Stage, FIXED_ENTRY_SIZE};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_empty_index_roundtrip() {
    let dir = tempdir().unwrap();
    let index_path = dir.path().join("index");

    let empty_index = Index::new();
    empty_index.write_to(&index_path).unwrap();

    let read_back = Index::read_from(&index_path).unwrap();
    assert_eq!(read_back.len(), 0);
    assert!(read_back.is_empty());
}

#[test]
fn test_entry_padding_alignment() {
    let test_lengths = [1, 4, 6, 7, 8, 9, 10, 15, 16, 23];
    let oid = ObjectId::ZERO;

    for len in test_lengths {
        let path = "a".repeat(len);
        let entry = IndexEntry::new(&path, oid, 0o100644, Stage::Normal, 100).expect("entry");

        let unpadded = FIXED_ENTRY_SIZE + path.len();
        let pad_len = entry.padding_len();
        let total_len = entry.total_serialized_len();

        assert!(
            (1..=8).contains(&pad_len),
            "Padding for len {} must be in [1, 8], got {}",
            len,
            pad_len
        );
        assert_eq!(
            (unpadded + pad_len) % 8,
            0,
            "Total length must be multiple of 8"
        );
        assert_eq!(total_len % 8, 0);
    }
}

#[test]
fn test_entry_sorting_invariant() {
    let mut index = Index::new();
    let oid = ObjectId::ZERO;

    let paths = ["z.txt", "a.txt", "b/c.txt", "b.txt"];
    for p in paths {
        index.add_entry(IndexEntry::new(p, oid, 0o100644, Stage::Normal, 10).expect("entry"));
    }

    let sorted_paths: Vec<&str> = index.entries().iter().map(|e| e.path.as_str()).collect();
    assert_eq!(sorted_paths, vec!["a.txt", "b.txt", "b/c.txt", "z.txt"]);
}

#[test]
fn test_multistage_conflict_handling_and_resolution() {
    let mut index = Index::new();
    let oid_base = ObjectId::hash(b"base");
    let oid_ours = ObjectId::hash(b"ours");
    let oid_theirs = ObjectId::hash(b"theirs");

    // Insert 3 conflict stages for "conflict.rs"
    index.add_entry(
        IndexEntry::new("conflict.rs", oid_base, 0o100644, Stage::Ancestor, 50)
            .expect("ancestor entry"),
    );
    index.add_entry(
        IndexEntry::new("conflict.rs", oid_ours, 0o100644, Stage::Ours, 60).expect("ours entry"),
    );
    index.add_entry(
        IndexEntry::new("conflict.rs", oid_theirs, 0o100644, Stage::Theirs, 70)
            .expect("theirs entry"),
    );

    assert!(index.has_conflicts());
    assert_eq!(index.len(), 3);
    assert_eq!(index.conflicts().get("conflict.rs").unwrap().len(), 3);

    // Resolve conflict by staging Stage::Normal
    let oid_resolved = ObjectId::hash(b"resolved");
    index.add_entry(
        IndexEntry::new("conflict.rs", oid_resolved, 0o100644, Stage::Normal, 80)
            .expect("resolved entry"),
    );

    assert!(!index.has_conflicts());
    assert_eq!(index.len(), 1);
    let entry = index.find_entry("conflict.rs", Stage::Normal).unwrap();
    assert_eq!(entry.oid, oid_resolved);
}

#[test]
fn test_long_path_name_clamp() {
    let dir = tempdir().unwrap();
    let index_path = dir.path().join("index");

    let mut index = Index::new();
    let long_path = "nested/".repeat(700) + "file.txt"; // ~5000 chars
    assert!(long_path.len() > 4095);

    let oid = ObjectId::hash(b"long path");
    index.add_entry(
        IndexEntry::new(&long_path, oid, 0o100644, Stage::Normal, 123).expect("long path entry"),
    );

    index.write_to(&index_path).unwrap();

    let read_back = Index::read_from(&index_path).unwrap();
    assert_eq!(read_back.len(), 1);
    let recovered = &read_back.entries()[0];
    assert_eq!(recovered.path, long_path);
    assert_eq!(recovered.oid, oid);
}

#[test]
fn test_checksum_bitflip_detection() {
    let dir = tempdir().unwrap();
    let index_path = dir.path().join("index");

    let mut index = Index::new();
    index.add_entry(
        IndexEntry::new("file.txt", ObjectId::ZERO, 0o100644, Stage::Normal, 10).expect("entry"),
    );
    index.write_to(&index_path).unwrap();

    // Corrupt one byte in the index file (not the checksum itself)
    let mut bytes = fs::read(&index_path).unwrap();
    bytes[15] ^= 0xFF;
    fs::write(&index_path, &bytes).unwrap();

    let result = Index::read_from(&index_path);
    assert!(matches!(result, Err(IndexError::CorruptChecksum { .. })));
}

#[test]
fn test_index_lock_contention_and_rollback() {
    let dir = tempdir().unwrap();
    let index_path = dir.path().join("index");

    let lock1 = IndexLock::acquire(&index_path).expect("acquire lock 1");

    // Second lock attempt must fail
    let lock2_err = IndexLock::acquire(&index_path);
    assert!(matches!(lock2_err, Err(IndexError::IndexLocked(_))));

    let lock_file = dir.path().join("index.lock");
    assert!(lock_file.exists());

    // Drop lock1 without committing -> rollback
    drop(lock1);
    assert!(!lock_file.exists());
}

#[test]
fn test_index_entry_path_validation() {
    let oid = ObjectId::ZERO;
    assert!(IndexEntry::validate_path("valid/path.txt").is_ok());
    assert!(matches!(
        IndexEntry::validate_path(""),
        Err(IndexError::InvalidPath(_))
    ));
    assert!(matches!(
        IndexEntry::validate_path("null\0byte.txt"),
        Err(IndexError::InvalidPath(_))
    ));

    assert!(IndexEntry::new("valid.txt", oid, 0o100644, Stage::Normal, 10).is_ok());
    assert!(matches!(
        IndexEntry::new("", oid, 0o100644, Stage::Normal, 0),
        Err(IndexError::InvalidPath(_))
    ));
    assert!(matches!(
        IndexEntry::new("evil\0file.txt", oid, 0o100644, Stage::Normal, 10),
        Err(IndexError::InvalidPath(_))
    ));
}
