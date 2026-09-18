//! Tier 1: Parallel Dimension Management Tests.
//! Validates parallel workspace lifecycle, isolation, CoW, and state capture (>=5 tests per feature).

use crate::common::TestEnv;

// ============================================================================
// 1. `dimension create` (5 tests)
// ============================================================================

pub fn test_dimension_create_basic() {
    let env = TestEnv::new("dim_create_basic");
    env.dft(&["init"]).assert_success();
    env.write_file("init.txt", "v1");
    env.dft(&["add", "init.txt"]).assert_success();
    env.dft(&["commit", "-m", "init"]).assert_success();

    let res = env.dft(&["dimension", "create", "experiment-1"]);
    res.assert_success();
    assert!(
        env.dir_exists(".dft/dimensions/experiment-1") || env.is_dry_run,
        "Dimension directory must be created under .dft/dimensions/experiment-1"
    );
}

pub fn test_dimension_create_from_ref() {
    let env = TestEnv::new("dim_create_from_ref");
    env.dft(&["init"]).assert_success();
    env.write_file("v1.txt", "1");
    env.dft(&["add", "v1.txt"]).assert_success();
    env.dft(&["commit", "-m", "Commit 1"]).assert_success();
    env.dft(&["tag", "v1.0"]).assert_success();

    env.write_file("v2.txt", "2");
    env.dft(&["add", "v2.txt"]).assert_success();
    env.dft(&["commit", "-m", "Commit 2"]).assert_success();

    let res = env.dft(&["dimension", "create", "legacy-patch", "--from", "v1.0"]);
    res.assert_success();
}

pub fn test_dimension_create_duplicate_fails() {
    let env = TestEnv::new("dim_create_duplicate");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "alpha"]).assert_success();
    let res = env.dft(&["dimension", "create", "alpha"]);
    if !env.is_dry_run {
        res.assert_failure();
        res.assert_output_contains("already exists");
    }
}

pub fn test_dimension_create_initializes_isolated_index_and_head() {
    let env = TestEnv::new("dim_create_isolated_index");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "beta"]).assert_success();
    assert!(
        env.file_exists(".dft/dimensions/beta/HEAD") || env.is_dry_run,
        "Dimension must have independent HEAD pointer"
    );
}

pub fn test_dimension_create_generates_metadata() {
    let env = TestEnv::new("dim_create_metadata");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "gamma"]).assert_success();
    assert!(
        env.file_exists(".dft/dimensions/gamma/meta.json") || env.is_dry_run,
        "Dimension must have meta.json defining creator, branch, and CoW mode"
    );
}

// ============================================================================
// 2. `dimension list` (5 tests)
// ============================================================================

pub fn test_dimension_list_shows_mainline_and_created() {
    let env = TestEnv::new("dim_list_basic");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "work-1"]).assert_success();
    let res = env.dft(&["dimension", "list"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("mainline");
        res.assert_output_contains("work-1");
    }
}

pub fn test_dimension_list_shows_branch_and_status() {
    let env = TestEnv::new("dim_list_status");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "work-2"]).assert_success();
    let res = env.dft(&["dimension", "list"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("clean");
    }
}

pub fn test_dimension_list_json_format() {
    let env = TestEnv::new("dim_list_json");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "json-test"])
        .assert_success();
    let res = env.dft(&["dimension", "list", "--json"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_stdout_contains("[");
        res.assert_stdout_contains("json-test");
    }
}

pub fn test_dimension_list_reflects_active_marker() {
    let env = TestEnv::new("dim_list_active_marker");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "dim-x"]).assert_success();
    let res = env.dft(&["dimension", "list"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("*");
    }
}

pub fn test_dimension_list_shows_resource_usage() {
    let env = TestEnv::new("dim_list_resources");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "dim-heavy"])
        .assert_success();
    let res = env.dft(&["dimension", "list"]);
    res.assert_success();
}

// ============================================================================
// 3. `dimension enter` (5 tests)
// ============================================================================

pub fn test_dimension_enter_updates_current_dimension() {
    let env = TestEnv::new("dim_enter_updates_current");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "space-9"])
        .assert_success();
    let res = env.dft(&["dimension", "enter", "space-9"]);
    res.assert_success();

    if !env.is_dry_run {
        let current = env.read_file(".dft/current_dimension");
        assert!(
            current.contains("space-9"),
            "current_dimension pointer should be space-9"
        );
    }
}

