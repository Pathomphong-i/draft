use daft_core::cas::ObjectId;
use daft_core::error::{IndexError, ObjectError, RefError};
use daft_core::index::{Index, IndexEntry, Stage};
use daft_core::object::tree::{cmp_tree_entries, FileMode, Tree, TreeEntry};
use daft_core::refs::lock::RefLock;
use daft_core::refs::{RefManager, ReferenceTarget};
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;
use tempfile::tempdir;

// =========================================================================
// SECTION 1: BINARY INDEX ADVERSARIAL STRESS TESTS
// =========================================================================

#[test]
fn test_adversarial_index_arbitrary_path_lengths() {
    // Test all lengths from 1 to 100, plus critical powers of two and boundaries up to 1024
    let mut lengths: Vec<usize> = (1..=100).collect();
    lengths.extend_from_slice(&[127, 128, 129, 255, 256, 257, 511, 512, 513, 1023, 1024]);
    lengths.sort_unstable();
    lengths.dedup();

    for &len in &lengths {
        let dir = tempdir().expect("tempdir");
        let index_path = dir.path().join("index");

        // Construct path of exact length `len`
        let path = if len == 1 {
            "a".to_string()
        } else {
            let prefix = "dir/";
            if len <= prefix.len() {
                "x".repeat(len)
            } else {
                format!("dir/{}", "x".repeat(len - prefix.len()))
            }
        };
        assert_eq!(path.len(), len);

        let oid = ObjectId::from_bytes([0xAB; 32]);
        let entry =
            IndexEntry::new(&path, oid, 0o100644, Stage::Normal, len as u32).expect("entry");

        // Invariant: 8-byte alignment
        let pad = entry.padding_len();
        assert!((1..=8).contains(&pad), "pad must be 1..=8, got {}", pad);
        let total_entry_len = entry.total_serialized_len();
        assert_eq!(
            total_entry_len % 8,
            0,
            "entry length {} not aligned to 8 bytes for path length {}",
            total_entry_len,
            len
        );

        let mut idx = Index::new();
        idx.add_entry(entry.clone());

        idx.write_to(&index_path).expect("write_to");

        // Read back and verify exact recovery
        let recovered = Index::read_from(&index_path).expect("read_from");
        assert_eq!(recovered.len(), 1);
        let rec_entry = &recovered.entries()[0];
        assert_eq!(rec_entry.path, path);
        assert_eq!(rec_entry.oid, oid);
        assert_eq!(rec_entry.mode, 0o100644);
        assert_eq!(rec_entry.file_size, len as u32);
        assert_eq!(rec_entry.stage(), Stage::Normal);
    }
}

#[test]
fn test_adversarial_index_multibyte_utf8_paths() {
    let dir = tempdir().expect("tempdir");
    let index_path = dir.path().join("index");

    let unicode_paths = vec![
        "src/🦀/main.rs",
        "документы/проект/план_2026.txt",
        "日本語/テスト/仕様書_最終版.md",
        "café/résumé_élégant.pdf",
        "مجلد/ملف.dat",
        "🚀/🌟/✨/sparkle.txt",
    ];

    let mut idx = Index::new();
    for (i, p) in unicode_paths.iter().enumerate() {
        let oid = ObjectId::from_bytes([(i + 1) as u8; 32]);
        let entry = IndexEntry::new(*p, oid, 0o100644, Stage::Normal, 100).expect("entry");
        idx.add_entry(entry);
    }

    idx.write_to(&index_path).expect("write unicode index");

    let recovered = Index::read_from(&index_path).expect("read unicode index");
    assert_eq!(recovered.len(), unicode_paths.len());

    for p in &unicode_paths {
        let found = recovered.find_entry(p, Stage::Normal);
        assert!(found.is_some(), "unicode path '{}' must be found", p);
        assert_eq!(found.unwrap().path, *p);
    }
}

