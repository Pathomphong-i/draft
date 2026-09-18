//! Adversarial Concurrency & CAS Integrity Stress Tests (Challenger M3).
//!
//! Rigorously verifies:
//! 1. Intra-dimension lock mutual exclusion:
//!    - Multi-threaded lock contention: simultaneous `try_acquire` rejects competing agents with accurate holder metadata.
//!    - Timed blocking acquisition serialization: zero race conditions or lost updates under high-concurrency counter increments.
//!    - Timeout boundary mechanics: sub-timeout fails, super-timeout succeeds after holder releases.
//!    - Stale metadata crash recovery: unheld lock file with stale metadata is immediately overwritten.
//! 2. Cross-dimension concurrency:
//!    - 10+ parallel agents committing simultaneously across isolated dimensions.
//!    - Zero lock contention between distinct dimensions (measured acquisition latency < 100ms).
//!    - Zero blocking across dimensions.
//!    - 100% cryptographic CAS object store integrity under concurrent writes (loose objects verified with SHA-256).

use daft_core::cas::{ObjectId, ObjectType, RawObject};
use daft_core::init::{init, InitOptions};
use daft_core::object::{Commit, FileMode, Signature, Tree};
use daft_core::Repository;
use daft_dimension::lock::{DimensionLockError, DimensionLockGuard};
use daft_dimension::DimensionManager;
use std::collections::BTreeMap;
use std::fs;
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};
use tempfile::tempdir;

