//! Daft E2E Master Test Runner and Metric Aggregator.
//! Executes and reports test results across Tiers 1 through 4.

pub mod common;
pub mod tier1_features;
pub mod tier2_bounds;
pub mod tier3_pairwise;
pub mod tier4_workload;

use std::env;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;

struct RunnerConfig {
    tier: Option<u8>,
    filter: Option<String>,
    verbose: bool,
    verify_only: bool,
    dft_bin: Option<String>,
}

#[derive(Default)]
struct TierStats {
    total: usize,
    passed: usize,
    failed: usize,
    skipped: usize,
    failures: Vec<(String, String)>,
}

fn parse_args() -> RunnerConfig {
    let mut config = RunnerConfig {
        tier: None,
        filter: None,
        verbose: false,
        verify_only: false,
        dft_bin: None,
    };

    let args: Vec<String> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--tier" => {
                if i + 1 < args.len() {
                    i += 1;
                    if let Ok(t) = args[i].parse::<u8>() {
                        config.tier = Some(t);
                    }
                }
            }
            "--filter" => {
                if i + 1 < args.len() {
                    i += 1;
                    config.filter = Some(args[i].clone());
                }
            }
            "--verbose" | "-v" => {
                config.verbose = true;
            }
            "--verify" | "--dry-run" => {
                config.verify_only = true;
            }
            "--dft-bin" => {
                if i + 1 < args.len() {
                    i += 1;
                    config.dft_bin = Some(args[i].clone());
                }
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }

    if let Ok(filter) = env::var("DFT_TEST_FILTER") {
        if config.filter.is_none() {
            config.filter = Some(filter);
        }
    }
    if env::var("DFT_TEST_VERBOSE").is_ok() {
        config.verbose = true;
    }

    config
}

fn print_help() {
    println!("Draft E2E Test Runner Harness");
    println!("Usage: runner [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --tier <1|2|3|4>    Execute only the specified tier");
    println!("  --filter <pattern>  Run only tests matching the pattern");
    println!("  --dft-bin <path>    Path to 'dft' executable (overrides DFT_BIN)");
    println!("  --verify, --dry-run Verify test suite definitions without spawning CLI");
    println!("  --verbose, -v       Show verbose test progress and outputs");
    println!("  --help, -h          Display this help message");
}

fn run_tier_tests(
    tier_num: u8,
    tier_name: &str,
    tests: Vec<(&'static str, fn())>,
    config: &RunnerConfig,
) -> TierStats {
    let mut stats = TierStats::default();

    if let Some(target_tier) = config.tier {
        if target_tier != tier_num {
            return stats;
        }
    }

    println!("\n▶ Running Tier {}: {}", tier_num, tier_name);
    println!("--------------------------------------------------------------------------------");

    for (test_name, test_fn) in tests {
        if let Some(ref filter) = config.filter {
            if !test_name.contains(filter) {
                stats.skipped += 1;
                continue;
            }
        }

        stats.total += 1;
        if config.verbose {
            print!("  Running {} ... ", test_name);
        }

        let start = Instant::now();
        let result = catch_unwind(AssertUnwindSafe(|| {
            test_fn();
        }));

        let elapsed = start.elapsed();

        match result {
            Ok(_) => {
                stats.passed += 1;
                if config.verbose {
                    println!("PASSED ({:.2?})", elapsed);
                } else {
                    print!(".");
                }
            }
            Err(payload) => {
                stats.failed += 1;
                let error_msg = if let Some(s) = payload.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = payload.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Test panicked with unknown payload".to_string()
                };

                if config.verbose {
                    println!("FAILED ({:.2?})\n    Error: {}", elapsed, error_msg);
                } else {
                    print!("F");
                }
                stats.failures.push((test_name.to_string(), error_msg));
            }
        }
    }

    if !config.verbose && stats.total > 0 {
        println!();
    }

    stats
}