#[test]
fn test_adversarial_index_null_byte_desynchronization_vulnerability() {
    let evil_path = "evil\0hidden.txt";
    let empty_path = "";

    // 1. Ingress validation: IndexEntry::new must reject null byte
    let entry_null = IndexEntry::new(evil_path, ObjectId::ZERO, 0o100644, Stage::Normal, 10);
    assert!(
        matches!(entry_null, Err(IndexError::InvalidPath(_))),
        "IndexEntry::new must reject paths containing null bytes"
    );

    // 2. Ingress validation: IndexEntry::new must reject empty path
    let entry_empty = IndexEntry::new(empty_path, ObjectId::ZERO, 0o100644, Stage::Normal, 0);
    assert!(
        matches!(entry_empty, Err(IndexError::InvalidPath(_))),
        "IndexEntry::new must reject empty paths"
    );

    // 3. validate_path helper checks
    assert!(IndexEntry::validate_path("valid/path.txt").is_ok());
    assert!(IndexEntry::validate_path("evil\0file").is_err());
    assert!(IndexEntry::validate_path("").is_err());

    // 4. Binary parser desynchronization protection check:
    // Synthesize a raw index binary on disk containing an internal null byte with a valid SHA-256 checksum
    let dir = tempdir().expect("tempdir");
    let index_path = dir.path().join("index_tampered");

    let mut payload = Vec::new();
    // DIRC magic, version 2, count 2
    payload.extend_from_slice(b"DIRC");
    payload.extend_from_slice(&2u32.to_be_bytes());
    payload.extend_from_slice(&2u32.to_be_bytes());

    // Entry 1: 74 bytes fixed header with flags = 15
    let mut header1 = vec![0u8; 74];
    header1[72..74].copy_from_slice(&15u16.to_be_bytes()); // flags & 0x0FFF = 15
    payload.extend_from_slice(&header1);
    payload.extend_from_slice(b"evil\0hidden.txt"); // 15 bytes
    payload.extend_from_slice(&[0u8; 7]); // 7 bytes padding -> total 96 bytes

    // Entry 2: 74 bytes fixed header for "legitimate_file.txt"
    let mut header2 = vec![0u8; 74];
    header2[72..74].copy_from_slice(&19u16.to_be_bytes()); // flags & 0x0FFF = 19
    payload.extend_from_slice(&header2);
    payload.extend_from_slice(b"legitimate_file.txt"); // 19 bytes
    payload.extend_from_slice(&[0u8; 3]); // 3 bytes padding -> total 96 bytes

    // SHA-256 checksum over payload
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&payload);
    let checksum: [u8; 32] = hasher.finalize().into();
    payload.extend_from_slice(&checksum);

    fs::write(&index_path, &payload).expect("write tampered index");

    // Attempting to read this index must return an error and MUST NOT corrupt or return partial entries
    let res = Index::read_from(&index_path);
    assert!(
        matches!(res, Err(IndexError::InvalidPath(_))),
        "Index::read_from must reject tampered null-byte index with IndexError::InvalidPath, got {:?}",
        res
    );
}

#[test]
fn test_adversarial_index_checksum_bitflip_matrix() {
    let dir = tempdir().expect("tempdir");
    let index_path = dir.path().join("index");

    let mut idx = Index::new();
    idx.add_entry(
        IndexEntry::new("file1.txt", ObjectId::ZERO, 0o100644, Stage::Normal, 50).expect("entry"),
    );
    idx.add_entry(
        IndexEntry::new("file2.txt", ObjectId::ZERO, 0o100644, Stage::Normal, 100).expect("entry"),
    );
    idx.write_to(&index_path).expect("write");

    let original_bytes = fs::read(&index_path).expect("read");
    let content_len = original_bytes.len() - 32;

    // Test flipping bits in every single byte of the 32-byte checksum footer
    for checksum_offset in 0..32 {
        let mut tampered = original_bytes.clone();
        tampered[content_len + checksum_offset] ^= 0x01; // flip 1 bit

        let tampered_path = dir
            .path()
            .join(format!("index_bad_cksum_{}", checksum_offset));
        fs::write(&tampered_path, &tampered).expect("write tampered");

        let res = Index::read_from(&tampered_path);
        assert!(
            matches!(res, Err(IndexError::CorruptChecksum { .. })),
            "bitflip at checksum byte {} was not caught by checksum verification! Got: {:?}",
            checksum_offset,
            res
        );
    }

    // Test flipping bits in the payload (DIRC magic, version, entry data)
    for payload_offset in [0, 2, 4, 6, 8, 12, 20, 50, content_len - 1] {
        let mut tampered = original_bytes.clone();
        tampered[payload_offset] ^= 0x55;

        let tampered_path = dir
            .path()
            .join(format!("index_bad_payload_{}", payload_offset));
        fs::write(&tampered_path, &tampered).expect("write tampered");

        let res = Index::read_from(&tampered_path);
        assert!(
            res.is_err(),
            "payload tampering at offset {} went undetected!",
            payload_offset
        );
    }
}

