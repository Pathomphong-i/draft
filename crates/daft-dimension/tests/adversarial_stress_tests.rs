//! Empirical Adversarial Stress Harness for Daft Dimension Engine (Milestone 3).
//!
//! Tests:
//! 1. CoW Cloning Benchmark (1,000 files, < 5.0s SLA, Clonefile/Hardlink/Copy strategies)
//! 2. Break-on-Write (BoW) hardlink unlinking & isolation
//! 3. Live State Fork (dirty workspace: unstaged edits, untracked files, staged index entries)
//!    via CLI and API, with zero dummy commits and two-way isolation
//! 4. Snapshot Lifecycle (create, list, rejection without force, restore with force, delete)

use daft_core::index::{Index, Stage};
use daft_core::init::{init, InitOptions};
use daft_core::Repository;
use daft_dimension::cow::{
    break_on_write_check_and_unlink, detect_best_strategy, CowEngine, CowStrategy,
};
use daft_dimension::snapshot::SnapshotManager;
use daft_dimension::DimensionManager;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use walkdir::WalkDir;

/// Finds the `dft` executable binary.
fn find_dft_bin() -> PathBuf {
    let candidates = [
        PathBuf::from("../../target/debug/dft"),
        PathBuf::from("target/debug/dft"),
        PathBuf::from("../target/debug/dft"),
    ];
    for c in &candidates {
        if c.exists() {
            return c.canonicalize().unwrap();
        }
    }
    panic!("Could not locate target/debug/dft executable");
}

/// Generates a synthetic repository with `count` files distributed across nested directories.
fn generate_repo_files(workdir: &Path, count: usize) {
    for i in 0..count {
        let dir_idx = i % 50;
        let sub_dir = workdir.join(format!("pkg_{:02}/sub_{:02}", dir_idx / 5, dir_idx % 5));
        fs::create_dir_all(&sub_dir).expect("create dir");

        let file_path = sub_dir.join(format!("source_file_{:04}.rs", i));
        let mut f = File::create(&file_path).expect("create file");
        writeln!(f, "// Daft Synthetic Source File {}", i).unwrap();
        writeln!(f, "pub fn routine_{}(val: u64) -> u64 {{", i).unwrap();
        writeln!(f, "    let salt = 0x{:08x};", i * 1337).unwrap();
        writeln!(f, "    val.wrapping_mul(salt).rotate_left(3)").unwrap();
        writeln!(f, "}}").unwrap();
    }
}

// ============================================================================
// 1. CoW Cloning Benchmark Stress (Features 38 & 63)
// ============================================================================

