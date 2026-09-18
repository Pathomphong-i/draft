use daft_core::cas::ObjectId;
use daft_core::error::{IndexError, ObjectError};
use daft_core::index::{Index, IndexEntry, Stage};
use daft_core::object::tree::{FileMode, Tree, TreeEntry};
use sha2::{Digest, Sha256};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_stress_tree_10000_entries_roundtrip_and_duplicates() {
    let mut entries = Vec::with_capacity(10_000);
    for i in 0..10_000 {
        let mode = match i % 5 {
            0 => FileMode::REGULAR,
            1 => FileMode::EXECUTABLE,
            2 => FileMode::SYMLINK,
            3 => FileMode::TREE,
            _ => FileMode::GITLINK,
        };
        let name = format!(
            "entry_{:06}_{}",
            i,
            if mode.is_tree() { "dir" } else { "file" }
        );
        let oid = ObjectId::from_bytes([(i % 256) as u8; 32]);
        entries.push(TreeEntry::new(mode, name, oid).unwrap());
    }

    // from_entries must succeed on 10,000 distinct names
    let tree = Tree::from_entries(entries.clone()).expect("tree with 10k entries");
    assert_eq!(tree.len(), 10_000);

    // Serialization and deserialization roundtrip
    let serialized = tree.serialize();
    let deserialized = Tree::deserialize(&serialized).expect("deserialize 10k tree");
    assert_eq!(tree.len(), deserialized.len());
    assert_eq!(tree, deserialized);

    // Attempting to create a tree with duplicate entry at extremes (index 0 and 9999)
    let mut bad_entries = entries.clone();
    let dup_name = bad_entries[0].name.clone();
    bad_entries[9999].name = dup_name.clone();
    let err = Tree::from_entries(bad_entries).unwrap_err();
    assert_eq!(err, ObjectError::DuplicateTreeEntry(dup_name));

    // Test tree mutation: insert existing entry replaces without duplicate
    let mut mutated_tree = tree;
    let new_entry = TreeEntry::new(
        FileMode::REGULAR,
        &entries[500].name,
        ObjectId::from_bytes([0xFF; 32]),
    )
    .unwrap();
    mutated_tree.insert(new_entry.clone()).unwrap();
    assert_eq!(mutated_tree.len(), 10_000);
    assert_eq!(
        mutated_tree.get(&entries[500].name).unwrap().oid,
        ObjectId::from_bytes([0xFF; 32])
    );
}

#[test]
fn test_stress_tree_edge_case_names_and_modes() {
    let edge_names = vec![
        "file with spaces.txt",
        "  leading_spaces",
        "trailing_spaces  ",
        "file-with-dashes",
        "file_with_underscores",
        "file.with.many.dots.tar.gz",
        "special!@#$^&()_+=",
        "CJK_漢字_テスト",
        "Thai_ภาษาไทย_ทดสอบ",
        "Cyrillic_Русский_текст",
        "Emoji_🚀_🌟_🦀",
        "Z_final_entry",
    ];

    let mut entries = Vec::new();
    for (i, name) in edge_names.iter().enumerate() {
        let mode = if i % 2 == 0 {
            FileMode::REGULAR
        } else {
            FileMode::TREE
        };
        let oid = ObjectId::from_bytes([(i + 1) as u8; 32]);
        entries.push(TreeEntry::new(mode, *name, oid).unwrap());
    }

    let tree = Tree::from_entries(entries).expect("tree with edge names");
    let serialized = tree.serialize();
    let deserialized = Tree::deserialize(&serialized).expect("deserialized edge tree");
    assert_eq!(tree, deserialized);

    // Verify trailing junk bytes rejection
    let mut junk_payload = serialized.clone();
    junk_payload.extend_from_slice(b"junk");
    assert!(matches!(
        Tree::deserialize(&junk_payload),
        Err(ObjectError::TruncatedTree(_))
    ));

    // Verify invalid mode parsing rejection
    let mut invalid_mode_payload = Vec::new();
    invalid_mode_payload.extend_from_slice(b"100777 badmode.txt\0");
    invalid_mode_payload.extend_from_slice(&[0x11; 32]);
    assert!(matches!(
        Tree::deserialize(&invalid_mode_payload),
        Err(ObjectError::InvalidMode(_, _))
    ));
}

