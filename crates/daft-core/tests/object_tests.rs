use daft_core::cas::{ObjectId, ObjectType};
use daft_core::object::{
    cmp_tree_entries, Blob, Commit, FileMode, Object, ObjectError, Signature, Tag, Tree, TreeEntry,
};

#[test]
fn test_blob_empty() {
    let empty_blob = Blob::new(Vec::new());
    assert!(empty_blob.is_empty());
    assert_eq!(empty_blob.len(), 0);

    let obj = Object::Blob(empty_blob);
    let envelope = obj.serialize_envelope();
    assert_eq!(envelope, b"blob 0\0");

    let computed_oid = obj.compute_id();
    assert_eq!(
        computed_oid.to_hex(),
        "473a0f4c3be8a93681a267e3b1e9a7dcda1185436fe141f7749120a303721813"
    );
}

#[test]
fn test_blob_binary_roundtrip() {
    let binary_data = vec![0x00, 0xFF, 0x12, 0x34, 0x00, 0x7F, 0xAA];
    let blob = Blob::new(binary_data.clone());
    let serialized = blob.serialize();
    let deserialized = Blob::deserialize(serialized).expect("deserialize blob");
    assert_eq!(deserialized.data(), &binary_data[..]);
}

#[test]
fn test_file_mode_parsing() {
    assert_eq!(
        FileMode::from_octal_str("100644").unwrap(),
        FileMode::REGULAR
    );
    assert_eq!(
        FileMode::from_octal_str("100755").unwrap(),
        FileMode::EXECUTABLE
    );
    assert_eq!(
        FileMode::from_octal_str("120000").unwrap(),
        FileMode::SYMLINK
    );
    assert_eq!(FileMode::from_octal_str("040000").unwrap(), FileMode::TREE);
    assert_eq!(FileMode::from_octal_str("40000").unwrap(), FileMode::TREE);
    assert_eq!(
        FileMode::from_octal_str("160000").unwrap(),
        FileMode::GITLINK
    );

    assert!(FileMode::from_octal_str("100666").is_err());
    assert!(FileMode::from_octal_str("invalid").is_err());
}

#[test]
fn test_tree_canonical_sort() {
    // 1. foo.c (file) vs foo (dir)
    // '.' is 0x2E, '/' is 0x2F. 0x2E < 0x2F, so foo.c must sort BEFORE foo.
    let ord1 = cmp_tree_entries("foo.c", FileMode::REGULAR, "foo", FileMode::TREE);
    assert_eq!(ord1, std::cmp::Ordering::Less);

    // 2. foo-bar (file) vs foo (dir)
    // '-' is 0x2D, '/' is 0x2F. 0x2D < 0x2F, so foo-bar must sort BEFORE foo.
    let ord2 = cmp_tree_entries("foo-bar", FileMode::REGULAR, "foo", FileMode::TREE);
    assert_eq!(ord2, std::cmp::Ordering::Less);

    // 3. foo (dir) vs foo_bar (file)
    // '/' is 0x2F, '_' is 0x5F. 0x2F < 0x5F, so foo (dir) must sort BEFORE foo_bar.
    let ord3 = cmp_tree_entries("foo", FileMode::TREE, "foo_bar", FileMode::REGULAR);
    assert_eq!(ord3, std::cmp::Ordering::Less);
}

#[test]
fn test_tree_serialization_roundtrip() {
    let oid1 = ObjectId::hash(b"blob1");
    let oid2 = ObjectId::hash(b"sub_tree");
    let oid3 = ObjectId::hash(b"exec_blob");

    let entries = vec![
        TreeEntry::new(FileMode::REGULAR, "foo.c", oid1).unwrap(),
        TreeEntry::new(FileMode::TREE, "foo", oid2).unwrap(),
        TreeEntry::new(FileMode::EXECUTABLE, "run.sh", oid3).unwrap(),
    ];

    let tree = Tree::from_entries(entries).expect("from entries");
    // Canonical order in tree must be: foo.c, foo, run.sh
    assert_eq!(tree.entries()[0].name, "foo.c");
    assert_eq!(tree.entries()[1].name, "foo");
    assert_eq!(tree.entries()[2].name, "run.sh");

    let serialized = tree.serialize();
    let deserialized = Tree::deserialize(&serialized).expect("deserialize tree");
    assert_eq!(tree, deserialized);
}

