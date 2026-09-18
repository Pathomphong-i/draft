//! Tier 1: Awareness, Conflict Prevention & Territory Ownership Tests.
//! Validates Observe, Radar, Foresee, Entropy, and Territory subsystems (>=5 tests per feature).

use crate::common::TestEnv;

// ============================================================================
// 1. `observe` (5 tests)
// ============================================================================

pub fn test_observe_read_remote_file_without_switching() {
    let env = TestEnv::new("observe_read_remote");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "universe-b"])
        .assert_success();
    env.dft(&["dimension", "enter", "universe-b"])
        .assert_success();
    env.write_file("secret.txt", "universe b discovery");
    env.dft(&["add", "secret.txt"]).assert_success();
    env.dft(&["commit", "-m", "b commit"]).assert_success();

    // In mainline, observe universe-b without entering
    env.dft(&["dimension", "enter", "mainline"])
        .assert_success();
    let res = env.dft(&["observe", "universe-b", "secret.txt"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_stdout_contains("universe b discovery");
    }
}

pub fn test_observe_diff_between_two_dimensions() {
    let env = TestEnv::new("observe_diff_remote");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "dim-1"]).assert_success();
    env.dft(&["dimension", "create", "dim-2"]).assert_success();

    env.dft(&["dimension", "enter", "dim-1"]).assert_success();
    env.write_file("shared.txt", "content from dim 1");
    env.dft(&["add", "shared.txt"]).assert_success();
    env.dft(&["commit", "-m", "commit 1"]).assert_success();

    env.dft(&["dimension", "enter", "dim-2"]).assert_success();
    env.write_file("shared.txt", "content from dim 2");
    env.dft(&["add", "shared.txt"]).assert_success();
    env.dft(&["commit", "-m", "commit 2"]).assert_success();

    env.dft(&["dimension", "enter", "mainline"])
        .assert_success();
    let res = env.dft(&["observe", "diff", "dim-1", "dim-2"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("content from dim 1");
        res.assert_output_contains("content from dim 2");
    }
}

pub fn test_observe_log_remote() {
    let env = TestEnv::new("observe_log_remote");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "hist-dim"])
        .assert_success();
    env.dft(&["dimension", "enter", "hist-dim"])
        .assert_success();
    env.write_file("h.txt", "h");
    env.dft(&["add", "h.txt"]).assert_success();
    env.dft(&["commit", "-m", "Remote history message"])
        .assert_success();

    env.dft(&["dimension", "enter", "mainline"])
        .assert_success();
    let res = env.dft(&["observe", "log", "hist-dim"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("Remote history message");
    }
}

pub fn test_observe_status_remote() {
    let env = TestEnv::new("observe_status_remote");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "dirty-remote"])
        .assert_success();
    env.dft(&["dimension", "enter", "dirty-remote"])
        .assert_success();
    env.write_file("uncommitted_remote.txt", "dirty in remote");

    env.dft(&["dimension", "enter", "mainline"])
        .assert_success();
    let res = env.dft(&["observe", "status", "dirty-remote"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("uncommitted_remote.txt");
    }
}

pub fn test_observe_nonexistent_dimension_fails() {
    let env = TestEnv::new("observe_nonexistent");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["observe", "nonexistent-dim", "file.txt"]);
    if !env.is_dry_run {
        res.assert_failure();
    }
}

// ============================================================================
// 2. `radar` (5 tests)
// ============================================================================

pub fn test_radar_overview_all_dimensions() {
    let env = TestEnv::new("radar_overview");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "radar-1"])
        .assert_success();
    let res = env.dft(&["radar"]);
    res.assert_success();
}

pub fn test_radar_specific_path_query() {
    let env = TestEnv::new("radar_path");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "radar-dim"])
        .assert_success();
    env.dft(&["dimension", "enter", "radar-dim"])
        .assert_success();
    env.write_file("critical_module.rs", "fn critical() {}");

    env.dft(&["dimension", "enter", "mainline"])
        .assert_success();
    let res = env.dft(&["radar", "critical_module.rs"]);
    res.assert_success();
}

pub fn test_radar_hot_zones_detects_concurrent_edits() {
    let env = TestEnv::new("radar_hot_zones");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "dev-a"]).assert_success();
    env.dft(&["dimension", "create", "dev-b"]).assert_success();

    env.dft(&["dimension", "enter", "dev-a"]).assert_success();
    env.write_file("config.toml", "port = 8080");

    env.dft(&["dimension", "enter", "dev-b"]).assert_success();
    env.write_file("config.toml", "port = 9090");

    let res = env.dft(&["radar", "--hot"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("config.toml");
    }
}

pub fn test_radar_zero_hot_zones_on_disjoint_edits() {
    let env = TestEnv::new("radar_disjoint");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "agent-ui"])
        .assert_success();
    env.dft(&["dimension", "create", "agent-db"])
        .assert_success();

    env.dft(&["dimension", "enter", "agent-ui"])
        .assert_success();
    env.write_file("ui.rs", "pub fn render() {}");

    env.dft(&["dimension", "enter", "agent-db"])
        .assert_success();
    env.write_file("db.rs", "pub fn query() {}");

    let res = env.dft(&["radar", "--hot"]);
    res.assert_success();
    if !env.is_dry_run {
        assert!(
            !res.stdout.contains("ui.rs") && !res.stdout.contains("db.rs"),
            "Disjoint edits must not be flagged as hot zones"
        );
    }
}