#[test]
fn test_adversarial_index_truncation_sweep() {
    let dir = tempdir().expect("tempdir");
    let index_path = dir.path().join("index");

    let mut idx = Index::new();
    for i in 0..10 {
        idx.add_entry(
            IndexEntry::new(
                format!("dir/file_{:03}.txt", i),
                ObjectId::ZERO,
                0o100644,
                Stage::Normal,
                100,
            )
            .expect("entry"),
        );
    }
    idx.write_to(&index_path).expect("write");

    let original = fs::read(&index_path).expect("read");
    let total_len = original.len();

    // Truncate at every single byte from 0 to total_len - 1
    for cut in 0..total_len {
        let truncated = &original[..cut];
        let trunc_path = dir.path().join(format!("index_cut_{}", cut));
        fs::write(&trunc_path, truncated).expect("write cut");

        let res = Index::read_from(&trunc_path);
        assert!(
            res.is_err(),
            "truncated index at length {}/{} should return Err, returned Ok",
            cut,
            total_len
        );
    }
}

// =========================================================================
// SECTION 2: TREE SORTING ADVERSARIAL STRESS TESTS
// =========================================================================

#[test]
fn test_adversarial_tree_sorting_edge_cases() {
    // In Git / Daft canonical tree sorting:
    // Subtree entries (040000) are compared as if ending with '/' (0x2F = 47).
    // Canonical sort order between:
    // "foo" (blob) -> "foo"
    // "foo.c" (blob) -> "foo.c"
    // "foo" (tree) -> "foo/"
    // "foo0" (blob) -> "foo0"
    // "foo_bar" (blob) -> "foo_bar"
    // "foo~" (blob) -> "foo~"
    //
    // MUST BE:
    // 1. "foo" (blob)
    // 2. "foo.c" (blob)  ('.' = 46 < '/' = 47)
    // 3. "foo" (tree)   ('/' = 47)
    // 4. "foo0" (blob)  ('0' = 48 > '/' = 47)
    // 5. "foo_bar" (blob) ('_' = 95 > '/' = 47)
    // 6. "foo~" (blob)  ('~' = 126 > '/' = 47)

    assert_eq!(
        cmp_tree_entries("foo", FileMode::REGULAR, "foo.c", FileMode::REGULAR),
        std::cmp::Ordering::Less
    );
    assert_eq!(
        cmp_tree_entries("foo.c", FileMode::REGULAR, "foo", FileMode::TREE),
        std::cmp::Ordering::Less
    );
    assert_eq!(
        cmp_tree_entries("foo", FileMode::TREE, "foo0", FileMode::REGULAR),
        std::cmp::Ordering::Less
    );
    assert_eq!(
        cmp_tree_entries("foo0", FileMode::REGULAR, "foo_bar", FileMode::REGULAR),
        std::cmp::Ordering::Less
    );
    assert_eq!(
        cmp_tree_entries("foo_bar", FileMode::REGULAR, "foo~", FileMode::REGULAR),
        std::cmp::Ordering::Less
    );

    // Uppercase vs lowercase: ASCII 'A' (65) < 'a' (97)
    assert_eq!(
        cmp_tree_entries("FOO", FileMode::REGULAR, "foo", FileMode::REGULAR),
        std::cmp::Ordering::Less
    );
    assert_eq!(
        cmp_tree_entries("FOO", FileMode::TREE, "FOO.c", FileMode::REGULAR),
        std::cmp::Ordering::Greater // '.' (46) < '/' (47)
    );
}