#[test]
fn stress_test_cow_1000_files_benchmark_all_strategies() {
    let tmp = tempdir().expect("tempdir");
    let repo_root = tmp.path().join("repo_benchmark");
    fs::create_dir_all(&repo_root).unwrap();

    init(&repo_root, &InitOptions::default()).expect("init repo");
    let file_count = 1000;
    println!("[BENCHMARK] Generating 1,000 files in synthetic workspace...");
    generate_repo_files(&repo_root, file_count);

    // Verify generation
    let mut generated_count = 0;
    for entry in WalkDir::new(&repo_root).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() && !entry.path().starts_with(repo_root.join(".dft")) {
            generated_count += 1;
        }
    }
    assert_eq!(generated_count, file_count, "Generated file count mismatch");

    // Benchmark Strategy 1: Detected Optimal Strategy (Clonefile on macOS APFS)
    let detected_strat = detect_best_strategy(&repo_root);
    let target_detected = repo_root.join(".dft/dimensions/dim_detected/workspace");
    fs::create_dir_all(&target_detected).unwrap();

    let start_detected = Instant::now();
    CowEngine::clone_workspace(
        &repo_root,
        &target_detected,
        detected_strat,
        Some(&repo_root.join(".dft")),
    )
    .expect("clone detected");
    let dur_detected = start_detected.elapsed();

    println!(
        "⏱️  [BENCHMARK] Detected Strategy ({}) clone: {:.4}s (SLA: < 5.0s)",
        detected_strat,
        dur_detected.as_secs_f64()
    );
    assert!(
        dur_detected < Duration::from_secs(5),
        "Detected CoW clone SLA violated: took {:.4}s >= 5.0s",
        dur_detected.as_secs_f64()
    );

    // Verify cloned file count
    let cloned_count: usize = WalkDir::new(&target_detected)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .count();
    assert_eq!(cloned_count, file_count, "Cloned file count mismatch");

    // Benchmark Strategy 2: POSIX Hardlinks
    let target_hardlink = repo_root.join(".dft/dimensions/dim_hardlink/workspace");
    fs::create_dir_all(&target_hardlink).unwrap();

    let start_hardlink = Instant::now();
    CowEngine::clone_workspace(
        &repo_root,
        &target_hardlink,
        CowStrategy::Hardlink,
        Some(&repo_root.join(".dft")),
    )
    .expect("clone hardlink");
    let dur_hardlink = start_hardlink.elapsed();

    println!(
        "⏱️  [BENCHMARK] Hardlink Strategy clone: {:.4}s (SLA: < 5.0s)",
        dur_hardlink.as_secs_f64()
    );
    assert!(
        dur_hardlink < Duration::from_secs(5),
        "Hardlink CoW clone SLA violated: took {:.4}s >= 5.0s",
        dur_hardlink.as_secs_f64()
    );

    // Benchmark Strategy 3: Multi-threaded Copy Fallback (Rayon)
    let target_copy = repo_root.join(".dft/dimensions/dim_copy/workspace");
    fs::create_dir_all(&target_copy).unwrap();

    let start_copy = Instant::now();
    CowEngine::clone_workspace(
        &repo_root,
        &target_copy,
        CowStrategy::Copy,
        Some(&repo_root.join(".dft")),
    )
    .expect("clone copy");
    let dur_copy = start_copy.elapsed();

    println!(
        "⏱️  [BENCHMARK] Rayon Parallel Copy clone: {:.4}s (SLA: < 5.0s)",
        dur_copy.as_secs_f64()
    );
    assert!(
        dur_copy < Duration::from_secs(5),
        "Rayon Copy clone SLA violated: took {:.4}s >= 5.0s",
        dur_copy.as_secs_f64()
    );
}

#[test]
fn stress_test_break_on_write_semantics() {
    let tmp = tempdir().unwrap();
    let dim_a_dir = tmp.path().join("dim_a");
    let dim_b_dir = tmp.path().join("dim_b");
    fs::create_dir_all(&dim_a_dir).unwrap();
    fs::create_dir_all(&dim_b_dir).unwrap();

    let file_a = dim_a_dir.join("shared_module.rs");
    let file_b = dim_b_dir.join("shared_module.rs");

    let initial_data = "// Pristine Shared Module V1\npub const K: u32 = 42;\n";
    fs::write(&file_a, initial_data).unwrap();

    // Create hardlink between dim_a and dim_b
    fs::hard_link(&file_a, &file_b).expect("hardlink");

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(
            fs::symlink_metadata(&file_a).unwrap().nlink(),
            2,
            "file_a nlink must be 2"
        );
        assert_eq!(
            fs::symlink_metadata(&file_b).unwrap().nlink(),
            2,
            "file_b nlink must be 2"
        );
    }

    // Step 1: Execute Break-on-Write check on file_b
    let was_unlinked = break_on_write_check_and_unlink(&file_b).expect("bow check");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert!(was_unlinked, "File with nlink > 1 must be unlinked");
        assert!(
            !file_b.exists(),
            "file_b should be unlinked before rewriting"
        );
        assert_eq!(
            fs::symlink_metadata(&file_a).unwrap().nlink(),
            1,
            "file_a nlink must now be 1"
        );
    }

    // Step 2: Write mutated content to file_b
    let mutated_data = "// Mutated in Dim B\npub const K: u32 = 9999;\n";
    fs::write(&file_b, mutated_data).unwrap();

    // Step 3: Assert strict isolation
    assert_eq!(
        fs::read_to_string(&file_a).unwrap(),
        initial_data,
        "Original file_a in Dim A was altered by mutation in Dim B!"
    );
    assert_eq!(
        fs::read_to_string(&file_b).unwrap(),
        mutated_data,
        "Mutated file_b in Dim B content mismatch"
    );

    // Step 4: Repeating BoW on an unlinked file (nlink == 1) should be a no-op
    let second_bow = break_on_write_check_and_unlink(&file_b).unwrap();
    assert!(
        !second_bow,
        "BoW on nlink == 1 should return false and not unlink"
    );
    assert!(file_b.exists(), "file_b should still exist after no-op BoW");
}

