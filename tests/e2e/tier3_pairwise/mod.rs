//! Tier 3 Test Suite Registry and Runner.

pub mod pairwise_combinations;

pub struct TestCase {
    pub name: &'static str,
    pub func: fn(),
}

pub fn get_all_tests() -> Vec<TestCase> {
    vec![
        TestCase {
            name: "t3::pairwise_branch_dimension_stash",
            func: pairwise_combinations::test_pairwise_branch_dimension_stash,
        },
        TestCase {
            name: "t3::pairwise_territory_fence_commit_blocking",
            func: pairwise_combinations::test_pairwise_territory_fence_commit_blocking,
        },
        TestCase {
            name: "t3::pairwise_dimension_live_fork_uncommitted_status",
            func: pairwise_combinations::test_pairwise_dimension_live_fork_uncommitted_status,
        },
        TestCase {
            name: "t3::pairwise_entangle_cronos_diff",
            func: pairwise_combinations::test_pairwise_entangle_cronos_diff,
        },
        TestCase {
            name: "t3::pairwise_foresee_converge_validation",
            func: pairwise_combinations::test_pairwise_foresee_converge_validation,
        },
        TestCase {
            name: "t3::pairwise_weave_commits_timeline",
            func: pairwise_combinations::test_pairwise_weave_commits_timeline,
        },
        TestCase {
            name: "t3::pairwise_observe_dirty_workspace",
            func: pairwise_combinations::test_pairwise_observe_dirty_workspace,
        },
        TestCase {
            name: "t3::pairwise_collapse_territory_claims",
            func: pairwise_combinations::test_pairwise_collapse_territory_claims,
        },
        TestCase {
            name: "t3::pairwise_agent_assignment_dimension_context",
            func: pairwise_combinations::test_pairwise_agent_assignment_dimension_context,
        },
        TestCase {
            name: "t3::pairwise_tag_dimension_checkout",
            func: pairwise_combinations::test_pairwise_tag_dimension_checkout,
        },
    ]
}