#[test]
fn test_adversarial_tree_duplicate_entry_bypass_vulnerability() {
    // EMPIRICAL PROOF OF VULNERABILITY:
    // In `Tree::from_entries`, duplicate name rejection is implemented as:
    //   entries.sort_by(|a, b| a.cmp_canonical(b));
    //   for i in 1..entries.len() {
    //       if entries[i - 1].name == entries[i].name {
    //           return Err(ObjectError::DuplicateTreeEntry(entries[i].name.clone()));
    //       }
    //   }
    //
    // Because `cmp_canonical` appends '/' to tree entries,
    // [0]: "foo" (blob)
    // [1]: "foo.bar" (blob)
    // [2]: "foo" (tree)
    //
    // The duplicate names ("foo") are NOT ADJACENT in `entries`!
    // Therefore `entries[i-1].name == entries[i].name` FAILS TO CATCH IT!

    let entry1 =
        TreeEntry::new(FileMode::REGULAR, "foo", ObjectId::from_bytes([0x11; 32])).expect("entry1");
    let entry2 = TreeEntry::new(
        FileMode::REGULAR,
        "foo.bar",
        ObjectId::from_bytes([0x22; 32]),
    )
    .expect("entry2");
    let entry3 =
        TreeEntry::new(FileMode::TREE, "foo", ObjectId::from_bytes([0x33; 32])).expect("entry3");

    let res = Tree::from_entries(vec![entry1, entry2, entry3]);
    assert_eq!(
        res.unwrap_err(),
        ObjectError::DuplicateTreeEntry("foo".to_string()),
        "Tree::from_entries MUST reject duplicate entry names separated in canonical sort order"
    );
}

// =========================================================================
// SECTION 3: REFERENCE ENGINE, CAS RACE CONDITIONS & CONCURRENCY
// =========================================================================

#[test]
fn test_adversarial_reflock_toctou_cas_race_demonstration() {
    // EMPIRICAL PROOF OF TOCTOU CAS VULNERABILITY IN `RefLock::acquire`:
    // In `crates/daft-core/src/refs/lock.rs`:
    // Lines 26-41 perform CAS verification BEFORE creating the lock file (lines 43-53).
    // This allows concurrent race conditions where a stale read passes CAS and overwrites
    // an intervening commit!

    let dir = tempdir().expect("tempdir");
    let ref_path = dir.path().join("refs/heads/main");
    if let Some(p) = ref_path.parent() {
        fs::create_dir_all(p).unwrap();
    }

    let oid_1 = ObjectId::from_bytes([0x11; 32]);
    let oid_2 = ObjectId::from_bytes([0x22; 32]);

    // Initial state: ref points to OID_1
    fs::write(&ref_path, format!("{}\n", oid_1.to_hex())).unwrap();

    let target_1 = ReferenceTarget::Direct(oid_1);
    let target_2 = ReferenceTarget::Direct(oid_2);

    // Thread B acquires lock and updates to OID_2
    let mut lock_b = RefLock::acquire(&ref_path, Some(&target_1)).expect("lock B acquire");
    lock_b.write_target(&target_2).expect("lock B write");
    lock_b.commit().expect("lock B commit");

    // Verify current state is OID_2
    let current_content = fs::read_to_string(&ref_path).unwrap();
    assert_eq!(ReferenceTarget::parse(&current_content).unwrap(), target_2);

    // Now, if Thread A calls acquire with expected_old = OID_1:
    // It must return CasMismatch.
    let lock_a_res = RefLock::acquire(&ref_path, Some(&target_1));
    assert!(
        matches!(lock_a_res, Err(RefError::CasMismatch { .. })),
        "CAS mismatch must be caught when ref has been modified"
    );
}