#[test]
fn test_tree_duplicate_rejection() {
    let oid = ObjectId::ZERO;
    let entries = vec![
        TreeEntry::new(FileMode::REGULAR, "dup.txt", oid).unwrap(),
        TreeEntry::new(FileMode::REGULAR, "dup.txt", oid).unwrap(),
    ];
    assert!(matches!(
        Tree::from_entries(entries),
        Err(ObjectError::DuplicateTreeEntry(_))
    ));
}

#[test]
fn test_tree_duplicate_rejection_separated_by_canonical_key() {
    let oid1 = ObjectId::hash(b"1");
    let oid2 = ObjectId::hash(b"2");
    let oid3 = ObjectId::hash(b"3");

    // "foo" (blob) < "foo.bar" (blob) < "foo" (tree)
    let entries = vec![
        TreeEntry::new(FileMode::REGULAR, "foo", oid1).unwrap(),
        TreeEntry::new(FileMode::REGULAR, "foo.bar", oid2).unwrap(),
        TreeEntry::new(FileMode::TREE, "foo", oid3).unwrap(),
    ];
    let err = Tree::from_entries(entries).unwrap_err();
    assert_eq!(err, ObjectError::DuplicateTreeEntry("foo".to_string()));
}

#[test]
fn test_tree_duplicate_rejection_adjacent_different_modes() {
    let oid1 = ObjectId::hash(b"1");
    let oid2 = ObjectId::hash(b"2");

    // Adjacent "foo" (blob) and "foo" (tree) without intervening files
    let entries = vec![
        TreeEntry::new(FileMode::REGULAR, "foo", oid1).unwrap(),
        TreeEntry::new(FileMode::TREE, "foo", oid2).unwrap(),
    ];
    let err = Tree::from_entries(entries).unwrap_err();
    assert_eq!(err, ObjectError::DuplicateTreeEntry("foo".to_string()));
}

#[test]
fn test_tree_duplicate_rejection_multibyte_unicode() {
    let oid1 = ObjectId::hash(b"1");
    let oid2 = ObjectId::hash(b"2");
    let oid3 = ObjectId::hash(b"3");

    let entries = vec![
        TreeEntry::new(FileMode::REGULAR, "🦀", oid1).unwrap(),
        TreeEntry::new(FileMode::REGULAR, "🦀.rs", oid2).unwrap(),
        TreeEntry::new(FileMode::TREE, "🦀", oid3).unwrap(),
    ];
    let err = Tree::from_entries(entries).unwrap_err();
    assert_eq!(err, ObjectError::DuplicateTreeEntry("🦀".to_string()));
}

#[test]
fn test_tree_deserialize_rejects_separated_duplicate() {
    // Manually craft a valid canonical payload with duplicate "foo"
    // "100644 foo\0<32>100644 foo.bar\0<32>40000 foo\0<32>"
    let mut payload = Vec::new();
    payload.extend_from_slice(b"100644 foo\0");
    payload.extend_from_slice(&[0x11; 32]);
    payload.extend_from_slice(b"100644 foo.bar\0");
    payload.extend_from_slice(&[0x22; 32]);
    payload.extend_from_slice(b"40000 foo\0");
    payload.extend_from_slice(&[0x33; 32]);

    let res = Tree::deserialize(&payload);
    assert_eq!(
        res.unwrap_err(),
        ObjectError::DuplicateTreeEntry("foo".to_string())
    );
}

#[test]
fn test_tree_deserialize_rejects_adjacent_mixed_mode_duplicate() {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"100644 foo\0");
    payload.extend_from_slice(&[0x11; 32]);
    payload.extend_from_slice(b"40000 foo\0");
    payload.extend_from_slice(&[0x22; 32]);

    let res = Tree::deserialize(&payload);
    assert_eq!(
        res.unwrap_err(),
        ObjectError::DuplicateTreeEntry("foo".to_string())
    );
}

#[test]
fn test_tree_deserialize_out_of_order_detection() {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"100644 z.txt\0");
    payload.extend_from_slice(&[0x11; 32]);
    payload.extend_from_slice(b"100644 a.txt\0");
    payload.extend_from_slice(&[0x22; 32]);

    let res = Tree::deserialize(&payload);
    assert_eq!(
        res.unwrap_err(),
        ObjectError::TreeOutOfOrder("z.txt".to_string(), "a.txt".to_string())
    );
}

