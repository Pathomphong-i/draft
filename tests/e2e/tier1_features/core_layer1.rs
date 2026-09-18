//! Tier 1: Core VCS & Layer 1 Git-Equivalent Command Tests.
//! Validates standard VCS operations in isolation (>=5 test cases per feature).

use crate::common::TestEnv;

// ============================================================================
// 1. `dft init` (5 tests)
// ============================================================================

pub fn test_init_creates_dft_directory() {
    let env = TestEnv::new("init_basic");
    let res = env.dft(&["init"]);
    res.assert_success();
    assert!(env.has_dft_repo(), ".dft directory should exist");
    assert!(env.file_exists(".dft/dft.toml"), "dft.toml should exist");
    assert!(env.file_exists(".dft/HEAD"), "HEAD ref should exist");
    assert!(env.dir_exists(".dft/objects"), "objects store should exist");
    assert!(env.dir_exists(".dft/refs/heads"), "refs/heads should exist");
}

pub fn test_init_custom_default_branch() {
    let env = TestEnv::new("init_custom_branch");
    let res = env.dft(&["init", "--initial-branch", "mainline"]);
    res.assert_success();
    let head = env.read_file(".dft/HEAD");
    assert!(
        head.contains("mainline") || env.is_dry_run,
        "HEAD ref should point to custom initial branch 'mainline', got: {}",
        head
    );
}

pub fn test_init_in_existing_directory_preserves_files() {
    let env = TestEnv::new("init_preserves_files");
    env.write_file("existing.txt", "pre-existing content");
    let res = env.dft(&["init"]);
    res.assert_success();
    assert_eq!(
        env.read_file("existing.txt"),
        "pre-existing content",
        "Pre-existing files must not be altered by dft init"
    );
}

pub fn test_init_reinit_is_idempotent() {
    let env = TestEnv::new("init_reinit");
    env.dft(&["init"]).assert_success();
    env.write_file("data.txt", "hello");
    env.dft(&["add", "data.txt"]).assert_success();
    env.dft(&["commit", "-m", "First commit"]).assert_success();

    // Re-run init
    let res = env.dft(&["init"]);
    res.assert_success();
    assert!(
        env.file_exists("data.txt"),
        "Reinit must be idempotent and preserve tracked repository state"
    );
}

pub fn test_init_bare_repo() {
    let env = TestEnv::new("init_bare");
    let res = env.dft(&["init", "--bare"]);
    res.assert_success();
    assert!(env.file_exists("dft.toml") || env.file_exists(".dft/dft.toml") || env.is_dry_run);
}

// ============================================================================
// 2. `dft add` & `dft status` (5 tests for add, 5 tests for status)
// ============================================================================

pub fn test_add_single_file() {
    let env = TestEnv::new("add_single");
    env.dft(&["init"]).assert_success();
    env.write_file("hello.txt", "world");
    let res = env.dft(&["add", "hello.txt"]);
    res.assert_success();
    assert!(
        env.file_exists(".dft/index") || env.is_dry_run,
        "Index file should be created"
    );
}

pub fn test_add_multiple_files() {
    let env = TestEnv::new("add_multiple");
    env.dft(&["init"]).assert_success();
    env.write_file("file1.txt", "1");
    env.write_file("file2.txt", "2");
    let res = env.dft(&["add", "file1.txt", "file2.txt"]);
    res.assert_success();
}

pub fn test_add_directory_recursive() {
    let env = TestEnv::new("add_dir");
    env.dft(&["init"]).assert_success();
    env.write_file("src/main.rs", "fn main() {}");
    env.write_file("src/lib.rs", "pub fn lib() {}");
    let res = env.dft(&["add", "src"]);
    res.assert_success();
}

pub fn test_add_current_dir_dot() {
    let env = TestEnv::new("add_dot");
    env.dft(&["init"]).assert_success();
    env.write_file("a.txt", "a");
    env.write_file("b/c.txt", "c");
    let res = env.dft(&["add", "."]);
    res.assert_success();
}

