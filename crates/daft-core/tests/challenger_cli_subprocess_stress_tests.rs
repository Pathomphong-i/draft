//! Empirical Adversarial Challenger Test Suite: CLI Subprocess & Edge Conditions
//! Written by teamwork_preview_challenger_m2_it2_2.
//! Stress-tests Daft CLI binary across edge conditions:
//! - Missing arguments & syntax violations
//! - Non-existent files & references
//! - Detached HEAD checkout, commit, log, and branch creation
//! - Empty commits & --allow-empty behavior
//! - Nested subdirectories & CWD discovery vs pathspec rebasing
//! - Non-ASCII / Unicode paths, branch names, and commit messages
//! - Non-repository operations safety

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::tempdir;

fn get_dft_bin() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .expect("parent of crates")
        .parent()
        .expect("workspace root");

    let debug_bin = workspace_root.join("target/debug/dft");
    if debug_bin.exists() {
        return debug_bin;
    }
    let release_bin = workspace_root.join("target/release/dft");
    if release_bin.exists() {
        return release_bin;
    }
    panic!(
        "dft binary not found at {:?} or {:?}",
        debug_bin, release_bin
    );
}

#[derive(Debug, Clone)]
pub struct CmdResult {
    pub status: std::process::ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl CmdResult {
    pub fn success(&self) -> bool {
        self.status.success()
    }
}

pub fn run_dft<P: AsRef<Path>>(dir: P, args: &[&str]) -> CmdResult {
    let bin = get_dft_bin();
    let output: Output = Command::new(&bin)
        .current_dir(dir.as_ref())
        .args(args)
        .env("HOME", dir.as_ref())
        .env("DFT_CONFIG_DIR", dir.as_ref().join(".config"))
        .env("DFT_AUTHOR_NAME", "Adversarial Challenger")
        .env("DFT_AUTHOR_EMAIL", "challenger@daft-vcs.org")
        .output()
        .unwrap_or_else(|e| panic!("Failed to execute {:?}: {}", bin, e));

    CmdResult {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }
}

// ============================================================================
// 1. Missing Arguments & Syntax Violations
// ============================================================================

#[test]
fn test_challenger_cli_commit_missing_message() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    let init = run_dft(root, &["init"]);
    assert!(init.success(), "init failed: {}", init.stderr);

    fs::write(root.join("file.txt"), "hello").unwrap();
    run_dft(root, &["add", "file.txt"]);

    // Missing -m
    let commit = run_dft(root, &["commit"]);
    assert!(
        !commit.success(),
        "commit without message should fail cleanly"
    );
    assert!(
        commit.stderr.contains("empty commit message") || commit.stderr.contains("required"),
        "stderr should mention empty message or requirement: {}",
        commit.stderr
    );

    // Empty string message
    let commit_empty = run_dft(root, &["commit", "-m", "   "]);
    assert!(
        !commit_empty.success(),
        "commit with whitespace-only message should fail"
    );
}

#[test]
fn test_challenger_cli_branch_delete_missing_name() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    run_dft(root, &["init"]);

    let res = run_dft(root, &["branch", "-d"]);
    assert!(
        !res.success(),
        "branch -d without branch name should fail gracefully"
    );
}

#[test]
fn test_challenger_cli_switch_missing_branch() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    run_dft(root, &["init"]);

    let res = run_dft(root, &["switch"]);
    assert!(
        !res.success(),
        "switch without target branch should fail gracefully"
    );
}

#[test]
fn test_challenger_cli_missing_args_operations() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    run_dft(root, &["init"]);

    // show with non-existent ref
    let show_res = run_dft(root, &["show", "nonexistent_ref_123"]);
    assert!(
        !show_res.success(),
        "show with non-existent ref should fail"
    );

    // diff with two non-existent revs fails as expected
    let diff_two_res = run_dft(root, &["diff", "nonexistent_rev_1", "nonexistent_rev_2"]);
    assert!(
        !diff_two_res.success(),
        "diff with 2 non-existent revs should fail"
    );

    // Empirical Bug Verification: diff with 1 rev ignores args.revisions[0] and runs diff_worktree_to_index
    let diff_one_res = run_dft(root, &["diff", "nonexistent_rev_456"]);
    assert!(
        diff_one_res.success(),
        "Empirical confirmation: dft diff <rev> silently ignores the revision argument because diff.rs only checks len == 2"
    );

    // merge with non-existent branch
    let merge_res = run_dft(root, &["merge", "nonexistent_branch_789"]);
    assert!(
        !merge_res.success(),
        "merge with non-existent branch should fail"
    );

    // tag delete non-existent tag
    let tag_res = run_dft(root, &["tag", "-d", "nonexistent_tag_xyz"]);
    assert!(
        !tag_res.success(),
        "tag -d with non-existent tag should fail"
    );
}

