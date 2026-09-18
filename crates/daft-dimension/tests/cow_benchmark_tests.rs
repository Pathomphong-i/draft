//! Sub-5-Second CoW Benchmark & Isolation Integration Tests (Features 38, 40, 63).

use daft_core::init::{init, InitOptions};
use daft_core::Repository;
use daft_dimension::cow::{
    break_on_write_check_and_unlink, detect_best_strategy, CowEngine, CowStrategy,
};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use walkdir::WalkDir;

/// Generates a realistic synthetic repository with 1,000 files across 40 nested subdirectories.
fn generate_synthetic_repo(repo_workdir: &Path, file_count: usize) {
    for i in 0..file_count {
        let dir_idx = i % 40;
        let dir_path = repo_workdir.join(format!(
            "src/module_{:02}/sub_{:02}",
            dir_idx / 4,
            dir_idx % 4
        ));
        fs::create_dir_all(&dir_path).expect("Failed to create nested dir");

        let file_path = dir_path.join(format!("item_{:04}.rs", i));
        let mut file = File::create(&file_path).expect("Failed to create benchmark file");

        writeln!(file, "// Daft Benchmark File {}", i).unwrap();
        writeln!(file, "pub fn compute_{}() -> usize {{", i).unwrap();
        writeln!(file, "    let base = {};", i * 42).unwrap();
        writeln!(file, "    base + 1337").unwrap();
        writeln!(file, "}}").unwrap();
    }
}

#[test]
fn test_cow_1000_files_clone_sub_5_seconds() {
    let tmp = tempdir().expect("tempdir");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    // Initialize Daft repo
    init(&repo_root, &InitOptions::default()).expect("init repo");
    let _repo = Repository::open(&repo_root.join(".dft")).expect("open repo");

    let file_count = 1000;
    generate_synthetic_repo(&repo_root, file_count);

    let target_ws = repo_root.join(".dft/dimensions/benchmark_dim/workspace");
    fs::create_dir_all(&target_ws).unwrap();

    let strategy = detect_best_strategy(&repo_root);
    println!("Detected CoW strategy: {}", strategy);

    let start = Instant::now();
    CowEngine::clone_workspace(
        &repo_root,
        &target_ws,
        strategy,
        Some(&repo_root.join(".dft")),
    )
    .expect("clone_workspace");
    let elapsed = start.elapsed();

    println!(
        "⏱️  1,000 files CoW clone completed in: {:?} (strategy: {})",
        elapsed, strategy
    );

    // Critical Performance SLA assertion: must be under 5.0 seconds
    assert!(
        elapsed < Duration::from_secs(5),
        "CoW benchmark SLA violated: took {:?}, must be under 5.0s",
        elapsed
    );

    // Integrity assertion: count cloned regular files
    let mut cloned_count = 0;
    for entry in WalkDir::new(&target_ws).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            cloned_count += 1;
        }
    }
    assert_eq!(
        cloned_count, file_count,
        "Cloned file count must match original repository"
    );

    // Isolation assertion: mutating cloned file must NEVER modify original
    let sample_rel = "src/module_00/sub_00/item_0000.rs";
    let src_sample = repo_root.join(sample_rel);
    let dst_sample = target_ws.join(sample_rel);

    let original_content = fs::read_to_string(&src_sample).unwrap();
    let _ = break_on_write_check_and_unlink(&dst_sample);
    fs::write(&dst_sample, "MUTATED IN CLONED WORKSPACE").unwrap();

    let after_content = fs::read_to_string(&src_sample).unwrap();
    assert_eq!(
        original_content, after_content,
        "Source file was corrupted by edit in cloned workspace!"
    );
    assert_eq!(
        fs::read_to_string(&dst_sample).unwrap(),
        "MUTATED IN CLONED WORKSPACE"
    );
}

#[test]
fn test_break_on_write_semantics() {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("source.txt");
    let dst = tmp.path().join("link.txt");

    fs::write(&src, "shared content").unwrap();
    fs::hard_link(&src, &dst).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(fs::symlink_metadata(&dst).unwrap().nlink(), 2);
    }

    // Break on write check
    let broken = break_on_write_check_and_unlink(&dst).unwrap();
    #[cfg(unix)]
    assert!(broken, "Expected hard link to be unlinked");

    // Write new content to dst
    fs::write(&dst, "new isolated content").unwrap();

    assert_eq!(fs::read_to_string(&src).unwrap(), "shared content");
    assert_eq!(fs::read_to_string(&dst).unwrap(), "new isolated content");
}

#[test]
fn test_cow_strategy_fallback_copy() {
    let tmp = tempdir().unwrap();
    let src_dir = tmp.path().join("src_copy");
    let dst_dir = tmp.path().join("dst_copy");
    fs::create_dir_all(&src_dir).unwrap();

    for i in 0..50 {
        fs::write(
            src_dir.join(format!("file_{}.txt", i)),
            format!("data {}", i),
        )
        .unwrap();
    }

    CowEngine::clone_dir(&src_dir, &dst_dir, CowStrategy::Copy).expect("clone_dir copy");

    for i in 0..50 {
        assert_eq!(
            fs::read_to_string(dst_dir.join(format!("file_{}.txt", i))).unwrap(),
            format!("data {}", i)
        );
    }
}
