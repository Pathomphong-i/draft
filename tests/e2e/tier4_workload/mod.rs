//! Tier 4 Test Suite Registry and Runner.

pub mod multi_agent_scenarios;

pub struct TestCase {
    pub name: &'static str,
    pub func: fn(),
}

pub fn get_all_tests() -> Vec<TestCase> {
    vec![
        TestCase {
            name: "t4::scenario_1_five_agent_parallel_swarm",
            func: multi_agent_scenarios::test_scenario_1_five_agent_parallel_swarm,
        },
        TestCase {
            name: "t4::scenario_2_hot_zone_detection_and_collision_prevention",
            func:
                multi_agent_scenarios::test_scenario_2_hot_zone_detection_and_collision_prevention,
        },
        TestCase {
            name: "t4::scenario_3_autonomous_cronos_synchronization",
            func: multi_agent_scenarios::test_scenario_3_autonomous_cronos_synchronization,
        },
        TestCase {
            name: "t4::scenario_4_multi_universe_convergence_and_collapse",
            func: multi_agent_scenarios::test_scenario_4_multi_universe_convergence_and_collapse,
        },
    ]
}
