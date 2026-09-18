use daft_core::cas::{ObjectId, ObjectStore};
use daft_core::error::{CasError, IndexError, RefError};
use daft_core::index::lock::IndexLock;
use daft_core::index::{Index, IndexEntry, Stage};
use daft_core::refs::lock::RefLock;
use daft_core::refs::ReferenceTarget;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_stress_50plus_threads_identical_writes() {
    const NUM_THREADS: usize = 64;
    let dir = tempdir().expect("tempdir");
    let store = Arc::new(ObjectStore::init(dir.path().join("objects")).expect("init store"));

    // 128 KB payload
    let payload = Arc::new(vec![0x7A; 128 * 1024]);
    let barrier = Arc::new(Barrier::new(NUM_THREADS));
    let mut handles = Vec::with_capacity(NUM_THREADS);

    for _ in 0..NUM_THREADS {
        let store_c = Arc::clone(&store);
        let payload_c = Arc::clone(&payload);
        let barrier_c = Arc::clone(&barrier);

        handles.push(thread::spawn(move || {
            barrier_c.wait();
            store_c.write_blob(&payload_c).expect("write blob")
        }));
    }

    let mut oids = Vec::with_capacity(NUM_THREADS);
    for h in handles {
        oids.push(h.join().expect("join"));
    }

    // All threads must agree on identical OID
    let first = oids[0];
    for oid in &oids {
        assert_eq!(oid, &first, "all threads must return identical OID");
    }

    // Store must have exactly 1 object
    let objects = store.list_objects().expect("list");
    assert_eq!(objects.len(), 1, "exactly 1 object stored");
    assert_eq!(objects[0], first);

    // Verified read
    let verified = store.read_raw_verified(&first).expect("read verified");
    assert_eq!(verified.data, *payload);

    // Tmp directory must be completely clean (no leftover temporary files)
    let tmp_entries: Vec<_> = fs::read_dir(store.tmp_path()).expect("read tmp").collect();
    assert_eq!(
        tmp_entries.len(),
        0,
        "tmp directory must not leak orphaned files after identical writes"
    );
}

#[test]
fn test_stress_50plus_threads_distinct_writes() {
    const NUM_THREADS: usize = 64;
    const OBJS_PER_THREAD: usize = 25;
    let total_expected = NUM_THREADS * OBJS_PER_THREAD;

    let dir = tempdir().expect("tempdir");
    let store = Arc::new(ObjectStore::init(dir.path().join("objects")).expect("init store"));
    let barrier = Arc::new(Barrier::new(NUM_THREADS));
    let mut handles = Vec::with_capacity(NUM_THREADS);

    for t_idx in 0..NUM_THREADS {
        let store_c = Arc::clone(&store);
        let barrier_c = Arc::clone(&barrier);

        handles.push(thread::spawn(move || {
            barrier_c.wait();
            let mut results = Vec::with_capacity(OBJS_PER_THREAD);
            for i in 0..OBJS_PER_THREAD {
                // Vary payload content and sizes
                let content = format!(
                    "thread-{:03}-obj-{:04}-content-{}",
                    t_idx,
                    i,
                    "x".repeat(50 + (i % 200))
                );
                let oid = store_c
                    .write_blob(content.as_bytes())
                    .expect("write distinct blob");
                results.push((oid, content));
            }
            results
        }));
    }

    let mut all_results = Vec::with_capacity(total_expected);
    for h in handles {
        let res = h.join().expect("join");
        all_results.extend(res);
    }

    assert_eq!(all_results.len(), total_expected);

    // Verify all objects in CAS
    let stored = store.list_objects().expect("list objects");
    assert_eq!(
        stored.len(),
        total_expected,
        "all distinct objects must be in CAS store"
    );

    // Concurrently verify all objects via mmap and verified read
    let verify_barrier = Arc::new(Barrier::new(NUM_THREADS));
    let chunk_size = total_expected / NUM_THREADS;
    let mut v_handles = Vec::with_capacity(NUM_THREADS);
    let all_arc = Arc::new(all_results);

    for t_idx in 0..NUM_THREADS {
        let store_c = Arc::clone(&store);
        let all_c = Arc::clone(&all_arc);
        let vb_c = Arc::clone(&verify_barrier);

        v_handles.push(thread::spawn(move || {
            vb_c.wait();
            let start = t_idx * chunk_size;
            let end = if t_idx == NUM_THREADS - 1 {
                all_c.len()
            } else {
                start + chunk_size
            };

            for i in start..end {
                let (ref oid, ref expected_content) = all_c[i];
                let raw = store_c.read_raw_verified(oid).expect("read verified");
                assert_eq!(raw.data, expected_content.as_bytes());

                let mmap = store_c.read_raw_mmap(oid).expect("read mmap");
                assert_eq!(mmap.data, expected_content.as_bytes());
            }
        }));
    }

    for vh in v_handles {
        vh.join().expect("join verify");
    }
}

