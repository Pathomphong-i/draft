//! Adversarial CLI stress harness for Milestone 5 Layer 2 features.
//! Tests agent register argument permutations, cronos config mutations & persistence,
//! and entangle link/sever subcommand routing without "DimensionNotFound(link)".

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn find_dft_bin() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest_dir.join("../../target/debug/dft"),
        manifest_dir.join("../target/debug/dft"),
        manifest_dir.join("target/debug/dft"),
        PathBuf::from("target/debug/dft"),
    ];

    for c in &candidates {
        if c.exists() {
            return c.canonicalize().unwrap_or_else(|_| c.clone());
        }
    }
    panic!("Could not find dft binary in candidates: {:?}", candidates);
}

struct CliRunner {
    bin: PathBuf,
    repo_dir: PathBuf,
    _temp: TempDir,
}

impl CliRunner {
    fn new() -> Self {
        let temp = TempDir::new().expect("create temp dir");
        let repo_dir = temp.path().to_path_buf();
        let bin = find_dft_bin();

        let runner = Self {
            bin,
            repo_dir,
            _temp: temp,
        };

        let out = runner.run(&["init"]);
        assert!(
            out.status.success(),
            "dft init failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        runner
    }

    fn run(&self, args: &[&str]) -> std::process::Output {
        Command::new(&self.bin)
            .args(args)
            .current_dir(&self.repo_dir)
            .output()
            .unwrap_or_else(|e| panic!("Failed to execute dft with args {:?}: {}", args, e))
    }

    fn run_success(&self, args: &[&str]) -> String {
        let out = self.run(args);
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(
            out.status.success(),
            "Command 'dft {}' failed with status {:?}\nSTDOUT:\n{}\nSTDERR:\n{}",
            args.join(" "),
            out.status.code(),
            stdout,
            stderr
        );
        stdout
    }
}

#[test]
fn test_adversarial_agent_register_argument_placements() {
    let runner = CliRunner::new();

    // 1. Placement: name first, trailing --type human
    let out1 = runner.run_success(&["agent", "register", "linus-human", "--type", "human"]);
    assert!(
        out1.contains("Registered agent 'linus-human'"),
        "Unexpected output: {}",
        out1
    );

    // 2. Placement: leading -t human, then name
    let out2 = runner.run_success(&["agent", "register", "-t", "human", "alan-turing"]);
    assert!(
        out2.contains("Registered agent 'alan-turing'"),
        "Unexpected output: {}",
        out2
    );

    // 3. Placement: --type=human inline, then name
    let out3 = runner.run_success(&["agent", "register", "--type=human", "ada-lovelace"]);
    assert!(
        out3.contains("Registered agent 'ada-lovelace'"),
        "Unexpected output: {}",
        out3
    );

    // 4. Verify in standard list
    let list_out = runner.run_success(&["agent", "list"]);
    assert!(list_out.contains("linus-human [human]"));
    assert!(list_out.contains("alan-turing [human]"));
    assert!(list_out.contains("ada-lovelace [human]"));

    // 5. Verify in structured JSON output
    let json_out = runner.run_success(&["--json", "agent", "list"]);
    let parsed: serde_json::Value = serde_json::from_str(&json_out).expect("Parse agent list JSON");
    let agents = parsed.as_array().expect("Agent list must be JSON array");

    assert_eq!(agents.len(), 3, "Expected 3 registered agents");

    for agent in agents {
        let name = agent["name"].as_str().expect("name field");
        let agent_type = agent["agent_type"].as_str().expect("agent_type field");
        assert_eq!(
            agent_type, "human",
            "Agent '{}' was registered with type '{}', expected 'human'",
            name, agent_type
        );
    }
}

#[test]
fn test_adversarial_cronos_config_mutations_and_persistence() {
    let runner = CliRunner::new();

    // Create dimensions dim-a and dim-b
    runner.run_success(&["dimension", "create", "dim-a"]);
    runner.run_success(&["dimension", "create", "dim-b"]);

    // 1. Initial inspect: verify real configuration output
    let initial_cfg = runner.run_success(&["cronos", "config"]);
    assert!(
        initial_cfg.contains("Cronos Configuration:"),
        "Must output real config header"
    );
    assert!(
        initial_cfg.contains("Interval:"),
        "Must output interval configuration"
    );
    assert!(
        initial_cfg.contains("Default Strategy:"),
        "Must output strategy"
    );

    // 2. Mutate configuration:
    // dft cronos config --strategy rebase --interval 15 --watch dim-a --target dim-b --paths "*.rs"
    let update_out = runner.run_success(&[
        "cronos",
        "config",
        "--strategy",
        "rebase",
        "--interval",
        "15",
        "--watch",
        "dim-a",
        "--target",
        "dim-b",
        "--paths",
        "*.rs",
    ]);
    assert!(
        update_out.contains("Cronos config updated"),
        "Unexpected output on config update: {}",
        update_out
    );

    // 3. Re-run dft cronos config to verify output reflects persisted values
    let re_cfg = runner.run_success(&["cronos", "config"]);
    assert!(
        re_cfg.contains("dim-a -> dim-b"),
        "Config output missing watched dimension mapping: {}",
        re_cfg
    );
    assert!(
        re_cfg.contains("strategy: rebase"),
        "Config output missing rebase strategy: {}",
        re_cfg
    );
    assert!(
        re_cfg.contains("interval: 15s"),
        "Config output missing 15s interval: {}",
        re_cfg
    );
    assert!(
        re_cfg.contains("paths: *.rs"),
        "Config output missing paths *.rs: {}",
        re_cfg
    );

    // 4. Verify disk persistence in .dft/cronos/config.json
    let config_path = runner.repo_dir.join(".dft/cronos/config.json");
    assert!(
        config_path.exists(),
        "config.json must exist at {}",
        config_path.display()
    );

    let config_content = fs::read_to_string(&config_path).expect("read config.json");
    let json_val: serde_json::Value =
        serde_json::from_str(&config_content).expect("parse config.json");

    let watched_arr = json_val["watched_dimensions"]
        .as_array()
        .expect("watched_dimensions array");
    assert_eq!(watched_arr.len(), 1, "Must have 1 watched dimension");

    let entry = &watched_arr[0];
    assert_eq!(entry["dimension"], "dim-a");
    assert_eq!(entry["target"], "dim-b");
    assert_eq!(entry["strategy"], "rebase");
    assert_eq!(entry["interval_secs"], 15);
    assert_eq!(entry["paths"], serde_json::json!(["*.rs"]));
}

#[test]
fn test_adversarial_entangle_link_and_sever_routing() {
    let runner = CliRunner::new();

    // Create dimensions dim1 and dim2
    runner.run_success(&["dimension", "create", "dim1"]);
    runner.run_success(&["dimension", "create", "dim2"]);

    // 1. Run dft entangle link dim1 dim2
    let link_out = runner.run_success(&["entangle", "link", "dim1", "dim2"]);
    assert!(
        link_out.contains("Entangled 'dim1' with 'dim2'"),
        "Link command failed or misrouted: {}",
        link_out
    );

    // Verify it didn't error with DimensionNotFound("link")
    // (If it did, run_success would have panicked above)

    // Verify presence in list
    let list_out = runner.run_success(&["entangle", "list"]);
    assert!(
        list_out.contains("dim1 <-> dim2"),
        "Active entanglements must contain dim1 <-> dim2: {}",
        list_out
    );

    // 2. Run dft entangle sever dim1 dim2
    let sever_out = runner.run_success(&["entangle", "sever", "dim1", "dim2"]);
    assert!(
        sever_out.contains("Severed entanglement between 'dim1' and 'dim2'"),
        "Sever command failed: {}",
        sever_out
    );

    // Verify list no longer contains dim1 <-> dim2
    let list_after = runner.run_success(&["entangle", "list"]);
    assert!(
        !list_after.contains("dim1 <-> dim2"),
        "Entanglement was not removed from active list after sever: {}",
        list_after
    );
}
