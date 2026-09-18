//! Tier 3: Pairwise Cross-Feature Combinations Tests.
//! Validates multidimensional interactions between disparate subsystems.

use crate::common::TestEnv;

// ============================================================================
// 1. Branch + Dimension + Stash
// ============================================================================

pub fn test_pairwise_branch_dimension_stash() {
    let env = TestEnv::new("pair_branch_dim_stash");
    env.dft(&["init"]).assert_success();
    env.write_file("base.txt", "base");
    env.dft(&["add", "base.txt"]).assert_success();
    env.dft(&["commit", "-m", "base"]).assert_success();

    // Work in dimension A
    env.dft(&["dimension", "create", "dim-a"]).assert_success();
    env.dft(&["dimension", "enter", "dim-a"]).assert_success();
    env.write_file("work.txt", "in-progress draft");
    env.dft(&["stash"]).assert_success();

    // Work in dimension B on a new branch
    env.dft(&["dimension", "create", "dim-b"]).assert_success();
    env.dft(&["dimension", "enter", "dim-b"]).assert_success();
    env.dft(&["switch", "-c", "branch-in-b"]).assert_success();
    env.write_file("feature_b.txt", "feature b");
    env.dft(&["add", "feature_b.txt"]).assert_success();
    env.dft(&["commit", "-m", "commit in b"]).assert_success();

    // Return to dimension A and restore stash
    env.dft(&["dimension", "enter", "dim-a"]).assert_success();
    let pop_res = env.dft(&["stash", "pop"]);
    pop_res.assert_success();
    if !env.is_dry_run {
        assert_eq!(env.read_file("work.txt"), "in-progress draft");
    }
}

// ============================================================================
// 2. Territory Fence + Commit
// ============================================================================

pub fn test_pairwise_territory_fence_commit_blocking() {
    let env = TestEnv::new("pair_fence_commit");
    env.dft(&["init"]).assert_success();
    env.write_file("security/auth.rs", "fn auth() {}");
    env.dft(&["add", "security/auth.rs"]).assert_success();
    env.dft(&["commit", "-m", "init auth"]).assert_success();

    // Dimension B modifies path and stages it prior to fence
    env.dft(&["dimension", "create", "rogue-dim"])
        .assert_success();
    env.dft(&["dimension", "enter", "rogue-dim"])
        .assert_success();
    env.write_file("security/auth.rs", "fn auth() { /* hacked */ }");
    env.dft(&["add", "security/auth.rs"]).assert_success();

    // Dimension A creates a hard fence on the path
    env.dft(&["dimension", "create", "guardian-dim"])
        .assert_success();
    env.dft(&["dimension", "enter", "guardian-dim"])
        .assert_success();
    env.dft(&["fence", "security/auth.rs", "--hard"])
        .assert_success();

    // Dimension B attempts to commit its staged change
    env.dft(&["dimension", "enter", "rogue-dim"])
        .assert_success();
    let commit_res = env.dft(&["commit", "-m", "rogue update"]);
    if !env.is_dry_run {
        commit_res.assert_failure();
        commit_res.assert_output_contains("FENCE");
    }
}

// ============================================================================
// 3. Dimension Live Fork + Uncommitted Changes + Status
// ============================================================================

pub fn test_pairwise_dimension_live_fork_uncommitted_status() {
    let env = TestEnv::new("pair_fork_uncommitted");
    env.dft(&["init"]).assert_success();
    env.write_file("tracked.txt", "v1");
    env.dft(&["add", "tracked.txt"]).assert_success();
    env.dft(&["commit", "-m", "v1"]).assert_success();

    // Uncommitted staged and unstaged files
    env.write_file("staged_edit.txt", "staged data");
    env.dft(&["add", "staged_edit.txt"]).assert_success();
    env.write_file("untracked_edit.txt", "untracked data");

    // Live fork into new dimension
    let fork_res = env.dft(&["dimension", "fork", "live-clone", "--from", "mainline"]);
    fork_res.assert_success();

    env.dft(&["dimension", "enter", "live-clone"])
        .assert_success();
    let status_res = env.dft(&["status"]);
    status_res.assert_success();
    if !env.is_dry_run {
        status_res.assert_output_contains("staged_edit.txt");
        status_res.assert_output_contains("untracked_edit.txt");
    }
}

// ============================================================================
// 4. Entangle + Cronos + Diff
// ============================================================================

pub fn test_pairwise_entangle_cronos_diff() {
    let env = TestEnv::new("pair_entangle_cronos");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "peer-alpha"])
        .assert_success();
    env.dft(&["dimension", "create", "peer-beta"])
        .assert_success();

    // Entangle dimensions on shared models
    env.dft(&[
        "entangle",
        "peer-alpha",
        "peer-beta",
        "--paths",
        "models/**",
    ])
    .assert_success();

    // Make edit in peer-alpha
    env.dft(&["dimension", "enter", "peer-alpha"])
        .assert_success();
    env.write_file("models/user.rs", "pub struct User { pub id: u64 }");
    env.dft(&["add", "models/user.rs"]).assert_success();
    env.dft(&["commit", "-m", "add user model"])
        .assert_success();

    // Observe diff across entangled dimensions
    let diff_res = env.dft(&[
        "observe",
        "diff",
        "peer-alpha",
        "peer-beta",
        "models/user.rs",
    ]);
    diff_res.assert_success();
}

// ============================================================================
// 5. Foresee + Converge Validation
// ============================================================================