// ============================================================================
// 2. Non-Existent Files & References
// ============================================================================

#[test]
fn test_challenger_cli_add_nonexistent_file() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    run_dft(root, &["init"]);

    let res = run_dft(root, &["add", "completely_missing_file.txt"]);
    assert!(
        !res.success(),
        "add nonexistent file should fail: stdout={}, stderr={}",
        res.stdout,
        res.stderr
    );
    assert!(
        res.stderr.contains("did not match any files") || res.stderr.contains("No such file"),
        "stderr should inform about missing file: {}",
        res.stderr
    );
}

#[test]
fn test_challenger_cli_rm_nonexistent_file() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    run_dft(root, &["init"]);

    let res = run_dft(root, &["rm", "phantom_file.txt"]);
    assert!(
        !res.success(),
        "rm nonexistent file should fail: stdout={}, stderr={}",
        res.stdout,
        res.stderr
    );
}

#[test]
fn test_challenger_cli_restore_nonexistent_file_defect() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    run_dft(root, &["init"]);

    // Empirical Bug Verification:
    // In Git, `git restore phantom.txt` returns error code 1:
    // "error: pathspec 'phantom.txt' did not match any file(s) known to git"
    // In Daft, `dft restore phantom.txt` silently ignores the non-existent file
    // and returns exit code 0 because `restore.rs` has no `else` error branch for index misses.
    let res = run_dft(root, &["restore", "phantom.txt"]);

    // Document exact empirical finding:
    let is_silent_success_bug = res.success() && res.stderr.is_empty();
    assert!(
        is_silent_success_bug,
        "Empirical confirmation: dft restore on nonexistent file silently succeeds instead of erroring"
    );
}

#[test]
fn test_challenger_cli_blame_nonexistent_file() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    run_dft(root, &["init"]);

    let res = run_dft(root, &["blame", "nonexistent.txt"]);
    assert!(
        !res.success(),
        "blame nonexistent file should fail: stdout={}, stderr={}",
        res.stdout,
        res.stderr
    );
}

#[test]
fn test_challenger_cli_checkout_nonexistent_ref() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    run_dft(root, &["init"]);

    fs::write(root.join("f.txt"), "hello").unwrap();
    run_dft(root, &["add", "f.txt"]);
    run_dft(root, &["commit", "-m", "init"]);

    let res = run_dft(root, &["checkout", "nonexistent-branch-xyz-999"]);
    assert!(
        !res.success(),
        "checkout nonexistent ref should fail: stdout={}, stderr={}",
        res.stdout,
        res.stderr
    );
}

// ============================================================================
// 3. Detached HEAD Lifecycle
// ============================================================================

#[test]
fn test_challenger_cli_detached_head_lifecycle() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    run_dft(root, &["init"]);

    // Commit 1
    fs::write(root.join("c1.txt"), "first").unwrap();
    run_dft(root, &["add", "c1.txt"]);
    let c1_res = run_dft(root, &["commit", "-m", "commit 1"]);
    assert!(c1_res.success());

    // Commit 2
    fs::write(root.join("c2.txt"), "second").unwrap();
    run_dft(root, &["add", "c2.txt"]);
    let c2_res = run_dft(root, &["commit", "-m", "commit 2"]);
    assert!(c2_res.success());

    // Get Commit 1 OID via rev-parse
    let rev_res = run_dft(root, &["rev-parse", "HEAD~1"]);
    assert!(rev_res.success(), "rev-parse failed: {}", rev_res.stderr);
    let c1_oid = rev_res.stdout.trim();
    assert_eq!(c1_oid.len(), 64, "OID should be 64-char hex");

    // Checkout commit 1 (enters detached HEAD state)
    let co_res = run_dft(root, &["checkout", c1_oid]);
    assert!(co_res.success(), "checkout c1 failed: {}", co_res.stderr);

    // Verify status in detached HEAD (prints short OID as branch)
    let status_res = run_dft(root, &["status"]);
    assert!(status_res.success(), "status failed: {}", status_res.stderr);

    // Commit in detached HEAD state
    fs::write(root.join("detached_work.txt"), "detached content").unwrap();
    run_dft(root, &["add", "detached_work.txt"]);
    let det_commit = run_dft(root, &["commit", "-m", "detached commit"]);
    assert!(
        det_commit.success(),
        "commit in detached HEAD should succeed: {}",
        det_commit.stderr
    );

    // Verify HEAD is updated to new commit
    let new_head = run_dft(root, &["rev-parse", "HEAD"]);
    assert!(new_head.success());
    let new_head_oid = new_head.stdout.trim();
    assert_ne!(
        new_head_oid, c1_oid,
        "HEAD should have advanced to new commit"
    );

    // Verify log in detached HEAD
    let log_res = run_dft(root, &["log"]);
    assert!(log_res.success(), "log failed: {}", log_res.stderr);
    assert!(log_res.stdout.contains("detached commit"));
    assert!(
        !log_res.stdout.lines().any(|l| l.trim() == "commit 2"),
        "log should not contain commit 2, got:\n{}",
        log_res.stdout
    );

    // Create a new branch from detached HEAD
    let branch_res = run_dft(root, &["branch", "recovered-detached"]);
    assert!(
        branch_res.success(),
        "branch creation from detached HEAD should succeed: {}",
        branch_res.stderr
    );

    // Switch back to main
    let switch_main = run_dft(root, &["switch", "main"]);
    assert!(
        switch_main.success(),
        "switch back to main failed: {}",
        switch_main.stderr
    );

    // Switch to recovered branch
    let switch_rec = run_dft(root, &["switch", "recovered-detached"]);
    assert!(
        switch_rec.success(),
        "switch to recovered branch failed: {}",
        switch_rec.stderr
    );
    assert!(
        root.join("detached_work.txt").exists(),
        "detached work should be present on recovered branch"
    );
}