#[test]
fn test_tree_invalid_entry_names() {
    let oid = ObjectId::ZERO;
    assert!(TreeEntry::new(FileMode::REGULAR, "", oid).is_err());
    assert!(TreeEntry::new(FileMode::REGULAR, ".", oid).is_err());
    assert!(TreeEntry::new(FileMode::REGULAR, "..", oid).is_err());
    assert!(TreeEntry::new(FileMode::REGULAR, "a/b", oid).is_err());
    assert!(TreeEntry::new(FileMode::REGULAR, "a\0b", oid).is_err());
}

#[test]
fn test_signature_roundtrip() {
    let sig1 = Signature::new(
        "Linus Torvalds",
        "torvalds@linux-foundation.org",
        1112911993,
        -420,
    );
    let str1 = sig1.to_string();
    assert_eq!(
        str1,
        "Linus Torvalds <torvalds@linux-foundation.org> 1112911993 -0700"
    );
    let parsed1 = Signature::parse(&str1).expect("parse sig1");
    assert_eq!(sig1, parsed1);

    // India time zone (+0530 = +330 mins)
    let sig2 = Signature::new("Developer", "dev@example.in", 1700000000, 330);
    assert_eq!(Signature::format_tz(sig2.tz_offset), "+0530");
    let str2 = sig2.to_string();
    let parsed2 = Signature::parse(&str2).expect("parse sig2");
    assert_eq!(sig2, parsed2);

    // Negative timestamp (pre-1970)
    let sig3 = Signature::new("History", "past@epoch.org", -1000000, 0);
    let str3 = sig3.to_string();
    let parsed3 = Signature::parse(&str3).expect("parse sig3");
    assert_eq!(sig3, parsed3);

    // Empty email
    let sig4 = Signature::new("Anonymous", "", 1700000000, 0);
    let str4 = sig4.to_string();
    assert_eq!(str4, "Anonymous <> 1700000000 +0000");
    let parsed4 = Signature::parse(&str4).expect("parse sig4");
    assert_eq!(sig4, parsed4);
}

#[test]
fn test_commit_serialization_roundtrip() {
    let tree_oid = ObjectId::hash(b"root tree");
    let parent1 = ObjectId::hash(b"parent 1");
    let parent2 = ObjectId::hash(b"parent 2");
    let author = Signature::new("Alice", "alice@example.com", 1600000000, 0);
    let committer = Signature::new("Bob", "bob@example.com", 1600000100, 60);

    let message = "Merge branch 'feature' into main\n\nDetailed multiline\ndescription here.\n";

    let mut commit = Commit::new(tree_oid, vec![parent1, parent2], author, committer, message);
    commit.gpg_sig = Some("-----BEGIN PGP SIGNATURE-----\nVersion: 1.0\n\niQEcBAABCAAGBQ...\n=abcd\n-----END PGP SIGNATURE-----".to_string());

    assert!(commit.is_merge());
    assert!(!commit.is_root());

    let serialized = commit.serialize();
    let deserialized = Commit::deserialize(&serialized).expect("deserialize commit");

    assert_eq!(commit.tree, deserialized.tree);
    assert_eq!(commit.parents, deserialized.parents);
    assert_eq!(commit.author, deserialized.author);
    assert_eq!(commit.committer, deserialized.committer);
    assert_eq!(commit.gpg_sig, deserialized.gpg_sig);
    assert_eq!(commit.message, deserialized.message);
}

#[test]
fn test_tag_annotated_roundtrip() {
    let target = ObjectId::hash(b"target commit");
    let tagger = Signature::new("Release Lead", "lead@company.com", 1650000000, -300);
    let tag = Tag::new(
        target,
        ObjectType::Commit,
        "v1.0.0",
        Some(tagger),
        "Production release 1.0.0\n",
    );

    let serialized = tag.serialize();
    let deserialized = Tag::deserialize(&serialized).expect("deserialize tag");
    assert_eq!(tag, deserialized);
}

#[test]
fn test_object_envelope_roundtrip() {
    let blob = Blob::new(b"roundtrip payload".to_vec());
    let obj = Object::Blob(blob);

    let envelope = obj.serialize_envelope();
    let (parsed_type, parsed_oid, parsed_obj) =
        Object::parse_envelope(&envelope).expect("parse envelope");

    assert_eq!(parsed_type, ObjectType::Blob);
    assert_eq!(parsed_oid, obj.compute_id());
    assert_eq!(parsed_obj, obj);
}