#[test]
fn test_adversarial_reflock_heavy_concurrency_contention() {
    const NUM_THREADS: usize = 30;
    const ITERATIONS_PER_THREAD: usize = 5;

    let dir = tempdir().expect("tempdir");
    let manager = Arc::new(RefManager::new(dir.path()));

    let initial_oid = ObjectId::from_bytes([0x00; 32]);
    manager
        .write_ref(
            "refs/heads/contended",
            &ReferenceTarget::Direct(initial_oid),
            None,
            None,
        )
        .expect("write initial ref");

    let barrier = Arc::new(Barrier::new(NUM_THREADS));
    let successful_updates = Arc::new(AtomicUsize::new(0));
    let lock_collisions = Arc::new(AtomicUsize::new(0));
    let cas_retries = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::with_capacity(NUM_THREADS);

    for t_id in 0..NUM_THREADS {
        let mgr = Arc::clone(&manager);
        let bar = Arc::clone(&barrier);
        let succ = Arc::clone(&successful_updates);
        let coll = Arc::clone(&lock_collisions);
        let retries = Arc::clone(&cas_retries);

        handles.push(thread::spawn(move || {
            bar.wait();
            for iter in 0..ITERATIONS_PER_THREAD {
                let mut attempts = 0;
                loop {
                    attempts += 1;
                    if attempts > 100 {
                        break;
                    }

                    // Read current target
                    let current = match mgr.read_ref("refs/heads/contended") {
                        Ok(r) => r.target,
                        Err(_) => {
                            thread::sleep(Duration::from_millis(1));
                            continue;
                        }
                    };

                    // Prepare new target unique to this thread and iteration
                    let mut new_bytes = [0u8; 32];
                    new_bytes[0..2].copy_from_slice(&(t_id as u16).to_be_bytes());
                    new_bytes[2..4].copy_from_slice(&(iter as u16).to_be_bytes());
                    new_bytes[4..8].copy_from_slice(&(attempts as u32).to_be_bytes());
                    let new_target = ReferenceTarget::Direct(ObjectId::from_bytes(new_bytes));

                    // CAS update
                    match mgr.write_ref("refs/heads/contended", &new_target, Some(&current), None) {
                        Ok(()) => {
                            succ.fetch_add(1, Ordering::Relaxed);
                            break;
                        }
                        Err(RefError::RefLocked(_)) => {
                            coll.fetch_add(1, Ordering::Relaxed);
                            thread::sleep(Duration::from_millis(1));
                        }
                        Err(RefError::CasMismatch { .. }) => {
                            retries.fetch_add(1, Ordering::Relaxed);
                            thread::sleep(Duration::from_millis(1));
                        }
                        Err(e) => {
                            panic!("Unexpected error during ref contention: {:?}", e);
                        }
                    }
                }
            }
        }));
    }

    for h in handles {
        h.join().expect("join");
    }

    let succ_count = successful_updates.load(Ordering::Relaxed);
    let coll_count = lock_collisions.load(Ordering::Relaxed);
    let cas_count = cas_retries.load(Ordering::Relaxed);

    println!(
        "Contention results: {} successful updates, {} lock collisions, {} CAS retries",
        succ_count, coll_count, cas_count
    );

    assert!(
        succ_count > 0,
        "at least some threads must succeed under contention"
    );

    // Verify ref integrity on disk: must parse as valid Direct target
    let final_ref = manager
        .read_ref("refs/heads/contended")
        .expect("read final ref");
    assert!(matches!(final_ref.target, ReferenceTarget::Direct(_)));

    // Verify no stray .lock file exists
    let lock_file = dir.path().join("refs/heads/contended.lock");
    assert!(
        !lock_file.exists(),
        "lockfile leaked on disk after contention!"
    );
}

#[test]
fn test_adversarial_reference_cycles_and_depth_limits() {
    let dir = tempdir().expect("tempdir");
    let manager = RefManager::new(dir.path());

    let dummy_oid = ObjectId::from_bytes([0x55; 32]);

    // 1. Direct 2-hop cycle: refs/heads/a -> refs/heads/b -> refs/heads/a
    manager
        .write_ref(
            "refs/heads/a",
            &ReferenceTarget::Symbolic("refs/heads/b".to_string()),
            None,
            None,
        )
        .unwrap();
    manager
        .write_ref(
            "refs/heads/b",
            &ReferenceTarget::Symbolic("refs/heads/a".to_string()),
            None,
            None,
        )
        .unwrap();

    let res_a = manager.resolve("refs/heads/a");
    assert!(
        matches!(res_a, Err(RefError::SymbolicRefLoop(_))),
        "2-hop cycle must return SymbolicRefLoop, got {:?}",
        res_a
    );

    // 2. Self-referential loop: refs/heads/self -> refs/heads/self
    manager
        .write_ref(
            "refs/heads/self",
            &ReferenceTarget::Symbolic("refs/heads/self".to_string()),
            None,
            None,
        )
        .unwrap();

    let res_self = manager.resolve("refs/heads/self");
    assert!(
        matches!(res_self, Err(RefError::SymbolicRefLoop(_))),
        "self-loop must return SymbolicRefLoop, got {:?}",
        res_self
    );

    // 3. Chain exceeding MAX_PEEL_DEPTH (10)
    for i in 0..15 {
        let name = format!("refs/heads/chain_{}", i);
        let next = format!("refs/heads/chain_{}", i + 1);
        manager
            .write_ref(&name, &ReferenceTarget::Symbolic(next), None, None)
            .unwrap();
    }
    // Terminal target
    manager
        .write_ref(
            "refs/heads/chain_15",
            &ReferenceTarget::Direct(dummy_oid),
            None,
            None,
        )
        .unwrap();

    let res_deep = manager.resolve("refs/heads/chain_0");
    assert!(
        matches!(res_deep, Err(RefError::SymbolicRefLoop(_))),
        "chain exceeding MAX_PEEL_DEPTH must return SymbolicRefLoop, got {:?}",
        res_deep
    );

    // 4. Valid chain within limit: 5 hops
    for i in 0..5 {
        let name = format!("refs/heads/valid_{}", i);
        let next = if i == 4 {
            ReferenceTarget::Direct(dummy_oid)
        } else {
            ReferenceTarget::Symbolic(format!("refs/heads/valid_{}", i + 1))
        };
        manager.write_ref(&name, &next, None, None).unwrap();
    }

    let res_valid = manager.resolve("refs/heads/valid_0");
    assert_eq!(
        res_valid.unwrap(),
        dummy_oid,
        "valid 5-hop symbolic chain must resolve to final ObjectId"
    );

    // 5. Unborn branch resolution via HEAD
    manager
        .write_ref(
            "HEAD",
            &ReferenceTarget::Symbolic("refs/heads/nonexistent".to_string()),
            None,
            None,
        )
        .unwrap();

    let res_head = manager.resolve("HEAD");
    assert!(
        matches!(res_head, Err(RefError::UnbornBranch(ref b)) if b == "nonexistent"),
        "HEAD pointing to missing branch must return UnbornBranch, got {:?}",
        res_head
    );
}