/// 1. Intra-dimension mutual exclusion: High-contention counter increment stress test.
/// 16 concurrent threads repeatedly contend for the SAME dimension lock,
/// reading, modifying, jitter-sleeping, and rewriting a shared workspace counter.
/// Invariant: Final counter value must EXACTLY equal 160 (16 threads * 10 increments),
/// proving zero race conditions, zero lost updates, and strict kernel-level serialization.
#[test]
fn test_adversarial_intra_dimension_counter_stress() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    init(&repo_root, &InitOptions::default()).unwrap();
    let repo = Arc::new(Repository::open(&repo_root.join(".dft")).unwrap());
    let manager = DimensionManager::new(repo.clone());
    manager.init().unwrap();
    manager
        .create_dimension("contested-dim", None, None)
        .unwrap();

    let dims_dir = manager.dimensions_dir().to_path_buf();
    let counter_path = dims_dir.join("contested-dim/workspace/counter.txt");
    fs::write(&counter_path, "0\n").unwrap();

    let thread_count = 16;
    let increments_per_thread = 10;
    let barrier = Arc::new(Barrier::new(thread_count));

    let mut handles = Vec::new();
    for thread_idx in 0..thread_count {
        let b = barrier.clone();
        let dims_dir_clone = dims_dir.clone();
        let counter_file = counter_path.clone();

        handles.push(std::thread::spawn(move || {
            b.wait(); // All 16 threads start contending at the exact same moment
            let agent_id = format!("agent-{}", thread_idx);

            for _ in 0..increments_per_thread {
                // Acquire exclusive dimension lock
                let guard = DimensionLockGuard::acquire_timeout(
                    &dims_dir_clone,
                    "contested-dim",
                    Some(&agent_id),
                    "increment",
                    Duration::from_secs(15),
                )
                .expect("Thread must acquire dimension lock within timeout");

                // Read current counter
                let content = fs::read_to_string(&counter_file).expect("Failed to read counter");
                let val: u64 = content.trim().parse().expect("Counter must be valid u64");

                // Introduce artificial micro-jitter while holding lock to test exclusion window
                std::thread::sleep(Duration::from_micros(200));

                // Write incremented counter
                let new_content = format!("{}\n", val + 1);
                fs::write(&counter_file, new_content).expect("Failed to write counter");

                drop(guard);
                // Brief pause before next cycle to allow fair scheduler handoff
                std::thread::sleep(Duration::from_micros(100));
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let final_content = fs::read_to_string(&counter_path).unwrap();
    let final_val: u64 = final_content.trim().parse().unwrap();
    let expected_val = (thread_count * increments_per_thread) as u64;

    assert_eq!(
        final_val, expected_val,
        "Race condition detected! Expected {}, got {}",
        expected_val, final_val
    );
}

/// 2. Intra-dimension lock mutual exclusion: Simultaneous try_acquire contention & metadata.
/// Verifies that when a holder holds the lock, all concurrent non-blocking try_acquire attempts
/// fail with LockContention and provide accurate holder metadata (pid, agent_id, operation).
#[test]
fn test_adversarial_intra_dimension_simultaneous_try_acquire() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    init(&repo_root, &InitOptions::default()).unwrap();
    let repo = Arc::new(Repository::open(&repo_root.join(".dft")).unwrap());
    let manager = DimensionManager::new(repo);
    manager.init().unwrap();
    manager.create_dimension("guarded-dim", None, None).unwrap();

    let dims_dir = manager.dimensions_dir().to_path_buf();

    // Acquire lock in main thread
    let holder_guard = DimensionLockGuard::acquire_timeout(
        &dims_dir,
        "guarded-dim",
        Some("agent-prime"),
        "rebase_in_progress",
        Duration::from_secs(1),
    )
    .expect("Main thread acquires lock");

    let num_attackers = 8;
    let barrier = Arc::new(Barrier::new(num_attackers));
    let mut handles = Vec::new();

    for attacker_idx in 0..num_attackers {
        let b = barrier.clone();
        let dims_dir_clone = dims_dir.clone();
        handles.push(std::thread::spawn(move || {
            b.wait();
            let attacker_id = format!("intruder-{}", attacker_idx);
            DimensionLockGuard::try_acquire(
                &dims_dir_clone,
                "guarded-dim",
                Some(&attacker_id),
                "commit",
            )
        }));
    }

    for handle in handles {
        let res = handle.join().unwrap();
        match res {
            Err(DimensionLockError::LockContention { dimension, holder }) => {
                assert_eq!(dimension, "guarded-dim");
                let info = holder.expect("Holder metadata must be recorded");
                assert_eq!(info.agent_id.as_deref(), Some("agent-prime"));
                assert_eq!(info.operation, "rebase_in_progress");
                assert_eq!(info.pid, std::process::id());
            }
            Ok(_) => panic!("Attacker unexpectedly acquired lock while held!"),
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    // Release holder lock
    drop(holder_guard);

    // Now an attacker can acquire cleanly
    let new_guard = DimensionLockGuard::try_acquire(
        &dims_dir,
        "guarded-dim",
        Some("intruder-0"),
        "commit_success",
    )
    .expect("Must acquire lock after holder release");
    drop(new_guard);
}

/// 3. Intra-dimension lock timeout mechanics & boundary behavior.
/// Verifies:
/// - Sub-timeout returns LockContention.
/// - Super-timeout blocks and acquires as soon as holder drops.
#[test]
fn test_adversarial_intra_dimension_timeout_precision() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    init(&repo_root, &InitOptions::default()).unwrap();
    let repo = Arc::new(Repository::open(&repo_root.join(".dft")).unwrap());
    let manager = DimensionManager::new(repo);
    manager.init().unwrap();
    manager.create_dimension("timing-dim", None, None).unwrap();
    let dims_dir = manager.dimensions_dir().to_path_buf();

    let (tx, rx) = std::sync::mpsc::channel();
    let dims_dir_clone = dims_dir.clone();

    // Holder thread holds lock for 200ms
    let holder = std::thread::spawn(move || {
        let guard = DimensionLockGuard::acquire_timeout(
            &dims_dir_clone,
            "timing-dim",
            Some("timing-holder"),
            "holding",
            Duration::from_millis(500),
        )
        .unwrap();
        tx.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        drop(guard);
    });

    rx.recv().unwrap(); // Wait until holder has acquired

    // 1. Sub-timeout: 40ms timeout must fail
    let start_sub = Instant::now();
    let sub_res = DimensionLockGuard::acquire_timeout(
        &dims_dir,
        "timing-dim",
        Some("impatient-agent"),
        "try",
        Duration::from_millis(40),
    );
    let sub_elapsed = start_sub.elapsed();
    assert!(sub_res.is_err(), "Sub-timeout should fail with contention");
    assert!(
        sub_elapsed >= Duration::from_millis(30),
        "Sub-timeout returned too quickly: {:?}",
        sub_elapsed
    );

    // 2. Super-timeout: 600ms timeout must succeed after holder finishes (~200ms)
    let start_super = Instant::now();
    let super_res = DimensionLockGuard::acquire_timeout(
        &dims_dir,
        "timing-dim",
        Some("patient-agent"),
        "try",
        Duration::from_millis(600),
    );
    let super_elapsed = start_super.elapsed();
    assert!(
        super_res.is_ok(),
        "Super-timeout should succeed once holder drops"
    );
    assert!(
        super_elapsed >= Duration::from_millis(100),
        "Acquired too fast before holder released: {:?}",
        super_elapsed
    );

    holder.join().unwrap();
}

/// 4. Stale metadata crash recovery.
/// Simulates an abrupt process crash where JSON metadata was left on disk,
/// but no kernel flock is held.
/// Verifies that a new process/thread immediately acquires the lock without false contention,
/// and overwrites the stale metadata cleanly.
#[test]
fn test_adversarial_intra_dimension_stale_metadata_recovery() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    init(&repo_root, &InitOptions::default()).unwrap();
    let repo = Arc::new(Repository::open(&repo_root.join(".dft")).unwrap());
    let manager = DimensionManager::new(repo);
    manager.init().unwrap();
    manager.create_dimension("crashed-dim", None, None).unwrap();
    let dims_dir = manager.dimensions_dir().to_path_buf();

    // Fabricate stale lock file from a dead process
    let stale_lock_path = dims_dir.join("crashed-dim/.lock");
    fs::write(
        &stale_lock_path,
        r#"{"pid": 999999, "agent_id": "ghost-agent", "operation": "crashed_op", "acquired_at": "2020-01-01T00:00:00Z"}"#,
    )
    .unwrap();

    // New agent tries to acquire
    let guard = DimensionLockGuard::try_acquire(
        &dims_dir,
        "crashed-dim",
        Some("survivor-agent"),
        "recovery_op",
    )
    .expect("Should acquire lock immediately despite stale metadata since no flock is held");

    // Verify metadata was overwritten
    let content = fs::read_to_string(&stale_lock_path).unwrap();
    assert!(
        content.contains("survivor-agent"),
        "Metadata was not updated"
    );
    assert!(
        !content.contains("ghost-agent"),
        "Stale agent metadata persisted"
    );

    drop(guard);
}

/// 5. Cross-dimension concurrency: 10 parallel agents committing simultaneously.
/// Verifies:
/// - Zero lock contention between different dimensions (lock latency < 100ms for each agent).
/// - Zero cross-dimension blocking.
/// - Simultaneous writes of both shared CAS blobs (CAS race deduplication) and distinct blobs.
/// - 100% cryptographic CAS object store integrity under concurrent writes.
#[test]
fn test_adversarial_cross_dimension_10_agents_simultaneous_commits() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    init(&repo_root, &InitOptions::default()).unwrap();
    let repo = Arc::new(Repository::open(&repo_root.join(".dft")).unwrap());
    let manager = DimensionManager::new(repo.clone());
    manager.init().unwrap();

    let agent_count = 10;
    let mut dimensions = Vec::new();
    for i in 0..agent_count {
        let dim_name = format!("dim-agent-{:02}", i);
        manager.create_dimension(&dim_name, None, None).unwrap();
        dimensions.push(dim_name);
    }

    let barrier = Arc::new(Barrier::new(agent_count));
    let start_all = Instant::now();

    let shared_payload = b"Common shared library code used across all dimensions 42";

    std::thread::scope(|s| {
        for (i, dim_name) in dimensions.iter().enumerate() {
            let b = barrier.clone();
            let repo_clone = repo.clone();
            let dim_str = dim_name.clone();
            let agent_id = format!("agent-{:02}", i);
            let dims_dir = repo_clone.dft_dir().join("dimensions");

            s.spawn(move || {
                // Synchronize all 10 agents to start at the exact same instant
                b.wait();

                // 1. Measure per-dimension lock acquisition latency
                let lock_start = Instant::now();
                let guard = DimensionLockGuard::acquire_timeout(
                    &dims_dir,
                    &dim_str,
                    Some(&agent_id),
                    "commit_feature",
                    Duration::from_secs(5),
                )
                .expect("Cross-dimension lock must succeed without contention");
                let lock_elapsed = lock_start.elapsed();

                assert!(
                    lock_elapsed < Duration::from_millis(100),
                    "Lock acquisition on distinct dimension took too long: {:?}",
                    lock_elapsed
                );

                // 2. Write files in dimension workspace
                let ws = dims_dir.join(&dim_str).join("workspace");
                let private_content =
                    format!("Private data for {} at {:?}", agent_id, Instant::now());
                fs::write(ws.join("private.txt"), &private_content).unwrap();
                fs::write(ws.join("common.txt"), shared_payload).unwrap();

                // 3. Write blobs to shared CAS (concurrent CAS writes with overlapping and unique keys)
                let private_blob_oid = repo_clone
                    .cas()
                    .write_blob(private_content.as_bytes())
                    .unwrap();
                let shared_blob_oid = repo_clone.cas().write_blob(shared_payload).unwrap();

                // 4. Build CAS tree
                let mut files = BTreeMap::new();
                files.insert(
                    "private.txt".to_string(),
                    (FileMode(0o100644), private_blob_oid),
                );
                files.insert(
                    "common.txt".to_string(),
                    (FileMode(0o100644), shared_blob_oid),
                );
                let tree_oid =
                    daft_core::merge::build_hierarchical_tree(repo_clone.cas().as_ref(), &files)
                        .unwrap();

                // 5. Build Commit object
                let sig = Signature::now(&agent_id, format!("{}@agents.daft", agent_id));
                let commit = Commit::new(
                    tree_oid,
                    vec![],
                    sig.clone(),
                    sig,
                    format!("Parallel commit by {}", agent_id),
                );
                let raw_commit = RawObject::new(ObjectType::Commit, commit.serialize());
                let commit_oid = repo_clone.cas().write_raw(&raw_commit).unwrap();

                // 6. Update dimension HEAD
                fs::write(
                    dims_dir.join(&dim_str).join("HEAD"),
                    format!("{}\n", commit_oid.to_hex()),
                )
                .unwrap();

                drop(guard);
            });
        }
    });

    let total_elapsed = start_all.elapsed();
    println!("⏱️  10 parallel agents committed in: {:?}", total_elapsed);
    assert!(
        total_elapsed < Duration::from_secs(3),
        "Parallel commits exceeded SLA: {:?}",
        total_elapsed
    );

    // 7. Rigorous CAS Integrity Audit across all objects created
    let all_objects = repo.cas().list_objects().unwrap();
    assert!(
        all_objects.len() >= agent_count * 2,
        "Expected at least {} objects in CAS store, found {}",
        agent_count * 2,
        all_objects.len()
    );

    for oid in &all_objects {
        // read_raw_verified computes SHA-256 and asserts cryptographic identity
        let raw = repo
            .cas()
            .read_raw_verified(oid)
            .unwrap_or_else(|e| panic!("CAS object {} failed verification: {:?}", oid.to_hex(), e));

        match raw.object_type {
            ObjectType::Commit => {
                let commit = Commit::deserialize(&raw.data).unwrap();
                // Verify tree referenced by commit exists and is verified
                let tree_raw = repo.cas().read_raw_verified(&commit.tree).unwrap();
                assert_eq!(tree_raw.object_type, ObjectType::Tree);
            }
            ObjectType::Tree => {
                let tree = Tree::deserialize(&raw.data).unwrap();
                for entry in tree.entries() {
                    // Verify every tree entry exists in CAS and passes SHA-256 check
                    assert!(
                        repo.cas().has_object(&entry.oid),
                        "Tree references nonexistent CAS object: {}",
                        entry.oid.to_hex()
                    );
                }
            }
            ObjectType::Blob => {
                assert!(!raw.data.is_empty(), "Blob should not be empty");
            }
            _ => {}
        }
    }

    // 8. Verify each dimension's HEAD matches the agent commit
    for (i, dim_name) in dimensions.iter().enumerate() {
        let head_path = repo
            .dft_dir()
            .join("dimensions")
            .join(dim_name)
            .join("HEAD");
        let head_hex = fs::read_to_string(head_path).unwrap().trim().to_string();
        let oid = ObjectId::from_hex(&head_hex).unwrap();
        let raw = repo.cas().read_raw_verified(&oid).unwrap();
        assert_eq!(raw.object_type, ObjectType::Commit);
        let commit = Commit::deserialize(&raw.data).unwrap();
        assert_eq!(commit.message, format!("Parallel commit by agent-{:02}", i));
    }
}

/// 6. Multi-round pipeline stress test across parallel dimensions.
/// 6 agents each executing 4 consecutive commit rounds (24 commits total) in parallel.
/// Tests sustained concurrent write-ahead CAS deduplication and ref integrity.
#[test]
fn test_adversarial_cross_dimension_multi_round_pipeline() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    init(&repo_root, &InitOptions::default()).unwrap();
    let repo = Arc::new(Repository::open(&repo_root.join(".dft")).unwrap());
    let manager = DimensionManager::new(repo.clone());
    manager.init().unwrap();

    let num_agents = 6;
    let rounds = 4;
    let mut dims = Vec::new();
    for i in 0..num_agents {
        let dim_name = format!("pipeline-dim-{}", i);
        manager.create_dimension(&dim_name, None, None).unwrap();
        dims.push(dim_name);
    }

    let barrier = Arc::new(Barrier::new(num_agents));

    std::thread::scope(|s| {
        for (i, dim_name) in dims.iter().enumerate() {
            let b = barrier.clone();
            let repo_clone = repo.clone();
            let dim_str = dim_name.clone();
            let agent_id = format!("worker-{}", i);
            let dims_dir = repo_clone.dft_dir().join("dimensions");

            s.spawn(move || {
                b.wait();
                let mut parent_commit: Option<ObjectId> = None;

                for round in 0..rounds {
                    let guard = DimensionLockGuard::acquire_timeout(
                        &dims_dir,
                        &dim_str,
                        Some(&agent_id),
                        "pipeline_commit",
                        Duration::from_secs(5),
                    )
                    .unwrap();

                    // Create unique file content
                    let data = format!("Payload {}:{} @ {:?}", agent_id, round, Instant::now());
                    let blob_oid = repo_clone.cas().write_blob(data.as_bytes()).unwrap();

                    let mut files = BTreeMap::new();
                    files.insert(
                        format!("file_{}.txt", round),
                        (FileMode(0o100644), blob_oid),
                    );
                    let tree_oid = daft_core::merge::build_hierarchical_tree(
                        repo_clone.cas().as_ref(),
                        &files,
                    )
                    .unwrap();

                    let sig = Signature::now(&agent_id, format!("{}@agents.daft", agent_id));
                    let parents = parent_commit.into_iter().collect();
                    let commit = Commit::new(
                        tree_oid,
                        parents,
                        sig.clone(),
                        sig,
                        format!("Commit {}:{}", agent_id, round),
                    );
                    let raw_commit = RawObject::new(ObjectType::Commit, commit.serialize());
                    let commit_oid = repo_clone.cas().write_raw(&raw_commit).unwrap();

                    fs::write(
                        dims_dir.join(&dim_str).join("HEAD"),
                        format!("{}\n", commit_oid.to_hex()),
                    )
                    .unwrap();

                    parent_commit = Some(commit_oid);
                    drop(guard);
                }
            });
        }
    });

    // Verify 100% of objects in CAS are valid
    let objects = repo.cas().list_objects().unwrap();
    for oid in &objects {
        repo.cas()
            .read_raw_verified(oid)
            .expect("Every CAS object must pass SHA-256 verification");
    }

    // Verify final commit depth for each dimension
    for dim_name in &dims {
        let head_path = repo
            .dft_dir()
            .join("dimensions")
            .join(dim_name)
            .join("HEAD");
        let head_hex = fs::read_to_string(head_path).unwrap().trim().to_string();
        let oid = ObjectId::from_hex(&head_hex).unwrap();
        let raw = repo.cas().read_raw_verified(&oid).unwrap();
        let commit = Commit::deserialize(&raw.data).unwrap();
        assert_eq!(
            commit.parents.len(),
            1,
            "Latest commit should have 1 parent"
        );
    }
}