// ============================================================================
// 2. Live State Fork Stress (Feature 41)
// ============================================================================

#[test]
fn stress_test_live_state_fork_cli_and_lib() {
    let dft_bin = find_dft_bin();
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo_live_fork");
    fs::create_dir_all(&repo_root).unwrap();

    let run_cmd = |args: &[&str]| {
        let out = Command::new(&dft_bin)
            .current_dir(&repo_root)
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("Failed to execute dft {:?}: {}", args, e));
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(
            out.status.success(),
            "Command 'dft {:?}' failed!\nSTDOUT: {}\nSTDERR: {}",
            args,
            stdout,
            stderr
        );
        stdout
    };

    // 1. Initialize Daft repository
    run_cmd(&["init"]);

    // 2. Create initial committed file
    let committed_file = repo_root.join("committed.txt");
    fs::write(&committed_file, "Initial base line\n").unwrap();
    run_cmd(&["add", "committed.txt"]);
    run_cmd(&["commit", "-m", "Initial commit on mainline"]);

    // Record initial commit hash
    let initial_head = fs::read_to_string(repo_root.join(".dft/HEAD")).unwrap();

    // 3. Make the workspace dirty:
    // a) Unstaged modification to committed file
    fs::write(
        &committed_file,
        "Initial base line\nUnstaged dirty modification!\n",
    )
    .unwrap();

    // b) Staged new file in index
    let staged_file = repo_root.join("staged_entry.rs");
    fs::write(&staged_file, "pub fn staged_fn() -> i32 { 101 }\n").unwrap();
    run_cmd(&["add", "staged_entry.rs"]);

    // c) Untracked dirty file
    let untracked_file = repo_root.join("untracked_scratchpad.tmp");
    fs::write(&untracked_file, "WIP untracked agent draft notes").unwrap();

    // 4. Fork dimension 'universe_beta' from 'mainline' using CLI
    let fork_output = run_cmd(&["dimension", "fork", "universe_beta", "--from", "mainline"]);
    assert!(
        fork_output.contains("Forked dimension 'universe_beta' from 'mainline'"),
        "Unexpected fork output: {}",
        fork_output
    );

    let child_ws = repo_root.join(".dft/dimensions/universe_beta/workspace");
    let child_index = repo_root.join(".dft/dimensions/universe_beta/index");
    let child_head = repo_root.join(".dft/dimensions/universe_beta/HEAD");

    assert!(child_ws.exists(), "Child workspace must exist");
    assert!(child_index.exists(), "Child index must exist");
    assert!(child_head.exists(), "Child HEAD must exist");

    // 5. Verify HEAD is identical: zero intermediate/dummy commits created
    let child_head_content = fs::read_to_string(&child_head).unwrap();
    assert_eq!(
        child_head_content.trim(),
        initial_head.trim(),
        "Live fork must not alter HEAD or introduce dummy commits"
    );

    // 6. Verify dirty changes and untracked files are faithfully preserved in child
    assert_eq!(
        fs::read_to_string(child_ws.join("committed.txt")).unwrap(),
        "Initial base line\nUnstaged dirty modification!\n",
        "Unstaged modification not preserved in child workspace"
    );
    assert_eq!(
        fs::read_to_string(child_ws.join("staged_entry.rs")).unwrap(),
        "pub fn staged_fn() -> i32 { 101 }\n",
        "Staged entry file not present in child workspace"
    );
    assert_eq!(
        fs::read_to_string(child_ws.join("untracked_scratchpad.tmp")).unwrap(),
        "WIP untracked agent draft notes",
        "Untracked file not preserved in child workspace"
    );

    // 7. Verify index entries in child match source index
    let src_idx = Index::read_from(&repo_root.join(".dft/index")).expect("read src index");
    let child_idx = Index::read_from(&child_index).expect("read child index");
    assert!(
        child_idx
            .find_entry("staged_entry.rs", Stage::Normal)
            .is_some(),
        "staged_entry.rs must be present in child index"
    );
    assert_eq!(
        child_idx.entries().len(),
        src_idx.entries().len(),
        "Child index entry count must match source index"
    );

    // 8. Verify Source is completely untouched
    assert_eq!(
        fs::read_to_string(&committed_file).unwrap(),
        "Initial base line\nUnstaged dirty modification!\n"
    );
    assert_eq!(
        fs::read_to_string(&staged_file).unwrap(),
        "pub fn staged_fn() -> i32 { 101 }\n"
    );
    assert_eq!(
        fs::read_to_string(&untracked_file).unwrap(),
        "WIP untracked agent draft notes"
    );

    // 9. Verify Two-Way Isolation: mutating child never alters source
    fs::write(
        child_ws.join("committed.txt"),
        "Diverged entirely in child workspace\n",
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(&committed_file).unwrap(),
        "Initial base line\nUnstaged dirty modification!\n",
        "Mutating child workspace corrupted source file!"
    );

    // Mutating source never alters child
    fs::write(&untracked_file, "Source untracked altered independently").unwrap();
    assert_eq!(
        fs::read_to_string(child_ws.join("untracked_scratchpad.tmp")).unwrap(),
        "WIP untracked agent draft notes",
        "Mutating source corrupted child untracked file!"
    );
}