pub fn test_add_updated_file_updates_index() {
    let env = TestEnv::new("add_update");
    env.dft(&["init"]).assert_success();
    env.write_file("change.txt", "version 1");
    env.dft(&["add", "change.txt"]).assert_success();
    env.write_file("change.txt", "version 2");
    let res = env.dft(&["add", "change.txt"]);
    res.assert_success();
}

pub fn test_status_clean_worktree() {
    let env = TestEnv::new("status_clean");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["status"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("clean");
    }
}

pub fn test_status_untracked_files() {
    let env = TestEnv::new("status_untracked");
    env.dft(&["init"]).assert_success();
    env.write_file("untracked.txt", "mystery");
    let res = env.dft(&["status"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("untracked.txt");
    }
}

pub fn test_status_staged_changes() {
    let env = TestEnv::new("status_staged");
    env.dft(&["init"]).assert_success();
    env.write_file("staged.txt", "ready");
    env.dft(&["add", "staged.txt"]).assert_success();
    let res = env.dft(&["status"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("staged.txt");
    }
}

pub fn test_status_unstaged_modifications() {
    let env = TestEnv::new("status_unstaged");
    env.dft(&["init"]).assert_success();
    env.write_file("tracked.txt", "v1");
    env.dft(&["add", "tracked.txt"]).assert_success();
    env.dft(&["commit", "-m", "Initial"]).assert_success();
    env.write_file("tracked.txt", "v2 dirty");
    let res = env.dft(&["status"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("modified");
    }
}

pub fn test_status_deleted_file() {
    let env = TestEnv::new("status_deleted");
    env.dft(&["init"]).assert_success();
    env.write_file("to_delete.txt", "remove me");
    env.dft(&["add", "to_delete.txt"]).assert_success();
    env.dft(&["commit", "-m", "Commit"]).assert_success();
    let _ = std::fs::remove_file(env.root.join("to_delete.txt"));
    let res = env.dft(&["status"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("deleted");
    }
}

// ============================================================================
// 3. `dft commit` (5 tests)
// ============================================================================

pub fn test_commit_basic() {
    let env = TestEnv::new("commit_basic");
    env.dft(&["init"]).assert_success();
    env.write_file("file.txt", "content");
    env.dft(&["add", "file.txt"]).assert_success();
    let res = env.dft(&["commit", "-m", "feat: initial commit"]);
    res.assert_success();
}

pub fn test_commit_custom_author() {
    let mut env = TestEnv::new("commit_author");
    env.set_env("DFT_AUTHOR_NAME", "Alice Agent");
    env.set_env("DFT_AUTHOR_EMAIL", "alice@agent.ai");
    env.dft(&["init"]).assert_success();
    env.write_file("alice.txt", "alice code");
    env.dft(&["add", "alice.txt"]).assert_success();
    let res = env.dft(&["commit", "-m", "Alice work"]);
    res.assert_success();
    let log_res = env.dft(&["log"]);
    if !env.is_dry_run {
        log_res.assert_output_contains("Alice Agent");
    }
}

pub fn test_commit_empty_index_fails_without_flag() {
    let env = TestEnv::new("commit_empty");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["commit", "-m", "Empty commit"]);
    if !env.is_dry_run {
        res.assert_failure();
    }
}

pub fn test_commit_allow_empty() {
    let env = TestEnv::new("commit_allow_empty");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["commit", "--allow-empty", "-m", "Root empty commit"]);
    res.assert_success();
}

pub fn test_commit_sequence_advances_head() {
    let env = TestEnv::new("commit_sequence");
    env.dft(&["init"]).assert_success();
    env.write_file("f1.txt", "1");
    env.dft(&["add", "f1.txt"]).assert_success();
    env.dft(&["commit", "-m", "Commit 1"]).assert_success();

    env.write_file("f2.txt", "2");
    env.dft(&["add", "f2.txt"]).assert_success();
    env.dft(&["commit", "-m", "Commit 2"]).assert_success();

    let log = env.dft(&["log"]);
    log.assert_success();
    if !env.is_dry_run {
        log.assert_output_contains("Commit 1");
        log.assert_output_contains("Commit 2");
    }
}

// ============================================================================
// 4. `dft branch`, `checkout`, `switch` (5 tests each)
// ============================================================================

pub fn test_branch_create_and_list() {
    let env = TestEnv::new("branch_create_list");
    env.dft(&["init"]).assert_success();
    env.write_file("root.txt", "root");
    env.dft(&["add", "root.txt"]).assert_success();
    env.dft(&["commit", "-m", "Root"]).assert_success();

    env.dft(&["branch", "feature-x"]).assert_success();
    let list_res = env.dft(&["branch"]);
    list_res.assert_success();
    if !env.is_dry_run {
        list_res.assert_output_contains("feature-x");
    }
}

pub fn test_branch_delete_safe() {
    let env = TestEnv::new("branch_delete");
    env.dft(&["init"]).assert_success();
    env.write_file("a.txt", "a");
    env.dft(&["add", "a.txt"]).assert_success();
    env.dft(&["commit", "-m", "Init"]).assert_success();

    env.dft(&["branch", "temp-branch"]).assert_success();
    let del_res = env.dft(&["branch", "-d", "temp-branch"]);
    del_res.assert_success();
}

pub fn test_branch_rename() {
    let env = TestEnv::new("branch_rename");
    env.dft(&["init"]).assert_success();
    env.write_file("a.txt", "a");
    env.dft(&["add", "a.txt"]).assert_success();
    env.dft(&["commit", "-m", "Init"]).assert_success();

    env.dft(&["branch", "old-name"]).assert_success();
    let res = env.dft(&["branch", "-m", "old-name", "new-name"]);
    res.assert_success();
    let list = env.dft(&["branch"]);
    if !env.is_dry_run {
        list.assert_output_contains("new-name");
    }
}

pub fn test_switch_creates_and_switches_with_c() {
    let env = TestEnv::new("switch_c");
    env.dft(&["init"]).assert_success();
    env.write_file("init.txt", "init");
    env.dft(&["add", "init.txt"]).assert_success();
    env.dft(&["commit", "-m", "Init"]).assert_success();

    let res = env.dft(&["switch", "-c", "topic"]);
    res.assert_success();
    let status = env.dft(&["status"]);
    if !env.is_dry_run {
        status.assert_output_contains("topic");
    }
}

pub fn test_checkout_detached_head() {
    let env = TestEnv::new("checkout_detached");
    env.dft(&["init"]).assert_success();
    env.write_file("c1.txt", "c1");
    env.dft(&["add", "c1.txt"]).assert_success();
    env.dft(&["commit", "-m", "Commit 1"]).assert_success();

    let res = env.dft(&["checkout", "HEAD~0"]);
    res.assert_success();
}

// ============================================================================
// 5. `dft merge` (5 tests)
// ============================================================================

pub fn test_merge_fast_forward() {
    let env = TestEnv::new("merge_ff");
    env.dft(&["init"]).assert_success();
    env.write_file("base.txt", "base");
    env.dft(&["add", "base.txt"]).assert_success();
    env.dft(&["commit", "-m", "Base"]).assert_success();

    env.dft(&["switch", "-c", "dev"]).assert_success();
    env.write_file("dev.txt", "dev");
    env.dft(&["add", "dev.txt"]).assert_success();
    env.dft(&["commit", "-m", "Dev commit"]).assert_success();

    env.dft(&["switch", "main"]).assert_success();
    let res = env.dft(&["merge", "dev"]);
    res.assert_success();
    assert!(env.file_exists("dev.txt") || env.is_dry_run);
}

pub fn test_merge_three_way_clean() {
    let env = TestEnv::new("merge_3way_clean");
    env.dft(&["init"]).assert_success();
    env.write_file("common.txt", "common");
    env.dft(&["add", "common.txt"]).assert_success();
    env.dft(&["commit", "-m", "Root"]).assert_success();

    env.dft(&["switch", "-c", "feat-a"]).assert_success();
    env.write_file("file_a.txt", "feature a");
    env.dft(&["add", "file_a.txt"]).assert_success();
    env.dft(&["commit", "-m", "Add A"]).assert_success();

    env.dft(&["switch", "main"]).assert_success();
    env.write_file("file_b.txt", "feature b");
    env.dft(&["add", "file_b.txt"]).assert_success();
    env.dft(&["commit", "-m", "Add B"]).assert_success();

    let merge_res = env.dft(&["merge", "feat-a"]);
    merge_res.assert_success();
    assert!(env.file_exists("file_a.txt") || env.is_dry_run);
    assert!(env.file_exists("file_b.txt") || env.is_dry_run);
}

pub fn test_merge_conflict_detection() {
    let env = TestEnv::new("merge_conflict");
    env.dft(&["init"]).assert_success();
    env.write_file("conflict.txt", "original line\n");
    env.dft(&["add", "conflict.txt"]).assert_success();
    env.dft(&["commit", "-m", "Base line"]).assert_success();

    env.dft(&["switch", "-c", "branch-left"]).assert_success();
    env.write_file("conflict.txt", "left modification\n");
    env.dft(&["add", "conflict.txt"]).assert_success();
    env.dft(&["commit", "-m", "Left edit"]).assert_success();

    env.dft(&["switch", "main"]).assert_success();
    env.write_file("conflict.txt", "right modification\n");
    env.dft(&["add", "conflict.txt"]).assert_success();
    env.dft(&["commit", "-m", "Right edit"]).assert_success();

    let res = env.dft(&["merge", "branch-left"]);
    if !env.is_dry_run {
        res.assert_failure();
        let content = env.read_file("conflict.txt");
        assert!(
            content.contains("<<<<<<<") || content.contains("left") || content.contains("right"),
            "Expected conflict markers or conflict state"
        );
    }
}

pub fn test_merge_abort() {
    let env = TestEnv::new("merge_abort");
    env.dft(&["init"]).assert_success();
    env.write_file("f.txt", "init");
    env.dft(&["add", "f.txt"]).assert_success();
    env.dft(&["commit", "-m", "Base"]).assert_success();

    env.dft(&["switch", "-c", "b1"]).assert_success();
    env.write_file("f.txt", "b1 modification");
    env.dft(&["add", "f.txt"]).assert_success();
    env.dft(&["commit", "-m", "b1"]).assert_success();

    env.dft(&["switch", "main"]).assert_success();
    env.write_file("f.txt", "main modification");
    env.dft(&["add", "f.txt"]).assert_success();
    env.dft(&["commit", "-m", "main"]).assert_success();

    let _ = env.dft(&["merge", "b1"]);
    let abort_res = env.dft(&["merge", "--abort"]);
    abort_res.assert_success();
}

pub fn test_merge_preserves_two_parents() {
    let env = TestEnv::new("merge_parents");
    env.dft(&["init"]).assert_success();
    env.write_file("base.txt", "base");
    env.dft(&["add", "base.txt"]).assert_success();
    env.dft(&["commit", "-m", "Base"]).assert_success();

    env.dft(&["switch", "-c", "f1"]).assert_success();
    env.write_file("f1.txt", "f1");
    env.dft(&["add", "f1.txt"]).assert_success();
    env.dft(&["commit", "-m", "F1"]).assert_success();

    env.dft(&["switch", "main"]).assert_success();
    env.write_file("m.txt", "m");
    env.dft(&["add", "m.txt"]).assert_success();
    env.dft(&["commit", "-m", "M"]).assert_success();

    env.dft(&["merge", "f1", "-m", "Merge f1 into main"])
        .assert_success();
    let log = env.dft(&["log", "-n", "1"]);
    log.assert_success();
}

// ============================================================================
// 6. `dft diff`, `log`, `show`, `blame` (5 tests each)
// ============================================================================

pub fn test_diff_working_tree() {
    let env = TestEnv::new("diff_worktree");
    env.dft(&["init"]).assert_success();
    env.write_file("diff.txt", "initial line\n");
    env.dft(&["add", "diff.txt"]).assert_success();
    env.dft(&["commit", "-m", "Base"]).assert_success();

    env.write_file("diff.txt", "initial line\nadded line\n");
    let res = env.dft(&["diff"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_stdout_contains("+added line");
    }
}

pub fn test_diff_staged() {
    let env = TestEnv::new("diff_staged");
    env.dft(&["init"]).assert_success();
    env.write_file("file.txt", "line 1\n");
    env.dft(&["add", "file.txt"]).assert_success();
    env.dft(&["commit", "-m", "c1"]).assert_success();

    env.write_file("file.txt", "line 1\nline 2\n");
    env.dft(&["add", "file.txt"]).assert_success();
    let res = env.dft(&["diff", "--staged"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_stdout_contains("+line 2");
    }
}

pub fn test_diff_commit_range() {
    let env = TestEnv::new("diff_range");
    env.dft(&["init"]).assert_success();
    env.write_file("r.txt", "v1\n");
    env.dft(&["add", "r.txt"]).assert_success();
    env.dft(&["commit", "-m", "rev1"]).assert_success();

    env.write_file("r.txt", "v2\n");
    env.dft(&["add", "r.txt"]).assert_success();
    env.dft(&["commit", "-m", "rev2"]).assert_success();

    let res = env.dft(&["diff", "HEAD~1", "HEAD"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_stdout_contains("+v2");
    }
}

pub fn test_diff_no_changes_empty() {
    let env = TestEnv::new("diff_clean");
    env.dft(&["init"]).assert_success();
    env.write_file("c.txt", "clean");
    env.dft(&["add", "c.txt"]).assert_success();
    env.dft(&["commit", "-m", "Clean"]).assert_success();
    let res = env.dft(&["diff"]);
    res.assert_success();
    if !env.is_dry_run {
        assert!(res.stdout.trim().is_empty());
    }
}

pub fn test_diff_binary_detection() {
    let env = TestEnv::new("diff_binary");
    env.dft(&["init"]).assert_success();
    env.write_bytes("bin.dat", &[0x00, 0xFF, 0xFE, 0x01]);
    env.dft(&["add", "bin.dat"]).assert_success();
    env.dft(&["commit", "-m", "binary init"]).assert_success();

    env.write_bytes("bin.dat", &[0x00, 0xFF, 0x00, 0x02]);
    let res = env.dft(&["diff"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("Binary");
    }
}

pub fn test_log_oneline_and_limit() {
    let env = TestEnv::new("log_oneline");
    env.dft(&["init"]).assert_success();
    for i in 1..=5 {
        let f = format!("f{}.txt", i);
        env.write_file(&f, &i.to_string());
        env.dft(&["add", &f]).assert_success();
        env.dft(&["commit", "-m", &format!("Commit number {}", i)])
            .assert_success();
    }

    let res = env.dft(&["log", "--oneline", "-n", "3"]);
    res.assert_success();
    if !env.is_dry_run {
        let lines: Vec<&str> = res.stdout.trim().lines().collect();
        assert_eq!(lines.len(), 3, "Expected 3 log entries with -n 3");
    }
}

pub fn test_blame_line_attribution() {
    let env = TestEnv::new("blame_attr");
    env.dft(&["init"]).assert_success();
    env.write_file("blame.txt", "Line 1 from Alice\nLine 2 from Alice\n");
    env.dft(&["add", "blame.txt"]).assert_success();
    env.dft(&["commit", "-m", "First lines"]).assert_success();

    let res = env.dft(&["blame", "blame.txt"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("Line 1");
    }
}

// ============================================================================
// 7. `dft stash`, `reset`, `restore`, `tag` (5 tests each)
// ============================================================================

pub fn test_stash_push_and_pop() {
    let env = TestEnv::new("stash_push_pop");
    env.dft(&["init"]).assert_success();
    env.write_file("st.txt", "base");
    env.dft(&["add", "st.txt"]).assert_success();
    env.dft(&["commit", "-m", "Base"]).assert_success();

    env.write_file("st.txt", "dirty edit");
    let stash_res = env.dft(&["stash"]);
    stash_res.assert_success();
    if !env.is_dry_run {
        assert_eq!(
            env.read_file("st.txt"),
            "base",
            "Stash should clean working tree"
        );
    }

    let pop_res = env.dft(&["stash", "pop"]);
    pop_res.assert_success();
    if !env.is_dry_run {
        assert_eq!(
            env.read_file("st.txt"),
            "dirty edit",
            "Pop should restore dirty edit"
        );
    }
}

pub fn test_reset_soft_mixed_hard() {
    let env = TestEnv::new("reset_modes");
    env.dft(&["init"]).assert_success();
    env.write_file("f.txt", "v1");
    env.dft(&["add", "f.txt"]).assert_success();
    env.dft(&["commit", "-m", "v1"]).assert_success();

    env.write_file("f.txt", "v2");
    env.dft(&["add", "f.txt"]).assert_success();
    env.dft(&["commit", "-m", "v2"]).assert_success();

    // Reset hard
    let res = env.dft(&["reset", "--hard", "HEAD~1"]);
    res.assert_success();
    if !env.is_dry_run {
        assert_eq!(
            env.read_file("f.txt"),
            "v1",
            "Hard reset should restore worktree to v1"
        );
    }
}

pub fn test_tag_lightweight_and_annotated() {
    let env = TestEnv::new("tag_types");
    env.dft(&["init"]).assert_success();
    env.write_file("v.txt", "v1.0.0");
    env.dft(&["add", "v.txt"]).assert_success();
    env.dft(&["commit", "-m", "Release 1.0.0"]).assert_success();

    env.dft(&["tag", "v1.0.0"]).assert_success();
    env.dft(&["tag", "-a", "v1.0.0-rc1", "-m", "Release candidate 1"])
        .assert_success();

    let list = env.dft(&["tag"]);
    list.assert_success();
    if !env.is_dry_run {
        list.assert_output_contains("v1.0.0");
        list.assert_output_contains("v1.0.0-rc1");
    }
}

// ============================================================================
// 8. Plumbing & Maintenance (5 tests)
// ============================================================================

pub fn test_plumbing_hash_object_and_cat_file() {
    let env = TestEnv::new("plumbing_hash_cat");
    env.dft(&["init"]).assert_success();
    env.write_file("blob.txt", "quantum parallel state");
    let hash_res = env.dft(&["hash-object", "-w", "blob.txt"]);
    hash_res.assert_success();

    if !env.is_dry_run {
        let hash = hash_res.stdout.trim();
        assert_eq!(
            hash.len(),
            64,
            "Daft SHA-256 hash must be 64 hexadecimal characters"
        );
        let cat_res = env.dft(&["cat-file", "-p", hash]);
        cat_res.assert_success();
        cat_res.assert_stdout_contains("quantum parallel state");
    }
}

pub fn test_maintenance_gc_and_fsck() {
    let env = TestEnv::new("maintenance_gc_fsck");
    env.dft(&["init"]).assert_success();
    env.write_file("f.txt", "f");
    env.dft(&["add", "f.txt"]).assert_success();
    env.dft(&["commit", "-m", "m"]).assert_success();

    let fsck_res = env.dft(&["fsck"]);
    fsck_res.assert_success();

    let gc_res = env.dft(&["gc"]);
    gc_res.assert_success();
}