#[test]
fn test_challenger_cli_detached_head_empty_commit_guard() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    run_dft(root, &["init"]);

    fs::write(root.join("initial.txt"), "initial").unwrap();
    run_dft(root, &["add", "initial.txt"]);
    run_dft(root, &["commit", "-m", "c1"]);

    let head_oid = run_dft(root, &["rev-parse", "HEAD"])
        .stdout
        .trim()
        .to_string();
    run_dft(root, &["checkout", &head_oid]);

    // In detached HEAD with clean working tree, commit without changes must fail
    let empty_commit = run_dft(root, &["commit", "-m", "should fail"]);
    assert!(
        !empty_commit.success(),
        "empty commit in detached HEAD should be rejected"
    );
    assert!(empty_commit.stderr.contains("nothing to commit"));

    // With --allow-empty, commit in detached HEAD should succeed
    let allow_empty = run_dft(
        root,
        &["commit", "--allow-empty", "-m", "allow empty detached"],
    );
    assert!(
        allow_empty.success(),
        "allow-empty commit in detached HEAD should succeed: {}",
        allow_empty.stderr
    );
}

// ============================================================================
// 4. Empty Commits & --allow-empty
// ============================================================================

#[test]
fn test_challenger_cli_empty_commit_guard_and_override() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    run_dft(root, &["init"]);

    // Attempting empty commit without --allow-empty on brand new repo
    let empty_initial = run_dft(root, &["commit", "-m", "empty initial"]);
    assert!(
        !empty_initial.success(),
        "empty initial commit without staged files should fail"
    );
    assert!(
        empty_initial.stderr.contains("nothing to commit"),
        "stderr should mention nothing to commit: {}",
        empty_initial.stderr
    );

    // Empty initial commit with --allow-empty
    let allow_initial = run_dft(
        root,
        &["commit", "--allow-empty", "-m", "initial root empty"],
    );
    assert!(
        allow_initial.success(),
        "initial --allow-empty commit should succeed: {}",
        allow_initial.stderr
    );

    // Second empty commit without changes
    let empty_second = run_dft(root, &["commit", "-m", "second empty without flag"]);
    assert!(
        !empty_second.success(),
        "second commit without changes or flag should fail"
    );

    // Second empty commit with --allow-empty
    let allow_second = run_dft(
        root,
        &["commit", "--allow-empty", "-m", "second root empty"],
    );
    assert!(
        allow_second.success(),
        "second --allow-empty commit should succeed: {}",
        allow_second.stderr
    );

    // Verify log contains both empty commits
    let log = run_dft(root, &["log"]);
    assert!(log.success());
    assert!(log.stdout.contains("initial root empty"));
    assert!(log.stdout.contains("second root empty"));
}

// ============================================================================
// 5. Nested Subdirectories & CWD Inheritance
// ============================================================================