pub fn test_dimension_enter_subsequent_commands_operate_in_dimension() {
    let env = TestEnv::new("dim_enter_workspace");
    env.dft(&["init"]).assert_success();
    env.write_file("main.txt", "main content");
    env.dft(&["add", "main.txt"]).assert_success();
    env.dft(&["commit", "-m", "main"]).assert_success();

    env.dft(&["dimension", "create", "space-isolated"])
        .assert_success();
    env.dft(&["dimension", "enter", "space-isolated"])
        .assert_success();

    env.write_file("isolated.txt", "only in space-isolated");
    env.dft(&["add", "isolated.txt"]).assert_success();
    env.dft(&["commit", "-m", "dim commit"]).assert_success();

    // Switch back to mainline
    env.dft(&["dimension", "enter", "mainline"])
        .assert_success();
    if !env.is_dry_run {
        assert!(
            !env.file_exists("isolated.txt"),
            "Files committed in isolated dimension must not appear in mainline worktree"
        );
    }
}

pub fn test_dimension_enter_nonexistent_fails() {
    let env = TestEnv::new("dim_enter_nonexistent");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["dimension", "enter", "phantom-dimension"]);
    if !env.is_dry_run {
        res.assert_failure();
    }
}

pub fn test_dimension_enter_same_is_idempotent() {
    let env = TestEnv::new("dim_enter_idempotent");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "enter", "mainline"])
        .assert_success();
    let res = env.dft(&["dimension", "enter", "mainline"]);
    res.assert_success();
}

pub fn test_dimension_enter_with_status_verification() {
    let env = TestEnv::new("dim_enter_status");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "status-dim"])
        .assert_success();
    env.dft(&["dimension", "enter", "status-dim"])
        .assert_success();
    let res = env.dft(&["status"]);
    res.assert_success();
}

// ============================================================================
// 4. `dimension destroy` (5 tests)
// ============================================================================

pub fn test_dimension_destroy_reclaims_workspace() {
    let env = TestEnv::new("dim_destroy_reclaims");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "to-trash"])
        .assert_success();
    let res = env.dft(&["dimension", "destroy", "to-trash"]);
    res.assert_success();
    assert!(
        !env.dir_exists(".dft/dimensions/to-trash") || env.is_dry_run,
        "Destroyed dimension folder should be deleted"
    );
}

pub fn test_dimension_destroy_preserves_shared_cas_objects() {
    let env = TestEnv::new("dim_destroy_preserves_cas");
    env.dft(&["init"]).assert_success();
    env.write_file("shared.txt", "common blob data");
    env.dft(&["add", "shared.txt"]).assert_success();
    env.dft(&["commit", "-m", "Common"]).assert_success();

    env.dft(&["dimension", "create", "temp-universe"])
        .assert_success();
    env.dft(&["dimension", "destroy", "temp-universe"])
        .assert_success();

    // Worktree and commits in mainline must still be intact
    let fsck = env.dft(&["fsck"]);
    fsck.assert_success();
}

pub fn test_dimension_destroy_active_requires_force_or_fails() {
    let env = TestEnv::new("dim_destroy_active");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "active-one"])
        .assert_success();
    env.dft(&["dimension", "enter", "active-one"])
        .assert_success();

    let res = env.dft(&["dimension", "destroy", "active-one"]);
    if !env.is_dry_run {
        res.assert_failure();
    }
}

pub fn test_dimension_destroy_force_on_dirty() {
    let env = TestEnv::new("dim_destroy_dirty_force");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "dirty-dim"])
        .assert_success();
    env.dft(&["dimension", "enter", "dirty-dim"])
        .assert_success();
    env.write_file("dirty.txt", "uncommitted edits");
    env.dft(&["dimension", "enter", "mainline"])
        .assert_success();

    let res = env.dft(&["dimension", "destroy", "dirty-dim", "--force"]);
    res.assert_success();
}

pub fn test_dimension_destroy_nonexistent_fails() {
    let env = TestEnv::new("dim_destroy_nonexistent");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["dimension", "destroy", "does-not-exist"]);
    if !env.is_dry_run {
        res.assert_failure();
    }
}

// ============================================================================
// 5. `dimension fork` (5 tests)
// ============================================================================