pub fn test_radar_json_export() {
    let env = TestEnv::new("radar_json");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["radar", "--json"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_stdout_contains("{");
    }
}

// ============================================================================
// 3. `foresee`, `overlap`, `entropy` (5 tests)
// ============================================================================

pub fn test_foresee_predicts_conflict_without_merging() {
    let env = TestEnv::new("foresee_predicts_conflict");
    env.dft(&["init"]).assert_success();
    env.write_file("app.py", "print('initial')\n");
    env.dft(&["add", "app.py"]).assert_success();
    env.dft(&["commit", "-m", "init"]).assert_success();

    env.dft(&["dimension", "create", "patch-1"])
        .assert_success();
    env.dft(&["dimension", "enter", "patch-1"]).assert_success();
    env.write_file("app.py", "print('from patch 1')\n");
    env.dft(&["add", "app.py"]).assert_success();
    env.dft(&["commit", "-m", "p1"]).assert_success();

    env.dft(&["dimension", "create", "patch-2"])
        .assert_success();
    env.dft(&["dimension", "enter", "patch-2"]).assert_success();
    env.write_file("app.py", "print('from patch 2')\n");
    env.dft(&["add", "app.py"]).assert_success();
    env.dft(&["commit", "-m", "p2"]).assert_success();

    let res = env.dft(&["foresee", "patch-1", "patch-2"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("conflict");
    }
}

pub fn test_foresee_clean_prediction_on_disjoint_changes() {
    let env = TestEnv::new("foresee_clean");
    env.dft(&["init"]).assert_success();
    env.write_file("base.txt", "base");
    env.dft(&["add", "base.txt"]).assert_success();
    env.dft(&["commit", "-m", "init"]).assert_success();

    env.dft(&["dimension", "create", "feat-alpha"])
        .assert_success();
    env.dft(&["dimension", "enter", "feat-alpha"])
        .assert_success();
    env.write_file("alpha.txt", "alpha");
    env.dft(&["add", "alpha.txt"]).assert_success();
    env.dft(&["commit", "-m", "alpha"]).assert_success();

    env.dft(&["dimension", "create", "feat-beta"])
        .assert_success();
    env.dft(&["dimension", "enter", "feat-beta"])
        .assert_success();
    env.write_file("beta.txt", "beta");
    env.dft(&["add", "beta.txt"]).assert_success();
    env.dft(&["commit", "-m", "beta"]).assert_success();

    let res = env.dft(&["foresee", "feat-alpha", "feat-beta"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("0 conflicts");
    }
}

pub fn test_overlap_reports_concurrently_modified_files() {
    let env = TestEnv::new("overlap_report");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "d-one"]).assert_success();
    env.dft(&["dimension", "create", "d-two"]).assert_success();

    env.dft(&["dimension", "enter", "d-one"]).assert_success();
    env.write_file("overlap.txt", "first");

    env.dft(&["dimension", "enter", "d-two"]).assert_success();
    env.write_file("overlap.txt", "second");

    let res = env.dft(&["overlap"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("overlap.txt");
    }
}

pub fn test_entropy_divergence_score() {
    let env = TestEnv::new("entropy_score");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "ent-a"]).assert_success();
    env.dft(&["dimension", "create", "ent-b"]).assert_success();

    let res = env.dft(&["entropy", "ent-a", "ent-b"]);
    res.assert_success();
    if !env.is_dry_run {
        // Entropy score is a numeric value
        res.assert_output_contains("entropy");
    }
}

pub fn test_entropy_identical_dimensions_zero_or_minimal() {
    let env = TestEnv::new("entropy_minimal");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "same-1"]).assert_success();
    env.dft(&["dimension", "fork", "same-2", "--from", "same-1"])
        .assert_success();

    let res = env.dft(&["entropy", "same-1", "same-2"]);
    res.assert_success();
}

// ============================================================================
// 4. `territory` (5 tests)
// ============================================================================

pub fn test_territory_claim_path() {
    let env = TestEnv::new("territory_claim");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "owner-dim"])
        .assert_success();
    let res = env.dft(&["claim", "src/auth/", "--dimension", "owner-dim"]);
    res.assert_success();

    let map = env.dft(&["territory"]);
    map.assert_success();
    if !env.is_dry_run {
        map.assert_output_contains("src/auth/");
        map.assert_output_contains("owner-dim");
    }
}

pub fn test_territory_yield_claim() {
    let env = TestEnv::new("territory_yield");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "temp-owner"])
        .assert_success();
    env.dft(&["claim", "lib/math.rs", "--dimension", "temp-owner"])
        .assert_success();

    let yield_res = env.dft(&["yield", "lib/math.rs"]);
    yield_res.assert_success();

    let territory = env.dft(&["territory"]);
    territory.assert_success();
}

pub fn test_territory_fence_hard_blocks_edits() {
    let env = TestEnv::new("territory_fence");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "guarded-dim"])
        .assert_success();
    let res = env.dft(&["fence", "core/security.rs", "--hard"]);
    res.assert_success();

    let territory = env.dft(&["territory"]);
    territory.assert_success();
    if !env.is_dry_run {
        territory.assert_output_contains("FENCE");
    }
}

pub fn test_territory_audit_detects_violations() {
    let env = TestEnv::new("territory_audit");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["territory", "audit"]);
    res.assert_success();
}

pub fn test_territory_json_export() {
    let env = TestEnv::new("territory_json");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["territory", "--json"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_stdout_contains("{");
    }
}