// ============================================================================
// 3. Snapshot Lifecycle Stress (Feature 42)
// ============================================================================

#[test]
fn stress_test_snapshot_lifecycle_cli_and_lib() {
    let dft_bin = find_dft_bin();
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo_snapshot_stress");
    fs::create_dir_all(&repo_root).unwrap();

    let run_cmd = |args: &[&str]| {
        let out = Command::new(&dft_bin)
            .current_dir(&repo_root)
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("Failed to execute dft {:?}: {}", args, e));
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(
            out.status.success(),
            "Command 'dft {:?}' failed!\nSTDOUT: {}\nSTDERR: {}",
            args,
            stdout,
            stderr
        );
        stdout
    };

    // 1. Initialize repository
    run_cmd(&["init"]);

    // 2. Create baseline files in mainline
    let f1 = repo_root.join("config.toml");
    let f2 = repo_root.join("src/lib.rs");
    fs::create_dir_all(repo_root.join("src")).unwrap();

    fs::write(&f1, "[package]\nname = \"quantum_sim\"\n").unwrap();
    fs::write(&f2, "pub fn simulate() -> bool { true }\n").unwrap();

    run_cmd(&["add", "config.toml", "src/lib.rs"]);
    run_cmd(&["commit", "-m", "Initial commit for snapshot test"]);

    // 3. Take Snapshot 'baseline-state' via CLI
    let create_out = run_cmd(&[
        "snapshot",
        "create",
        "baseline-state",
        "-m",
        "Baseline before experimentation",
    ]);
    assert!(
        create_out.contains("Created snapshot 'baseline-state'"),
        "Unexpected create output: {}",
        create_out
    );

    // 4. List Snapshots via CLI
    let list_out = run_cmd(&["snapshot", "list"]);
    assert!(
        list_out.contains("baseline-state"),
        "Snapshot list must contain 'baseline-state'"
    );

    // 5. Dirty the workspace: modify files, add uncommitted files
    fs::write(&f1, "[package]\nname = \"corrupted_dirty_sim\"\n").unwrap();
    fs::write(
        repo_root.join("dirty_experiment.rs"),
        "// dirty uncommitted code",
    )
    .unwrap();
    run_cmd(&["add", "config.toml"]); // stage change

    // 6. Stress Test Restore Dirty Guard:
    // Attempt restore WITHOUT --force -> MUST BE REJECTED
    let restore_attempt = Command::new(&dft_bin)
        .current_dir(&repo_root)
        .args(&["snapshot", "restore", "baseline-state"])
        .output()
        .expect("run restore without force");

    assert!(
        !restore_attempt.status.success(),
        "Snapshot restore on dirty workspace without --force MUST FAIL!"
    );
    let stderr = String::from_utf8_lossy(&restore_attempt.stderr);
    let stdout = String::from_utf8_lossy(&restore_attempt.stdout);
    assert!(
        stderr.contains("uncommitted")
            || stdout.contains("uncommitted")
            || stderr.contains("force"),
        "Expected error message indicating uncommitted changes / use --force. Got: {}\n{}",
        stderr,
        stdout
    );

    // Verify workspace was NOT modified by failed restore
    assert_eq!(
        fs::read_to_string(&f1).unwrap(),
        "[package]\nname = \"corrupted_dirty_sim\"\n"
    );

    // 7. Force Restore:
    // Attempt restore WITH --force -> MUST SUCCEED
    let force_restore_out = run_cmd(&["snapshot", "restore", "baseline-state", "--force"]);
    assert!(
        force_restore_out.contains("Restored dimension 'mainline' to snapshot 'baseline-state'"),
        "Unexpected force restore output: {}",
        force_restore_out
    );

    // Verify workspace was restored to baseline
    assert_eq!(
        fs::read_to_string(&f1).unwrap(),
        "[package]\nname = \"quantum_sim\"\n",
        "config.toml was not restored to baseline state"
    );
    assert_eq!(
        fs::read_to_string(&f2).unwrap(),
        "pub fn simulate() -> bool { true }\n",
        "src/lib.rs was not restored to baseline state"
    );

    // 8. Delete Snapshot:
    let delete_out = run_cmd(&["snapshot", "delete", "baseline-state"]);
    assert!(
        delete_out.contains("Deleted snapshot 'baseline-state'"),
        "Unexpected delete output: {}",
        delete_out
    );

    // Verify snapshot is gone from list
    let list_after = run_cmd(&["snapshot", "list"]);
    assert!(
        !list_after.contains("baseline-state"),
        "Deleted snapshot must not appear in list"
    );

    // Restoring deleted snapshot must fail
    let restore_deleted = Command::new(&dft_bin)
        .current_dir(&repo_root)
        .args(&["snapshot", "restore", "baseline-state", "--force"])
        .output()
        .expect("restore deleted");
    assert!(
        !restore_deleted.status.success(),
        "Restoring deleted snapshot must fail"
    );
}

