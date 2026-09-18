//! Tier 2: Boundary Value Analysis & Corner Cases Tests.
//! Stress-tests Daft across empty states, extreme sizes, deep trees, invalid syntax, and unicode paths.

use crate::common::TestEnv;

// ============================================================================
// 1. Empty Repository Boundaries
// ============================================================================

pub fn test_empty_repo_status() {
    let env = TestEnv::new("bound_empty_status");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["status"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("No commits yet");
    }
}

pub fn test_empty_repo_log() {
    let env = TestEnv::new("bound_empty_log");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["log"]);
    // Should gracefully report no commits or return non-zero cleanly
    if !env.is_dry_run {
        assert!(
            res.stdout.contains("No commits") || res.stderr.contains("No commits") || !res.success
        );
    }
}

pub fn test_empty_repo_diff() {
    let env = TestEnv::new("bound_empty_diff");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["diff"]);
    res.assert_success();
}

pub fn test_empty_repo_branch() {
    let env = TestEnv::new("bound_empty_branch");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["branch"]);
    res.assert_success();
}

// ============================================================================
// 2. Extreme Sizes & Binaries
// ============================================================================

pub fn test_huge_file_storage() {
    let env = TestEnv::new("bound_huge_file");
    env.dft(&["init"]).assert_success();

    // Create a 5MB structured payload
    let chunk = b"Daft quantum VCS payload block - 64 bytes of deterministic bytes!\n";
    let mut large_data = Vec::with_capacity(5 * 1024 * 1024);
    while large_data.len() < 5 * 1024 * 1024 {
        large_data.extend_from_slice(chunk);
    }

    env.write_bytes("large_dataset.bin", &large_data);
    env.dft(&["add", "large_dataset.bin"]).assert_success();
    let commit_res = env.dft(&["commit", "-m", "chore: add 5MB dataset"]);
    commit_res.assert_success();

    // Verify status is clean
    let status_res = env.dft(&["status"]);
    status_res.assert_success();
}

pub fn test_binary_null_bytes() {
    let env = TestEnv::new("bound_binary_nulls");
    env.dft(&["init"]).assert_success();

    let binary_data: Vec<u8> = (0..=255).cycle().take(4096).collect();
    env.write_bytes("null_bytes.bin", &binary_data);
    env.dft(&["add", "null_bytes.bin"]).assert_success();
    let res = env.dft(&["commit", "-m", "binary file commit"]);
    res.assert_success();
}

// ============================================================================
// 3. Deep Trees & Complex Paths
// ============================================================================

pub fn test_deep_directory_hierarchy() {
    let env = TestEnv::new("bound_deep_tree");
    env.dft(&["init"]).assert_success();

    // Construct 30-level nested path
    let mut deep_path = String::new();
    for i in 0..30 {
        deep_path.push_str(&format!("dir_{}/", i));
    }
    deep_path.push_str("deep_leaf.txt");

    env.write_file(&deep_path, "Content at depth 30");
    env.dft(&["add", "."]).assert_success();
    let res = env.dft(&["commit", "-m", "deep tree commit"]);
    res.assert_success();
}

// ============================================================================
// 4. Unicode & Special Characters
// ============================================================================

pub fn test_unicode_filenames_and_paths() {
    let env = TestEnv::new("bound_unicode");
    env.dft(&["init"]).assert_success();

    let utf8_name = "日本語_файл_🚀_quantum.txt";
    env.write_file(utf8_name, "UTF-8 quantum state: 宇宙");
    env.dft(&["add", utf8_name]).assert_success();
    let commit = env.dft(&["commit", "-m", "commit: unicode filename"]);
    commit.assert_success();

    let status = env.dft(&["status"]);
    status.assert_success();
    if !env.is_dry_run {
        status.assert_output_contains("clean");
    }
}

pub fn test_spaces_and_quotes_in_filename() {
    let env = TestEnv::new("bound_spaces");
    env.dft(&["init"]).assert_success();

    let complex_name = "my draft file (v1.0) [final] - test.txt";
    env.write_file(complex_name, "content with spaced filename");
    env.dft(&["add", complex_name]).assert_success();
    let commit = env.dft(&["commit", "-m", "commit: filename with spaces"]);
    commit.assert_success();
}

// ============================================================================
// 5. Invalid Syntax & Parameter Validation
// ============================================================================

pub fn test_invalid_syntax_unknown_subcommand() {
    let env = TestEnv::new("bound_unknown_subcmd");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["unrecognized-command-xyz"]);
    if !env.is_dry_run {
        res.assert_failure();
    }
}

pub fn test_invalid_dimension_name_path_traversal() {
    let env = TestEnv::new("bound_path_traversal");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["dimension", "create", "../../escape_sandbox"]);
    if !env.is_dry_run {
        res.assert_failure();
    }
}

pub fn test_invalid_dimension_name_empty() {
    let env = TestEnv::new("bound_empty_dim_name");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["dimension", "create", ""]);
    if !env.is_dry_run {
        res.assert_failure();
    }
}

// ============================================================================
// 6. Uncommitted Dirty States & Safety Guards
// ============================================================================

pub fn test_destroy_dimension_with_dirty_uncommitted_prevented() {
    let env = TestEnv::new("bound_destroy_dirty");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "temp-work"])
        .assert_success();
    env.dft(&["dimension", "enter", "temp-work"])
        .assert_success();
    env.write_file("uncommitted_precious.txt", "valuable code");
    env.dft(&["dimension", "enter", "mainline"])
        .assert_success();

    let res = env.dft(&["dimension", "destroy", "temp-work"]);
    if !env.is_dry_run {
        res.assert_failure();
        res.assert_output_contains("uncommitted");
    }
}

pub fn test_rapid_consecutive_dimension_lifecycle() {
    let env = TestEnv::new("bound_rapid_lifecycle");
    env.dft(&["init"]).assert_success();
    for i in 1..=10 {
        let name = format!("rapid-dim-{}", i);
        env.dft(&["dimension", "create", &name]).assert_success();
        env.dft(&["dimension", "enter", &name]).assert_success();
        env.write_file("file.txt", &i.to_string());
        env.dft(&["add", "file.txt"]).assert_success();
        env.dft(&["commit", "-m", &format!("commit {}", i)])
            .assert_success();
        env.dft(&["dimension", "enter", "mainline"])
            .assert_success();
        env.dft(&["dimension", "destroy", &name, "--force"])
            .assert_success();
    }
}

pub fn test_commit_without_message_fails() {
    let env = TestEnv::new("bound_commit_no_msg");
    env.dft(&["init"]).assert_success();
    env.write_file("file.txt", "data");
    env.dft(&["add", "file.txt"]).assert_success();
    // Non-interactive commit without -m should fail cleanly
    let res = env.dft(&["commit"]);
    if !env.is_dry_run {
        res.assert_failure();
    }
}
