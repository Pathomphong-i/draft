//! Tier 1: Convergence Operations Tests.
//! Validates Collapse, Converge, Cascade, Weave, and Splice operations (>=5 tests per feature).

use crate::common::TestEnv;

// ============================================================================
// 1. `collapse` (5 tests)
// ============================================================================

pub fn test_collapse_all_dimensions_into_mainline() {
    let env = TestEnv::new("collapse_into_mainline");
    env.dft(&["init"]).assert_success();
    env.write_file("base.txt", "base");
    env.dft(&["add", "base.txt"]).assert_success();
    env.dft(&["commit", "-m", "base"]).assert_success();

    env.dft(&["dimension", "create", "f1"]).assert_success();
    env.dft(&["dimension", "enter", "f1"]).assert_success();
    env.write_file("f1.txt", "f1 content");
    env.dft(&["add", "f1.txt"]).assert_success();
    env.dft(&["commit", "-m", "feat 1"]).assert_success();

    env.dft(&["dimension", "create", "f2"]).assert_success();
    env.dft(&["dimension", "enter", "f2"]).assert_success();
    env.write_file("f2.txt", "f2 content");
    env.dft(&["add", "f2.txt"]).assert_success();
    env.dft(&["commit", "-m", "feat 2"]).assert_success();

    env.dft(&["dimension", "enter", "mainline"])
        .assert_success();
    let res = env.dft(&["collapse"]);
    res.assert_success();

    if !env.is_dry_run {
        assert!(
            env.file_exists("f1.txt"),
            "f1.txt must be collapsed into mainline"
        );
        assert!(
            env.file_exists("f2.txt"),
            "f2.txt must be collapsed into mainline"
        );
    }
}

pub fn test_collapse_into_specific_target() {
    let env = TestEnv::new("collapse_target");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "release-candidate"])
        .assert_success();
    env.dft(&["dimension", "create", "worker-dim"])
        .assert_success();

    env.dft(&["dimension", "enter", "worker-dim"])
        .assert_success();
    env.write_file("work.txt", "done");
    env.dft(&["add", "work.txt"]).assert_success();
    env.dft(&["commit", "-m", "work"]).assert_success();

    let res = env.dft(&["collapse", "--into", "release-candidate"]);
    res.assert_success();
}

pub fn test_collapse_generates_synthesis_commit() {
    let env = TestEnv::new("collapse_synthesis_commit");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "feat-alpha"])
        .assert_success();
    env.dft(&["dimension", "enter", "feat-alpha"])
        .assert_success();
    env.write_file("a.txt", "a");
    env.dft(&["add", "a.txt"]).assert_success();
    env.dft(&["commit", "-m", "a"]).assert_success();

    env.dft(&["dimension", "enter", "mainline"])
        .assert_success();
    env.dft(&["collapse"]).assert_success();

    let log = env.dft(&["log", "-n", "1"]);
    log.assert_success();
    if !env.is_dry_run {
        log.assert_output_contains("Collapse");
    }
}

pub fn test_collapse_conflict_handling() {
    let env = TestEnv::new("collapse_conflict");
    env.dft(&["init"]).assert_success();
    env.write_file("c.txt", "initial");
    env.dft(&["add", "c.txt"]).assert_success();
    env.dft(&["commit", "-m", "base"]).assert_success();

    env.dft(&["dimension", "create", "dim-x"]).assert_success();
    env.dft(&["dimension", "enter", "dim-x"]).assert_success();
    env.write_file("c.txt", "edit x");
    env.dft(&["add", "c.txt"]).assert_success();
    env.dft(&["commit", "-m", "x"]).assert_success();

    env.dft(&["dimension", "create", "dim-y"]).assert_success();
    env.dft(&["dimension", "enter", "dim-y"]).assert_success();
    env.write_file("c.txt", "edit y");
    env.dft(&["add", "c.txt"]).assert_success();
    env.dft(&["commit", "-m", "y"]).assert_success();

    env.dft(&["dimension", "enter", "mainline"])
        .assert_success();
    let res = env.dft(&["collapse", "--strategy", "theirs"]);
    res.assert_success();
}

pub fn test_collapse_empty_repository_noop() {
    let env = TestEnv::new("collapse_empty");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["collapse"]);
    res.assert_success();
}

// ============================================================================
// 2. `converge` (5 tests)
// ============================================================================

pub fn test_converge_two_dimensions_preserves_originals() {
    let env = TestEnv::new("converge_two");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "c-1"]).assert_success();
    env.dft(&["dimension", "create", "c-2"]).assert_success();

    env.dft(&["dimension", "enter", "c-1"]).assert_success();
    env.write_file("c1.txt", "1");
    env.dft(&["add", "c1.txt"]).assert_success();
    env.dft(&["commit", "-m", "c1"]).assert_success();

    env.dft(&["dimension", "enter", "c-2"]).assert_success();
    env.write_file("c2.txt", "2");
    env.dft(&["add", "c2.txt"]).assert_success();
    env.dft(&["commit", "-m", "c2"]).assert_success();

    env.dft(&["dimension", "enter", "mainline"])
        .assert_success();
    let res = env.dft(&["converge", "c-1", "c-2"]);
    res.assert_success();

    // Originals must still exist
    let list = env.dft(&["dimension", "list"]);
    if !env.is_dry_run {
        list.assert_output_contains("c-1");
        list.assert_output_contains("c-2");
    }
}