// ============================================================================
// 4. Advanced Edge Cases & Boundary Stress Tests
// ============================================================================

#[test]
fn stress_test_cow_edge_cases_empty_binary_unicode_files() {
    let tmp = tempdir().unwrap();
    let src_dir = tmp.path().join("src_edge");
    let dst_dir = tmp.path().join("dst_edge");
    fs::create_dir_all(&src_dir).unwrap();

    // 1. Zero-byte file
    fs::write(src_dir.join("empty.bin"), b"").unwrap();

    // 2. 1MB pseudo-random binary file
    let mut large_bin = vec![0u8; 1024 * 1024];
    for (i, b) in large_bin.iter_mut().enumerate() {
        *b = ((i * 31) ^ (i >> 8)) as u8;
    }
    fs::write(src_dir.join("large.blob"), &large_bin).unwrap();

    // 3. Deeply nested file (10 levels)
    let deep_dir = src_dir.join("a/b/c/d/e/f/g/h/i/j");
    fs::create_dir_all(&deep_dir).unwrap();
    fs::write(deep_dir.join("deep.txt"), "deep contents").unwrap();

    // 4. Unicode filename
    let unicode_file = src_dir.join("файл_量子_🚀.md");
    fs::write(&unicode_file, "Multilingual quantum timeline!").unwrap();

    // Clone using detected strategy
    let strat = detect_best_strategy(&src_dir);
    CowEngine::clone_workspace(&src_dir, &dst_dir, strat, None).expect("clone edge cases");

    // Verify all files match exactly
    assert_eq!(fs::read(dst_dir.join("empty.bin")).unwrap(), b"");
    assert_eq!(fs::read(dst_dir.join("large.blob")).unwrap(), large_bin);
    assert_eq!(
        fs::read_to_string(dst_dir.join("a/b/c/d/e/f/g/h/i/j/deep.txt")).unwrap(),
        "deep contents"
    );
    assert_eq!(
        fs::read_to_string(dst_dir.join("файл_量子_🚀.md")).unwrap(),
        "Multilingual quantum timeline!"
    );
}

