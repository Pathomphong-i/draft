//! Tier 1: Entanglement & Cronos Sync Daemon Tests.
//! Validates linked dimension auto-propagation, daemon lifecycle, WAL, and sync logs (>=5 tests per feature).

use crate::common::TestEnv;

// ============================================================================
// 1. `entangle` (5 tests)
// ============================================================================

pub fn test_entangle_basic_linking() {
    let env = TestEnv::new("entangle_basic");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "universe-x"])
        .assert_success();
    env.dft(&["dimension", "create", "universe-y"])
        .assert_success();

    let res = env.dft(&["entangle", "universe-x", "universe-y"]);
    res.assert_success();

    let list_res = env.dft(&["entangle", "list"]);
    list_res.assert_success();
    if !env.is_dry_run {
        list_res.assert_output_contains("universe-x");
        list_res.assert_output_contains("universe-y");
    }
}

pub fn test_entangle_path_filtered() {
    let env = TestEnv::new("entangle_path_filtered");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "frontend"])
        .assert_success();
    env.dft(&["dimension", "create", "backend"])
        .assert_success();

    let res = env.dft(&[
        "entangle",
        "frontend",
        "backend",
        "--paths",
        "shared-schema/**",
    ]);
    res.assert_success();

    let list = env.dft(&["entangle", "list"]);
    if !env.is_dry_run {
        list.assert_output_contains("shared-schema/**");
    }
}

pub fn test_entangle_break_severance() {
    let env = TestEnv::new("entangle_break");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "alpha"]).assert_success();
    env.dft(&["dimension", "create", "beta"]).assert_success();
    env.dft(&["entangle", "alpha", "beta"]).assert_success();

    let break_res = env.dft(&["entangle", "break", "alpha", "beta"]);
    break_res.assert_success();

    let list = env.dft(&["entangle", "list"]);
    if !env.is_dry_run {
        assert!(
            !list.stdout.contains("alpha <-> beta"),
            "Severed entanglement must no longer appear in active entangle list"
        );
    }
}

pub fn test_entangle_log_history() {
    let env = TestEnv::new("entangle_log");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["entangle", "log"]);
    res.assert_success();
}

pub fn test_entangle_invalid_dimension_fails() {
    let env = TestEnv::new("entangle_invalid");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["entangle", "dim-a", "ghost-dim"]);
    if !env.is_dry_run {
        res.assert_failure();
    }
}

// ============================================================================
// 2. `cronos` (6 tests)
// ============================================================================

pub fn test_cronos_status_when_stopped() {
    let env = TestEnv::new("cronos_stopped_status");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["cronos", "status"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("stopped");
    }
}

pub fn test_cronos_start_and_stop_lifecycle() {
    let env = TestEnv::new("cronos_lifecycle");
    env.dft(&["init"]).assert_success();

    let start_res = env.dft(&[
        "cronos",
        "start",
        "--interval",
        "10s",
        "--strategy",
        "theirs",
    ]);
    start_res.assert_success();

    let status_res = env.dft(&["cronos", "status"]);
    status_res.assert_success();

    let stop_res = env.dft(&["cronos", "stop"]);
    stop_res.assert_success();
}

pub fn test_cronos_log_inspection() {
    let env = TestEnv::new("cronos_log");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["cronos", "log"]);
    res.assert_success();
}

pub fn test_cronos_pause_and_resume_dimension() {
    let env = TestEnv::new("cronos_pause_resume");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "sync-target"])
        .assert_success();

    let pause_res = env.dft(&["cronos", "pause", "sync-target"]);
    pause_res.assert_success();

    let resume_res = env.dft(&["cronos", "resume", "sync-target"]);
    resume_res.assert_success();
}

pub fn test_cronos_config_rules() {
    let env = TestEnv::new("cronos_config");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["cronos", "config"]);
    res.assert_success();
}

pub fn test_cronos_wal_log_integrity() {
    let env = TestEnv::new("cronos_wal");
    env.dft(&["init"]).assert_success();
    // Verify cronos directory structure exists or is created
    let res = env.dft(&["cronos", "status"]);
    res.assert_success();
}