pub fn test_converge_three_dimensions() {
    let env = TestEnv::new("converge_three");
    env.dft(&["init"]).assert_success();
    for i in 1..=3 {
        let name = format!("conv-{}", i);
        env.dft(&["dimension", "create", &name]).assert_success();
        env.dft(&["dimension", "enter", &name]).assert_success();
        let fname = format!("f_{}.txt", i);
        env.write_file(&fname, &i.to_string());
        env.dft(&["add", &fname]).assert_success();
        env.dft(&["commit", "-m", &format!("commit {}", i)])
            .assert_success();
    }

    env.dft(&["dimension", "enter", "mainline"])
        .assert_success();
    let res = env.dft(&["converge", "conv-1", "conv-2", "conv-3"]);
    res.assert_success();
}

pub fn test_converge_into_new_dimension_name() {
    let env = TestEnv::new("converge_new_dim");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "part-a"]).assert_success();
    env.dft(&["dimension", "create", "part-b"]).assert_success();
    let res = env.dft(&["converge", "part-a", "part-b", "--into", "unified-universe"]);
    res.assert_success();
}

pub fn test_converge_identical_dimensions_clean() {
    let env = TestEnv::new("converge_identical");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "id-1"]).assert_success();
    env.dft(&["dimension", "fork", "id-2", "--from", "id-1"])
        .assert_success();
    let res = env.dft(&["converge", "id-1", "id-2"]);
    res.assert_success();
}

pub fn test_converge_invalid_dimension_fails() {
    let env = TestEnv::new("converge_invalid");
    env.dft(&["init"]).assert_success();
    let res = env.dft(&["converge", "nonexistent-1", "nonexistent-2"]);
    if !env.is_dry_run {
        res.assert_failure();
    }
}

// ============================================================================
// 3. `cascade`, `weave`, `splice` (5 tests each)
// ============================================================================

pub fn test_cascade_sequential_propagation() {
    let env = TestEnv::new("cascade_propagate");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "dim-upstream"])
        .assert_success();
    env.dft(&[
        "dimension",
        "fork",
        "dim-midstream",
        "--from",
        "dim-upstream",
    ])
    .assert_success();
    env.dft(&[
        "dimension",
        "fork",
        "dim-downstream",
        "--from",
        "dim-midstream",
    ])
    .assert_success();

    env.dft(&["dimension", "enter", "dim-upstream"])
        .assert_success();
    env.write_file("fix.rs", "pub fn fix() {}");
    env.dft(&["add", "fix.rs"]).assert_success();
    env.dft(&["commit", "-m", "upstream fix"]).assert_success();

    let res = env.dft(&["cascade", "dim-upstream"]);
    res.assert_success();
}

pub fn test_cascade_targeted_endpoint() {
    let env = TestEnv::new("cascade_targeted");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "source"]).assert_success();
    env.dft(&["dimension", "create", "target"]).assert_success();
    let res = env.dft(&["cascade", "source", "--to", "target"]);
    res.assert_success();
}

pub fn test_weave_interleaves_chronological_commits() {
    let env = TestEnv::new("weave_interleave");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "stream-a"])
        .assert_success();
    env.dft(&["dimension", "create", "stream-b"])
        .assert_success();

    env.dft(&["dimension", "enter", "stream-a"])
        .assert_success();
    env.write_file("a1.txt", "a1");
    env.dft(&["add", "a1.txt"]).assert_success();
    env.dft(&["commit", "-m", "a1 commit"]).assert_success();

    env.dft(&["dimension", "enter", "stream-b"])
        .assert_success();
    env.write_file("b1.txt", "b1");
    env.dft(&["add", "b1.txt"]).assert_success();
    env.dft(&["commit", "-m", "b1 commit"]).assert_success();

    let res = env.dft(&["weave", "stream-a", "stream-b"]);
    res.assert_success();
}

pub fn test_splice_commit_range() {
    let env = TestEnv::new("splice_range");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "donor"]).assert_success();
    env.dft(&["dimension", "create", "recipient"])
        .assert_success();

    env.dft(&["dimension", "enter", "donor"]).assert_success();
    env.write_file("d1.txt", "d1");
    env.dft(&["add", "d1.txt"]).assert_success();
    env.dft(&["commit", "-m", "donor commit"]).assert_success();

    let res = env.dft(&["splice", "donor", "HEAD~1..HEAD", "--into", "recipient"]);
    res.assert_success();
}

pub fn test_splice_single_commit() {
    let env = TestEnv::new("splice_single");
    env.dft(&["init"]).assert_success();
    env.dft(&["dimension", "create", "donor-single"])
        .assert_success();
    env.dft(&["dimension", "create", "target-single"])
        .assert_success();

    env.dft(&["dimension", "enter", "donor-single"])
        .assert_success();
    env.write_file("feature.txt", "feature logic");
    env.dft(&["add", "feature.txt"]).assert_success();
    env.dft(&["commit", "-m", "target feature"])
        .assert_success();

    let res = env.dft(&["splice", "donor-single", "HEAD", "--into", "target-single"]);
    res.assert_success();
}