#[test]
fn stress_test_live_fork_inactive_dimension_and_lineage_chain() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo_lineage");
    fs::create_dir_all(&repo_root).unwrap();

    init(&repo_root, &InitOptions::default()).unwrap();
    let repo = std::sync::Arc::new(Repository::open(&repo_root.join(".dft")).unwrap());
    let manager = DimensionManager::new(repo.clone());
    manager.init().unwrap();

    // Create dim1 and populate it with uncommitted files
    manager.create_dimension("dim1", None, None).unwrap();
    let dim1_ws = manager.dimensions_dir().join("dim1/workspace");
    fs::write(dim1_ws.join("stage1.txt"), "stage 1 uncommitted work").unwrap();

    // Fork dim2 from inactive dim1
    let meta2 = manager.fork_dimension("dim2", "dim1").expect("fork dim2");
    assert_eq!(meta2.parent.as_deref(), Some("dim1"));
    let dim2_ws = manager.dimensions_dir().join("dim2/workspace");
    assert_eq!(
        fs::read_to_string(dim2_ws.join("stage1.txt")).unwrap(),
        "stage 1 uncommitted work"
    );

    // Modify dim2 and fork dim3 from inactive dim2
    fs::write(dim2_ws.join("stage2.txt"), "stage 2 addition").unwrap();
    let meta3 = manager.fork_dimension("dim3", "dim2").expect("fork dim3");
    assert_eq!(meta3.parent.as_deref(), Some("dim2"));
    let dim3_ws = manager.dimensions_dir().join("dim3/workspace");

    assert_eq!(
        fs::read_to_string(dim3_ws.join("stage1.txt")).unwrap(),
        "stage 1 uncommitted work"
    );
    assert_eq!(
        fs::read_to_string(dim3_ws.join("stage2.txt")).unwrap(),
        "stage 2 addition"
    );

    // Verify isolation back up the chain: dim1 must NOT have stage2.txt
    assert!(
        !dim1_ws.join("stage2.txt").exists(),
        "dim1 was polluted by downstream fork work!"
    );
}

#[test]
fn stress_test_concurrent_snapshot_operations() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo_concurrent_snap");
    fs::create_dir_all(&repo_root).unwrap();

    init(&repo_root, &InitOptions::default()).unwrap();
    let repo = std::sync::Arc::new(Repository::open(&repo_root.join(".dft")).unwrap());
    let manager = DimensionManager::new(repo.clone());
    manager.init().unwrap();

    // Create 4 dimensions
    for i in 0..4 {
        let name = format!("agent_dim_{}", i);
        manager.create_dimension(&name, None, None).unwrap();
        let ws = manager.dimensions_dir().join(format!("{}/workspace", name));
        fs::write(ws.join("task.txt"), format!("agent {} payload", i)).unwrap();
    }

    // Take snapshots across all 4 dimensions concurrently using rayon
    use rayon::prelude::*;
    let snapshot_results: Vec<Result<daft_dimension::snapshot::Snapshot, _>> = (0..4)
        .into_par_iter()
        .map(|i| {
            let dim_name = format!("agent_dim_{}", i);
            let snap_name = format!("snap_chk_{}", i);
            SnapshotManager::create_snapshot(&repo, &dim_name, &snap_name, Some("Concurrent snap"))
        })
        .collect();

    for res in snapshot_results {
        assert!(
            res.is_ok(),
            "Concurrent snapshot creation failed: {:?}",
            res
        );
    }

    // Verify snapshot filtering by dimension
    for i in 0..4 {
        let dim_name = format!("agent_dim_{}", i);
        let list = SnapshotManager::list_snapshots(&repo, Some(&dim_name)).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, format!("snap_chk_{}", i));
    }

    // Verify global snapshot list contains all 4
    let all_snaps = SnapshotManager::list_snapshots(&repo, None).unwrap();
    assert_eq!(all_snaps.len(), 4);
}