#[test]
fn test_stress_concurrent_mixed_readers_and_writers() {
    const WRITERS: usize = 32;
    const READERS: usize = 32;
    const TOTAL_THREADS: usize = WRITERS + READERS;
    const OBJS_PER_WRITER: usize = 20;
    let total_objs = WRITERS * OBJS_PER_WRITER;

    let dir = tempdir().expect("tempdir");
    let store = Arc::new(ObjectStore::init(dir.path().join("objects")).expect("init store"));
    let shared_pool = Arc::new(Mutex::new(Vec::new()));
    let writers_done = Arc::new(AtomicBool::new(false));
    let read_count = Arc::new(AtomicUsize::new(0));

    let barrier = Arc::new(Barrier::new(TOTAL_THREADS));
    let mut handles = Vec::with_capacity(TOTAL_THREADS);

    // Spawn writers
    for w_idx in 0..WRITERS {
        let store_c = Arc::clone(&store);
        let pool_c = Arc::clone(&shared_pool);
        let barrier_c = Arc::clone(&barrier);

        handles.push(thread::spawn(move || {
            barrier_c.wait();
            for i in 0..OBJS_PER_WRITER {
                let payload = format!("writer-{}-val-{}", w_idx, i);
                let oid = store_c.write_blob(payload.as_bytes()).expect("write blob");
                {
                    let mut lock = pool_c.lock().unwrap();
                    lock.push((oid, payload));
                }
                thread::yield_now();
            }
        }));
    }

    // Spawn readers
    for _ in 0..READERS {
        let store_c = Arc::clone(&store);
        let pool_c = Arc::clone(&shared_pool);
        let barrier_c = Arc::clone(&barrier);
        let done_c = Arc::clone(&writers_done);
        let read_cnt_c = Arc::clone(&read_count);

        handles.push(thread::spawn(move || {
            barrier_c.wait();
            while !done_c.load(Ordering::Relaxed) {
                let target = {
                    let lock = pool_c.lock().unwrap();
                    if lock.is_empty() {
                        None
                    } else {
                        // Pick random or last entry
                        let idx = lock.len() - 1;
                        Some(lock[idx].clone())
                    }
                };

                if let Some((oid, expected)) = target {
                    let read_data = store_c.read_blob(&oid).expect("read blob in flight");
                    assert_eq!(read_data, expected.as_bytes());
                    read_cnt_c.fetch_add(1, Ordering::Relaxed);
                }
                thread::sleep(Duration::from_micros(200));
            }
        }));
    }

    // Wait for writers to complete
    for h in handles.drain(0..WRITERS) {
        h.join().expect("writer join");
    }
    writers_done.store(true, Ordering::Relaxed);

    // Wait for readers to complete
    for h in handles {
        h.join().expect("reader join");
    }

    assert!(
        read_count.load(Ordering::Relaxed) > 50,
        "concurrent readers must successfully read in-flight objects"
    );

    let final_objects = store.list_objects().expect("list");
    assert_eq!(final_objects.len(), total_objs);
}

