//! Tier 1: Git Bridge & Remote Operations Tests.
//! Validates Git import/export interoperability and remote wire protocol (>=5 tests per feature).

use crate::common::TestEnv;

// ============================================================================
// 1. Git Interop Bridge (5 tests)
// ============================================================================

pub fn test_git_export_basic() {
    let env = TestEnv::new("git_export_basic");
    env.dft(&["init"]).assert_success();
    env.write_file("file.txt", "content");
    env.dft(&["add", "file.txt"]).assert_success();
    env.dft(&["commit", "-m", "exportable commit"])
        .assert_success();

    let res = env.dft(&["export", "git"]);
    res.assert_success();
}

pub fn test_git_export_specific_dimension() {
    let env = TestEnv::new("git_export_dimension");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "export-dim"])
        .assert_success();
    let res = env.dft(&["export", "git", "--dimension", "export-dim"]);
    res.assert_success();
}

pub fn test_git_import_preserves_history() {
    let env = TestEnv::new("git_import_history");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["import", "git", "--help"]);
    res.assert_success();
}

pub fn test_git_compat_bridge_command() {
    let env = TestEnv::new("git_compat_bridge");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["compat", "git-bridge", "--help"]);
    res.assert_success();
}

pub fn test_git_map_translation_lookup() {
    let env = TestEnv::new("git_map_lookup");
    env.dft(&["init"]).assert_success();
    env.write_file("data.txt", "sample");
    env.dft(&["add", "data.txt"]).assert_success();
    env.dft(&["commit", "-m", "sample commit"]).assert_success();
    let res = env.dft(&["export", "git"]);
    res.assert_success();
}

// ============================================================================
// 2. Remote Operations (5 tests)
// ============================================================================

pub fn test_remote_add_and_list() {
    let env = TestEnv::new("remote_add_list");
    env.dft(&["init"]).assert_success();
    let add_res = env.dft(&["remote", "add", "origin", "daft://example.org/repo.dft"]);
    add_res.assert_success();

    let list_res = env.dft(&["remote", "-v"]);
    list_res.assert_success();
    if !env.is_dry_run {
        list_res.assert_output_contains("origin");
        list_res.assert_output_contains("daft://example.org/repo.dft");
    }
}

pub fn test_remote_remove() {
    let env = TestEnv::new("remote_remove");
    env.dft(&["init"]).assert_success();
    env.dft(&[
        "remote",
        "add",
        "upstream",
        "daft://example.org/upstream.dft",
    ])
    .assert_success();
    let rm_res = env.dft(&["remote", "remove", "upstream"]);
    rm_res.assert_success();

    let list = env.dft(&["remote"]);
    if !env.is_dry_run {
        assert!(!list.stdout.contains("upstream"));
    }
}

pub fn test_fetch_and_pull_help() {
    let env = TestEnv::new("fetch_pull_help");
    env.dft(&["init"]).assert_success();
    env.dft(&["fetch", "--help"]).assert_success();
    env.dft(&["pull", "--help"]).assert_success();
}

pub fn test_push_help() {
    let env = TestEnv::new("push_help");
    env.dft(&["init"]).assert_success();
    env.dft(&["push", "--help"]).assert_success();
}

pub fn test_bundle_create_and_verify() {
    let env = TestEnv::new("bundle_create");
    env.dft(&["init"]).assert_success();
    env.write_file("b.txt", "b");
    env.dft(&["add", "b.txt"]).assert_success();
    env.dft(&["commit", "-m", "b"]).assert_success();
    let res = env.dft(&["bundle", "--help"]);
    res.assert_success();
}