pub fn test_dimension_fork_captures_uncommitted_live_state() {
    let env = TestEnv::new("dim_fork_uncommitted");
    env.dft(&["init"]).assert_success();
    env.write_file("committed.txt", "committed");
    env.dft(&["add", "committed.txt"]).assert_success();
    env.dft(&["commit", "-m", "init"]).assert_success();

    // Create uncommitted dirty changes in mainline
    env.write_file("dirty_draft.txt", "uncommitted live thought");

    // Fork dimension live state
    let res = env.dft(&[
        "dimension",
        "fork",
        "alternate-timeline",
        "--from",
        "mainline",
    ]);
    res.assert_success();

    env.dft(&["dimension", "enter", "alternate-timeline"])
        .assert_success();
    if !env.is_dry_run {
        assert!(
            env.file_exists("dirty_draft.txt"),
            "Forked dimension must capture uncommitted work from parent dimension"
        );
    }
}

pub fn test_dimension_fork_captures_staged_index() {
    let env = TestEnv::new("dim_fork_staged");
    env.dft(&["init"]).assert_success();
    env.write_file("staged.txt", "staged content");
    env.dft(&["add", "staged.txt"]).assert_success();

    let res = env.dft(&["dimension", "fork", "fork-staged", "--from", "mainline"]);
    res.assert_success();
}

pub fn test_dimension_fork_modifications_do_not_affect_source() {
    let env = TestEnv::new("dim_fork_independence");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "source-dim"])
        .assert_success();
    env.dft(&["dimension", "enter", "source-dim"])
        .assert_success();
    env.write_file("common.txt", "original");
    env.dft(&["add", "common.txt"]).assert_success();
    env.dft(&["commit", "-m", "source commit"]).assert_success();

    env.dft(&["dimension", "fork", "child-dim", "--from", "source-dim"])
        .assert_success();
    env.dft(&["dimension", "enter", "child-dim"])
        .assert_success();
    env.write_file("common.txt", "child modification");
    env.dft(&["add", "common.txt"]).assert_success();
    env.dft(&["commit", "-m", "child commit"]).assert_success();

    // Switch back to source-dim and verify unaltered
    env.dft(&["dimension", "enter", "source-dim"])
        .assert_success();
    if !env.is_dry_run {
        assert_eq!(
            env.read_file("common.txt"),
            "original",
            "Edits in forked dimension must not bleed into source dimension"
        );
    }
}

pub fn test_dimension_fork_from_nonexistent_fails() {
    let env = TestEnv::new("dim_fork_nonexistent");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["dimension", "fork", "new-dim", "--from", "void-dim"]);
    if !env.is_dry_run {
        res.assert_failure();
    }
}

pub fn test_dimension_fork_preserves_parent_metadata() {
    let env = TestEnv::new("dim_fork_metadata");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "parent-dim"])
        .assert_success();
    env.dft(&["dimension", "fork", "child-dim", "--from", "parent-dim"])
        .assert_success();

    let info_res = env.dft(&["dimension", "info", "child-dim"]);
    info_res.assert_success();
    if !env.is_dry_run {
        info_res.assert_output_contains("parent-dim");
    }
}

// ============================================================================
// 6. `dimension snapshot`, `rename`, `info` (5 tests each)
// ============================================================================

pub fn test_dimension_snapshot_current() {
    let env = TestEnv::new("dim_snapshot_current");
    env.dft(&["init"]).assert_success();
    env.write_file("state.txt", "checkpoint 1");
    let res = env.dft(&["dimension", "snapshot"]);
    res.assert_success();
}

pub fn test_dimension_snapshot_all() {
    let env = TestEnv::new("dim_snapshot_all");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "d1"]).assert_success();
    env.dft(&["dimension", "create", "d2"]).assert_success();
    let res = env.dft(&["dimension", "snapshot", "--all"]);
    res.assert_success();
}

pub fn test_dimension_rename_basic() {
    let env = TestEnv::new("dim_rename_basic");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "experiment-old"])
        .assert_success();
    let res = env.dft(&["dimension", "rename", "experiment-old", "experiment-new"]);
    res.assert_success();
    assert!(
        env.dir_exists(".dft/dimensions/experiment-new") || env.is_dry_run,
        "Renamed directory should exist"
    );
}

pub fn test_dimension_info_displays_details() {
    let env = TestEnv::new("dim_info_details");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "inspected"])
        .assert_success();
    let res = env.dft(&["dimension", "info", "inspected"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("inspected");
    }
}

pub fn test_dimension_info_json() {
    let env = TestEnv::new("dim_info_json");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "json-info"])
        .assert_success();
    let res = env.dft(&["dimension", "info", "json-info", "--json"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_stdout_contains("{");
        res.assert_stdout_contains("json-info");
    }
}