#[test]
fn test_stress_simulated_crash_mid_write_and_recovery() {
    let dir = tempdir().expect("tempdir");
    let store = ObjectStore::init(dir.path().join("objects")).expect("init store");

    // Simulate scenario: process killed mid-write leaving various partial files in tmp/
    let tmp_path = store.tmp_path();

    // 1. Zero-byte incomplete file
    fs::write(tmp_path.join("abandoned_0byte.tmp"), b"").expect("write 0byte");

    // 2. Partial uncompressed junk
    fs::write(
        tmp_path.join("abandoned_junk.tmp"),
        b"partial truncated stream",
    )
    .expect("write junk");

    // 3. Truncated zlib stream
    let truncated_zlib_path = tmp_path.join("abandoned_zlib.tmp");
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&truncated_zlib_path)
            .expect("open");
        let mut enc = ZlibEncoder::new(&mut file, Compression::default());
        enc.write_all(b"half data").expect("write");
        // deliberately omit enc.finish() so stream is incomplete
    }

    // Assert that CAS operations are completely immune to orphaned tmp files
    let test_data = b"resilient write under crash debris";
    let oid = store.write_blob(test_data).expect("write blob");
    let read_back = store.read_raw_verified(&oid).expect("read verified");
    assert_eq!(read_back.data, test_data);

    // Verify list_objects does NOT count any tmp debris
    let objects = store.list_objects().expect("list");
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0], oid);

    // Clean stale tmp files older than 1ms
    thread::sleep(Duration::from_millis(20));
    let cleaned = store
        .clean_stale_tmp(Duration::from_millis(5))
        .expect("clean stale");
    assert_eq!(cleaned, 3, "must clean exactly 3 abandoned files");

    let remaining_tmp: Vec<_> = fs::read_dir(tmp_path).expect("read dir").collect();
    assert_eq!(remaining_tmp.len(), 0, "tmp must be empty after cleanup");
}

#[test]
fn test_stress_object_corruption_detection() {
    let dir = tempdir().expect("tempdir");
    let store = ObjectStore::init(dir.path().join("objects")).expect("init store");

    let valid_data = b"important immutable commit data";
    let oid = store.write_blob(valid_data).expect("write");
    let obj_path = store.object_path(&oid);
    assert!(obj_path.exists());

    // Temporarily make file writable on Unix to simulate bit rot / disk corruption
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&obj_path, fs::Permissions::from_mode(0o644)).expect("chmod 644");
    }

    // Corrupt the zlib contents: flip several bytes
    let mut raw_bytes = fs::read(&obj_path).expect("read obj file");
    for i in 0..5.min(raw_bytes.len()) {
        raw_bytes[i] ^= 0xFF;
    }
    fs::write(&obj_path, &raw_bytes).expect("write corrupted");

    // read_raw should fail with Decompression error, NOT panic
    let read_res = store.read_raw(&oid);
    assert!(
        matches!(read_res, Err(CasError::Decompression(_))),
        "corrupted zlib must return CasError::Decompression, got {:?}",
        read_res
    );

    // read_raw_verified must also fail safely
    let verified_res = store.read_raw_verified(&oid);
    assert!(
        verified_res.is_err(),
        "verified read must reject corruption"
    );
}