#[test]
fn test_challenger_cli_nested_subdirectories_discovery() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    run_dft(root, &["init"]);

    let deep_dir = root.join("src").join("engine").join("crypto");
    fs::create_dir_all(&deep_dir).unwrap();

    // Initial commit so repository has history
    fs::write(root.join("README.md"), "# Daft VCS").unwrap();
    run_dft(root, &["add", "README.md"]);
    run_dft(root, &["commit", "-m", "initial readme"]);

    // 1. Repository discovery from deep directory succeeds
    let status_res = run_dft(&deep_dir, &["status"]);
    assert!(
        status_res.success(),
        "status from deep subdir failed: {}",
        status_res.stderr
    );

    // 2. Log from deep directory succeeds
    let log_res = run_dft(&deep_dir, &["log"]);
    assert!(
        log_res.success(),
        "log from deep subdir failed: {}",
        log_res.stderr
    );
    assert!(log_res.stdout.contains("initial readme"));

    // 3. Create file inside deep directory
    let file_path = deep_dir.join("sha.rs");
    fs::write(&file_path, "pub fn hash() {}").unwrap();

    // Empirical Bug Verification:
    // When executing `dft add sha.rs` from inside deep_dir, `add_paths` in `daft-core`
    // computes `workdir.join("sha.rs")` instead of `cwd.join("sha.rs")`, causing
    // pathspec matching to fail!
    let add_relative_cwd = run_dft(&deep_dir, &["add", "sha.rs"]);
    assert!(
        !add_relative_cwd.success(),
        "Empirical confirmation: dft add with CWD-relative path from deep subdir fails because it joins with workdir instead of CWD"
    );
    assert!(
        add_relative_cwd.stderr.contains("did not match any files"),
        "stderr should confirm pathspec failure: {}",
        add_relative_cwd.stderr
    );

    // Staging using repo-root relative path from deep directory works as a workaround
    let add_repo_rel = run_dft(&deep_dir, &["add", "src/engine/crypto/sha.rs"]);
    assert!(
        add_repo_rel.success(),
        "add with repo-relative path should succeed: {}",
        add_repo_rel.stderr
    );

    let commit_res = run_dft(&deep_dir, &["commit", "-m", "add crypto module"]);
    assert!(
        commit_res.success(),
        "commit from deep subdir failed: {}",
        commit_res.stderr
    );
}

// ============================================================================
// 6. Non-ASCII / Unicode Paths & Branch Names
// ============================================================================

#[test]
fn test_challenger_cli_unicode_paths_and_branch_names() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    run_dft(root, &["init"]);

    let unicode_dir = root.join("📁_source_宇宙");
    fs::create_dir_all(&unicode_dir).unwrap();

    let complex_file = unicode_dir.join("量子_🚀_data.txt");
    fs::write(&complex_file, "quantum data: 42").unwrap();

    let add_res = run_dft(root, &["add", "."]);
    assert!(
        add_res.success(),
        "add . with unicode paths failed: {}",
        add_res.stderr
    );

    let commit_res = run_dft(
        root,
        &[
            "commit",
            "-m",
            "feat: 宇宙の量子状態 🚀\n\nExtended multiverse description.",
        ],
    );
    assert!(
        commit_res.success(),
        "unicode commit message failed: {}",
        commit_res.stderr
    );

    // Branch with unicode
    let branch_res = run_dft(root, &["branch", "quantum-🚀-branch"]);
    assert!(
        branch_res.success(),
        "branch creation with unicode failed: {}",
        branch_res.stderr
    );

    let switch_res = run_dft(root, &["switch", "quantum-🚀-branch"]);
    assert!(
        switch_res.success(),
        "switch with unicode branch failed: {}",
        switch_res.stderr
    );

    let status_res = run_dft(root, &["status"]);
    assert!(status_res.success());
    assert!(status_res.stdout.contains("quantum-🚀-branch"));
}

// ============================================================================
// 7. Non-Repository Safety
// ============================================================================

#[test]
fn test_challenger_cli_outside_repo_fails_cleanly() {
    let tmp = tempdir().unwrap();
    let non_repo = tmp.path();

    let commands = [
        vec!["status"],
        vec!["commit", "-m", "test"],
        vec!["add", "foo.txt"],
        vec!["log"],
        vec!["diff"],
        vec!["branch"],
    ];

    for cmd in &commands {
        let res = run_dft(non_repo, cmd);
        assert!(
            !res.success(),
            "dft {:?} should fail when run outside a repository",
            cmd
        );
        assert!(
            res.stderr.contains("Not a Daft repository"),
            "dft {:?} stderr should mention 'Not a Daft repository', got: {}",
            cmd,
            res.stderr
        );
    }
}