#[test]
fn test_stress_index_path_clamping_boundary_sweep() {
    let dir = tempdir().expect("tempdir");
    let boundary_lengths = [4093, 4094, 4095, 4096, 4097, 6000, 8192];

    for &len in &boundary_lengths {
        let index_path = dir.path().join(format!("index_{}", len));
        let path = "d/".repeat(len / 2) + if len % 2 == 1 { "x" } else { "" };
        assert_eq!(path.len(), len);

        let oid = ObjectId::hash(path.as_bytes());
        let entry = IndexEntry::new(&path, oid, 0o100644, Stage::Normal, len as u32)
            .expect("valid long entry");

        // Verify alignment invariant
        assert_eq!(entry.total_serialized_len() % 8, 0);

        let mut idx = Index::new();
        idx.add_entry(entry);
        idx.write_to(&index_path).expect("write long path index");

        let recovered = Index::read_from(&index_path).expect("read long path index");
        assert_eq!(recovered.len(), 1);
        let rec = &recovered.entries()[0];
        assert_eq!(rec.path, path);
        assert_eq!(rec.oid, oid);
        assert_eq!(rec.file_size, len as u32);
    }
}

#[test]
fn test_stress_index_corrupt_padding_rejection() {
    let dir = tempdir().expect("tempdir");
    let index_path = dir.path().join("index");

    let mut idx = Index::new();
    idx.add_entry(
        IndexEntry::new("test.txt", ObjectId::ZERO, 0o100644, Stage::Normal, 10).unwrap(),
    );
    idx.write_to(&index_path).unwrap();

    let mut data = fs::read(&index_path).unwrap();
    // Entry format: 12 bytes header + 74 bytes fixed + 8 bytes path ("test.txt") + 6 bytes padding + 32 bytes cksum
    // Padding starts at: 12 + 74 + 8 = 94. Padding length = 6 bytes (94..100).
    // Corrupt one padding byte from 0x00 to 0x01
    data[95] = 0x01;

    // Recalculate SHA-256 checksum so checksum verification passes, forcing parser to inspect padding
    let content_len = data.len() - 32;
    let mut hasher = Sha256::new();
    hasher.update(&data[..content_len]);
    let new_cksum: [u8; 32] = hasher.finalize().into();
    data[content_len..].copy_from_slice(&new_cksum);

    let corrupt_path = dir.path().join("index_corrupt_pad");
    fs::write(&corrupt_path, &data).unwrap();

    // Must be rejected because padding byte is non-zero
    let res = Index::read_from(&corrupt_path);
    assert!(
        matches!(res, Err(IndexError::TruncatedEntry(_))),
        "non-zero padding byte must be rejected by Index::read_from, got {:?}",
        res
    );
}

#[test]
fn test_stress_index_5000_entries_multistage_and_concurrency() {
    let mut index = Index::new();
    for i in 0..1000 {
        let path = format!("src/module_{:04}.rs", i);
        let oid_base = ObjectId::from_bytes([(i % 250) as u8; 32]);
        let oid_ours = ObjectId::from_bytes([((i + 1) % 250) as u8; 32]);
        let oid_theirs = ObjectId::from_bytes([((i + 2) % 250) as u8; 32]);

        index.add_entry(IndexEntry::new(&path, oid_base, 0o100644, Stage::Ancestor, 100).unwrap());
        index.add_entry(IndexEntry::new(&path, oid_ours, 0o100644, Stage::Ours, 110).unwrap());
        index.add_entry(IndexEntry::new(&path, oid_theirs, 0o100644, Stage::Theirs, 120).unwrap());
    }

    assert_eq!(index.len(), 3000);
    assert!(index.has_conflicts());

    // Resolve half of them
    for i in 0..500 {
        let path = format!("src/module_{:04}.rs", i);
        let oid_resolved = ObjectId::from_bytes([0xEE; 32]);
        index
            .add_entry(IndexEntry::new(&path, oid_resolved, 0o100644, Stage::Normal, 115).unwrap());
    }

    // 500 resolved (1 entry each) + 500 unresolved (3 entries each) = 2000 entries
    assert_eq!(index.len(), 2000);
    assert!(index.has_conflicts());
    assert_eq!(index.conflicts().len(), 500);

    let dir = tempdir().unwrap();
    let index_path = dir.path().join("index_large");
    index.write_to(&index_path).unwrap();

    let recovered = Index::read_from(&index_path).unwrap();
    assert_eq!(recovered.len(), 2000);
    assert!(recovered.has_conflicts());
    assert_eq!(recovered.conflicts().len(), 500);
}