#[test]
fn test_stress_index_lock_concurrency_and_orphans() {
    const THREADS: usize = 50;
    let dir = tempdir().expect("tempdir");
    let index_path = dir.path().join("index");

    let barrier = Arc::new(Barrier::new(THREADS));
    let success_count = Arc::new(AtomicUsize::new(0));
    let locked_count = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(THREADS);

    for _ in 0..THREADS {
        let index_path_c = index_path.clone();
        let barrier_c = Arc::clone(&barrier);
        let succ_c = Arc::clone(&success_count);
        let lock_c = Arc::clone(&locked_count);

        handles.push(thread::spawn(move || {
            barrier_c.wait();
            match IndexLock::acquire(&index_path_c) {
                Ok(mut lock) => {
                    succ_c.fetch_add(1, Ordering::SeqCst);
                    // Hold lock briefly, write small index
                    let mut idx = Index::new();
                    let entry =
                        IndexEntry::new("file.txt", ObjectId::ZERO, 0o100644, Stage::Normal, 10)
                            .expect("entry");
                    idx.add_entry(entry);
                    lock.write_index(&idx).expect("write index");
                    thread::sleep(Duration::from_millis(50));
                    lock.commit().expect("commit index");
                }
                Err(IndexError::IndexLocked(_)) => {
                    lock_c.fetch_add(1, Ordering::SeqCst);
                }
                Err(other) => panic!("unexpected error: {:?}", other),
            }
        }));
    }

    for h in handles {
        h.join().expect("join");
    }

    // Under strict concurrency, exactly 1 thread acquired the lock during the race
    assert_eq!(
        success_count.load(Ordering::SeqCst),
        1,
        "exactly 1 thread should succeed in race"
    );
    assert_eq!(
        locked_count.load(Ordering::SeqCst),
        THREADS - 1,
        "all other threads should receive IndexError::IndexLocked"
    );
    assert!(index_path.exists(), "committed index must exist");

    // Orphan lock test: simulate hard crash leaving orphan .lock file
    let lock_file = PathBuf::from(format!("{}.lock", index_path.display()));
    fs::write(&lock_file, b"orphan pid 9999").expect("write orphan lock");

    // Acquire must fail cleanly with IndexLocked
    let orphan_res = IndexLock::acquire(&index_path);
    assert!(
        matches!(orphan_res, Err(IndexError::IndexLocked(_))),
        "orphan lock must prevent acquisition with IndexLocked"
    );

    // Remove orphan lock manually and verify acquisition succeeds immediately
    fs::remove_file(&lock_file).expect("remove orphan");
    let recovered_lock = IndexLock::acquire(&index_path);
    assert!(
        recovered_lock.is_ok(),
        "acquisition must succeed after orphan lock cleanup"
    );
}

use std::path::PathBuf;

#[test]
fn test_stress_ref_lock_concurrency_and_orphans() {
    const THREADS: usize = 50;
    let dir = tempdir().expect("tempdir");
    let ref_path = dir.path().join("refs/heads/feature");

    let barrier = Arc::new(Barrier::new(THREADS));
    let success_count = Arc::new(AtomicUsize::new(0));
    let locked_count = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(THREADS);

    for _ in 0..THREADS {
        let ref_path_c = ref_path.clone();
        let barrier_c = Arc::clone(&barrier);
        let succ_c = Arc::clone(&success_count);
        let lock_c = Arc::clone(&locked_count);

        handles.push(thread::spawn(move || {
            barrier_c.wait();
            match RefLock::acquire(&ref_path_c, None) {
                Ok(mut lock) => {
                    succ_c.fetch_add(1, Ordering::SeqCst);
                    let target = ReferenceTarget::Direct(ObjectId::from_bytes([7u8; 32]));
                    lock.write_target(&target).expect("write target");
                    thread::sleep(Duration::from_millis(50));
                    lock.commit().expect("commit ref");
                }
                Err(RefError::RefLocked(_)) => {
                    lock_c.fetch_add(1, Ordering::SeqCst);
                }
                Err(other) => panic!("unexpected error: {:?}", other),
            }
        }));
    }

    for h in handles {
        h.join().expect("join");
    }

    assert_eq!(
        success_count.load(Ordering::SeqCst),
        1,
        "exactly 1 thread should acquire ref lock"
    );
    assert_eq!(
        locked_count.load(Ordering::SeqCst),
        THREADS - 1,
        "other threads must get RefLocked"
    );
    assert!(ref_path.exists());

    // Orphan lock test: simulate hard crash leaving orphan ref lock
    let orphan_lock = PathBuf::from(format!("{}.lock", ref_path.display()));
    fs::write(&orphan_lock, b"orphan").expect("write orphan ref lock");

    let orphan_res = RefLock::acquire(&ref_path, None);
    assert!(
        matches!(orphan_res, Err(RefError::RefLocked(_))),
        "orphan ref lock must block acquisition"
    );

    // Clean orphan lock and verify recovery
    fs::remove_file(&orphan_lock).expect("remove orphan");
    let recovered = RefLock::acquire(&ref_path, None);
    assert!(
        recovered.is_ok(),
        "ref lock acquisition recovers after orphan removal"
    );
}