pub fn test_pairwise_foresee_converge_validation() {
    let env = TestEnv::new("pair_foresee_converge");
    env.dft(&["init"]).assert_success();
    env.write_file("main.rs", "fn main() {}\n");
    env.dft(&["add", "main.rs"]).assert_success();
    env.dft(&["commit", "-m", "init"]).assert_success();

    env.dft(&["dimension", "create", "branch-a"])
        .assert_success();
    env.dft(&["dimension", "enter", "branch-a"])
        .assert_success();
    env.write_file("module_a.rs", "pub fn a() {}\n");
    env.dft(&["add", "module_a.rs"]).assert_success();
    env.dft(&["commit", "-m", "module a"]).assert_success();

    env.dft(&["dimension", "create", "branch-b"])
        .assert_success();
    env.dft(&["dimension", "enter", "branch-b"])
        .assert_success();
    env.write_file("module_b.rs", "pub fn b() {}\n");
    env.dft(&["add", "module_b.rs"]).assert_success();
    env.dft(&["commit", "-m", "module b"]).assert_success();

    // Foresee predicts 0 conflicts
    let foresee = env.dft(&["foresee", "branch-a", "branch-b"]);
    foresee.assert_success();

    // Converge executes cleanly
    let converge = env.dft(&[
        "converge",
        "branch-a",
        "branch-b",
        "--into",
        "converged-target",
    ]);
    converge.assert_success();
}

// ============================================================================
// 6. Weave + Commits + Timeline
// ============================================================================

pub fn test_pairwise_weave_commits_timeline() {
    let env = TestEnv::new("pair_weave_timeline");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "timeline-1"])
        .assert_success();
    env.dft(&["dimension", "create", "timeline-2"])
        .assert_success();

    env.dft(&["dimension", "enter", "timeline-1"])
        .assert_success();
    env.write_file("t1.txt", "1");
    env.dft(&["add", "t1.txt"]).assert_success();
    env.dft(&["commit", "-m", "t1 commit"]).assert_success();

    env.dft(&["dimension", "enter", "timeline-2"])
        .assert_success();
    env.write_file("t2.txt", "2");
    env.dft(&["add", "t2.txt"]).assert_success();
    env.dft(&["commit", "-m", "t2 commit"]).assert_success();

    let weave_res = env.dft(&["weave", "timeline-1", "timeline-2"]);
    weave_res.assert_success();

    let timeline_res = env.dft(&["timeline", "--ancestry"]);
    timeline_res.assert_success();
}

// ============================================================================
// 7. Observe + Dirty State
// ============================================================================

pub fn test_pairwise_observe_dirty_workspace() {
    let env = TestEnv::new("pair_observe_dirty");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "active-worker"])
        .assert_success();

    env.dft(&["dimension", "enter", "active-worker"])
        .assert_success();
    env.write_file("wip.txt", "in progress content not committed");

    // Observe from mainline
    env.dft(&["dimension", "enter", "mainline"])
        .assert_success();
    let obs_status = env.dft(&["observe", "status", "active-worker"]);
    obs_status.assert_success();
    if !env.is_dry_run {
        obs_status.assert_output_contains("wip.txt");
    }
}

// ============================================================================
// 8. Collapse + Territory Claims
// ============================================================================

pub fn test_pairwise_collapse_territory_claims() {
    let env = TestEnv::new("pair_collapse_territory");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "worker-auth"])
        .assert_success();
    env.dft(&["claim", "src/auth/", "--dimension", "worker-auth"])
        .assert_success();

    env.dft(&["dimension", "enter", "worker-auth"])
        .assert_success();
    env.write_file("src/auth/jwt.rs", "pub fn sign() {}");
    env.dft(&["add", "src/auth/jwt.rs"]).assert_success();
    env.dft(&["commit", "-m", "add jwt"]).assert_success();

    env.dft(&["dimension", "enter", "mainline"])
        .assert_success();
    let collapse_res = env.dft(&["collapse"]);
    collapse_res.assert_success();
}

// ============================================================================
// 9. Agent Assignment + Dimension Context
// ============================================================================

pub fn test_pairwise_agent_assignment_dimension_context() {
    let env = TestEnv::new("pair_agent_dim");
    env.dft(&["init"]).assert_success();
    env.dft(&["agent", "register", "agent-orchestrator", "--type", "ai"])
        .assert_success();
    env.dft(&["dimension", "create", "dim-orchestration"])
        .assert_success();
    env.dft(&["agent", "assign", "agent-orchestrator", "dim-orchestration"])
        .assert_success();

    let status = env.dft(&["agent", "status"]);
    status.assert_success();
    if !env.is_dry_run {
        status.assert_output_contains("dim-orchestration");
    }
}

// ============================================================================
// 10. Tag + Dimension Checkout
// ============================================================================

pub fn test_pairwise_tag_dimension_checkout() {
    let env = TestEnv::new("pair_tag_dim");
    env.dft(&["init"]).assert_success();
    env.write_file("release.txt", "v1.0.0-gold");
    env.dft(&["add", "release.txt"]).assert_success();
    env.dft(&["commit", "-m", "gold release"]).assert_success();
    env.dft(&["tag", "-a", "v1.0.0-gold", "-m", "stable release tag"])
        .assert_success();

    // Create dimension from tag
    let res = env.dft(&[
        "dimension",
        "create",
        "hotfix-v1.0",
        "--from",
        "v1.0.0-gold",
    ]);
    res.assert_success();
}