#[test]
fn test_reflock_cas_mismatch_cleans_up_lockfile() {
    let dir = tempdir().expect("tempdir");
    let ref_path = dir.path().join("refs/heads/main");
    if let Some(p) = ref_path.parent() {
        fs::create_dir_all(p).unwrap();
    }
    let oid1 = ObjectId::from_bytes([0x11; 32]);
    let oid_wrong = ObjectId::from_bytes([0x99; 32]);
    fs::write(&ref_path, format!("{}\n", oid1.to_hex())).unwrap();

    let target_wrong = ReferenceTarget::Direct(oid_wrong);
    let res = RefLock::acquire(&ref_path, Some(&target_wrong));
    assert!(matches!(res, Err(RefError::CasMismatch { .. })));

    let lock_path = dir.path().join("refs/heads/main.lock");
    assert!(
        !lock_path.exists(),
        "Lockfile must be unlinked when CAS fails!"
    );

    // Immediate subsequent acquisition must succeed without RefLocked error
    let target1 = ReferenceTarget::Direct(oid1);
    let lock_ok = RefLock::acquire(&ref_path, Some(&target1));
    assert!(lock_ok.is_ok(), "Subsequent acquire must succeed cleanly");
}

#[test]
fn test_concurrent_ref_delete_and_write_mutual_exclusion() {
    let dir = tempdir().expect("tempdir");
    let mgr = Arc::new(RefManager::new(dir.path()));
    let oid1 = ObjectId::from_bytes([0x11; 32]);
    let oid2 = ObjectId::from_bytes([0x22; 32]);
    let target1 = ReferenceTarget::Direct(oid1);
    let target2 = ReferenceTarget::Direct(oid2);

    mgr.write_ref("refs/heads/test_branch", &target1, None, None)
        .unwrap();

    // 1. Holding RefLock on the ref prevents delete_ref
    let ref_path = mgr.ref_path("refs/heads/test_branch");
    let lock = RefLock::acquire(&ref_path, None).unwrap();
    let del_res = mgr.delete_ref("refs/heads/test_branch", None);
    assert!(
        matches!(del_res, Err(RefError::RefLocked(_))),
        "delete_ref must return RefLocked when ref is locked"
    );
    drop(lock);

    // 2. CAS delete rejects stale target
    let stale_del = mgr.delete_ref("refs/heads/test_branch", Some(&target2));
    assert!(
        matches!(stale_del, Err(RefError::CasMismatch { .. })),
        "delete_ref with stale target must return CasMismatch"
    );
    assert!(
        ref_path.exists(),
        "Ref file must remain intact on CasMismatch"
    );

    // 3. Valid CAS delete succeeds and cleans up lockfile
    let ok_del = mgr.delete_ref("refs/heads/test_branch", Some(&target1));
    assert!(ok_del.is_ok());
    assert!(!ref_path.exists(), "Ref file must be removed");
    let lock_path = dir.path().join("refs/heads/test_branch.lock");
    assert!(
        !lock_path.exists(),
        "Lock file must not be leaked after deletion"
    );
}
