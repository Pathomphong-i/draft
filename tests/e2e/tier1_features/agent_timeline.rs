//! Tier 1: Multi-Agent Subsystem & Multiverse Timeline Tests.
//! Validates agent identity, mailboxes, heartbeats, and multiverse graph visualization (>=5 tests per feature).

use crate::common::TestEnv;

// ============================================================================
// 1. `agent` (7 tests)
// ============================================================================

pub fn test_agent_register_ai_and_human() {
    let env = TestEnv::new("agent_register");
    env.dft(&["init"]).assert_success();

    let r1 = env.dft(&["agent", "register", "agent-codex", "--type", "ai"]);
    r1.assert_success();

    let r2 = env.dft(&["agent", "register", "linus-human", "--type", "human"]);
    r2.assert_success();

    let list = env.dft(&["agent", "list"]);
    list.assert_success();
    if !env.is_dry_run {
        list.assert_output_contains("agent-codex");
        list.assert_output_contains("linus-human");
    }
}

pub fn test_agent_assign_dimension() {
    let env = TestEnv::new("agent_assign");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "dim-auth"])
        .assert_success();
    env.dft(&["agent", "register", "auth-bot", "--type", "ai"])
        .assert_success();

    let assign_res = env.dft(&["agent", "assign", "auth-bot", "dim-auth"]);
    assign_res.assert_success();

    let list = env.dft(&["agent", "list"]);
    list.assert_success();
    if !env.is_dry_run {
        list.assert_output_contains("dim-auth");
    }
}

pub fn test_agent_status_activity() {
    let env = TestEnv::new("agent_status");
    env.dft(&["init"]).assert_success();
    env.dft(&["agent", "register", "worker-1"]).assert_success();
    let res = env.dft(&["agent", "status"]);
    res.assert_success();
}

pub fn test_agent_broadcast_and_inbox() {
    let env = TestEnv::new("agent_broadcast_inbox");
    env.dft(&["init"]).assert_success();
    env.dft(&["agent", "register", "agent-sender"])
        .assert_success();
    env.dft(&["agent", "register", "agent-receiver"])
        .assert_success();

    let bcast = env.dft(&["agent", "broadcast", "Coordinate: refactoring core modules"]);
    bcast.assert_success();

    let inbox = env.dft(&["agent", "inbox", "agent-receiver"]);
    inbox.assert_success();
    if !env.is_dry_run {
        inbox.assert_output_contains("Coordinate: refactoring core modules");
    }
}

pub fn test_agent_heartbeat_reporting() {
    let env = TestEnv::new("agent_heartbeat");
    env.dft(&["init"]).assert_success();
    env.dft(&["agent", "register", "pulse-agent"])
        .assert_success();
    let res = env.dft(&["agent", "heartbeat"]);
    res.assert_success();
}

pub fn test_agent_inbox_json_format() {
    let env = TestEnv::new("agent_inbox_json");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["agent", "inbox", "--json"]);
    res.assert_success();
}

pub fn test_agent_register_duplicate_name_fails() {
    let env = TestEnv::new("agent_dup");
    env.dft(&["init"]).assert_success();
    env.dft(&["agent", "register", "unique-agent"])
        .assert_success();
    let res = env.dft(&["agent", "register", "unique-agent"]);
    if !env.is_dry_run {
        res.assert_failure();
    }
}

// ============================================================================
// 2. `timeline` (5 tests)
// ============================================================================

pub fn test_timeline_multiverse_graph_rendering() {
    let env = TestEnv::new("timeline_graph");
    env.dft(&["init"]).assert_success();
    env.write_file("init.txt", "v");
    env.dft(&["add", "init.txt"]).assert_success();
    env.dft(&["commit", "-m", "init"]).assert_success();

    env.dft(&["dimension", "create", "parallel-1"])
        .assert_success();
    env.dft(&["dimension", "create", "parallel-2"])
        .assert_success();

    let res = env.dft(&["timeline"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("mainline");
        res.assert_output_contains("parallel-1");
        res.assert_output_contains("parallel-2");
    }
}

pub fn test_timeline_single_dimension_history() {
    let env = TestEnv::new("timeline_single");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "focused-dim"])
        .assert_success();
    let res = env.dft(&["timeline", "focused-dim"]);
    res.assert_success();
}

pub fn test_timeline_ancestry_graph() {
    let env = TestEnv::new("timeline_ancestry");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "ancestor"])
        .assert_success();
    env.dft(&["dimension", "fork", "descendant", "--from", "ancestor"])
        .assert_success();

    let res = env.dft(&["timeline", "--ancestry"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_output_contains("ancestor");
        res.assert_output_contains("descendant");
    }
}

pub fn test_timeline_export_json() {
    let env = TestEnv::new("timeline_export_json");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["timeline", "export", "--format", "json"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_stdout_contains("{");
    }
}

pub fn test_timeline_export_dot() {
    let env = TestEnv::new("timeline_export_dot");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["timeline", "export", "--format", "dot"]);
    res.assert_success();
    if !env.is_dry_run {
        res.assert_stdout_contains("digraph");
    }
}