fn main() {
    let start_time = Instant::now();
    let config = parse_args();

    if let Some(bin) = &config.dft_bin {
        env::set_var("DFT_BIN", bin);
    }
    if config.verify_only {
        env::set_var("DFT_DRY_RUN", "1");
    }

    println!("================================================================================");
    println!("                    DAFT (`dft`) E2E TEST RUNNER HARNESS                        ");
    println!("================================================================================");
    println!(
        "Mode: {}",
        if config.verify_only || env::var("DFT_DRY_RUN").is_ok() {
            "Dry-run / Verification"
        } else {
            "Subprocess CLI Execution"
        }
    );
    if let Ok(bin) = env::var("DFT_BIN") {
        println!("Binary: {}", bin);
    }

    // Collect tests across all 4 tiers
    let t1_tests: Vec<(&'static str, fn())> = tier1_features::get_all_tests()
        .into_iter()
        .map(|t| (t.name, t.func))
        .collect();

    let t2_tests: Vec<(&'static str, fn())> = tier2_bounds::get_all_tests()
        .into_iter()
        .map(|t| (t.name, t.func))
        .collect();

    let t3_tests: Vec<(&'static str, fn())> = tier3_pairwise::get_all_tests()
        .into_iter()
        .map(|t| (t.name, t.func))
        .collect();

    let t4_tests: Vec<(&'static str, fn())> = tier4_workload::get_all_tests()
        .into_iter()
        .map(|t| (t.name, t.func))
        .collect();

    let s1 = run_tier_tests(1, "Feature Coverage", t1_tests, &config);
    let s2 = run_tier_tests(2, "Boundary & Corner Cases", t2_tests, &config);
    let s3 = run_tier_tests(3, "Pairwise Cross-Feature", t3_tests, &config);
    let s4 = run_tier_tests(4, "Multi-Agent Workload Scenarios", t4_tests, &config);

    let total_executed = s1.total + s2.total + s3.total + s4.total;
    let total_passed = s1.passed + s2.passed + s3.passed + s4.passed;
    let total_failed = s1.failed + s2.failed + s3.failed + s4.failed;
    let total_skipped = s1.skipped + s2.skipped + s3.skipped + s4.skipped;

    println!("\n================================================================================");
    println!("                            TEST EXECUTION SUMMARY                              ");
    println!("================================================================================");
    println!(
        "  Tier 1: Feature Coverage ................ {:>3} passed, {:>3} failed, {:>3} skipped",
        s1.passed, s1.failed, s1.skipped
    );
    println!(
        "  Tier 2: Boundary & Corner Cases ......... {:>3} passed, {:>3} failed, {:>3} skipped",
        s2.passed, s2.failed, s2.skipped
    );
    println!(
        "  Tier 3: Pairwise Cross-Feature .......... {:>3} passed, {:>3} failed, {:>3} skipped",
        s3.passed, s3.failed, s3.skipped
    );
    println!(
        "  Tier 4: Multi-Agent Workloads ........... {:>3} passed, {:>3} failed, {:>3} skipped",
        s4.passed, s4.failed, s4.skipped
    );
    println!("--------------------------------------------------------------------------------");

    let pass_rate = if total_executed > 0 {
        (total_passed as f64 / total_executed as f64) * 100.0
    } else {
        100.0
    };

    println!(
        "  TOTAL: {} / {} Passed ({:.1}% Pass Rate) | Failed: {} | Skipped: {}",
        total_passed, total_executed, pass_rate, total_failed, total_skipped
    );
    println!("  DURATION: {:.2?}", start_time.elapsed());

    let all_failures: Vec<(String, String)> =
        [s1.failures, s2.failures, s3.failures, s4.failures].concat();
    if !all_failures.is_empty() {
        println!("\n❌ FAILURES ({}):", all_failures.len());
        for (name, err) in &all_failures {
            println!("  - {}:\n      {}", name, err);
        }
        println!("\nSTATUS: TEST SUITE FAILED");
        std::process::exit(1);
    } else {
        println!("  STATUS: ALL TESTS PASSED — READY FOR MILESTONE ACCEPTANCE");
        println!(
            "================================================================================"
        );
        std::process::exit(0);
    }
}
