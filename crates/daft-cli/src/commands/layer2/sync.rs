use crate::cli::{CronosArgs, EntangleArgs};
use crate::error::CliError;
use crate::output::print_output;
use daft_core::Repository;
use daft_sync::{CronosDaemon, EntangleDirection, EntangleEngine, SyncStrategy, WatchedDimension};
use std::env;
use std::sync::Arc;
use std::time::Duration;

pub fn execute_entangle(args: EntangleArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Arc::new(Repository::discover(&cwd)?);
    let engine = EntangleEngine::new(Arc::clone(&repo));

    let mut paths = args.paths.clone();
    let mut clean_args = Vec::new();
    let is_json = args.args.iter().any(|a| a == "--json");
    let mut i = 0;
    while i < args.args.len() {
        if args.args[i] == "--paths" {
            if i + 1 < args.args.len() {
                paths = Some(args.args[i + 1].clone());
                i += 2;
                continue;
            }
        } else if args.args[i].starts_with("--paths=") {
            paths = Some(args.args[i]["--paths=".len()..].to_string());
            i += 1;
            continue;
        } else if args.args[i] == "--json" {
            i += 1;
            continue;
        }
        clean_args.push(args.args[i].clone());
        i += 1;
    }

    if clean_args.is_empty() {
        return Err(CliError::General(
            "Usage: dft entangle [link] <dim1> <dim2> | sever <dim1> <dim2> | list | log | sync"
                .into(),
        ));
    }

    match clean_args[0].as_str() {
        "link" => {
            if clean_args.len() < 3 {
                return Err(CliError::General(
                    "Usage: dft entangle link <dim1> <dim2> [--paths <glob>]".into(),
                ));
            }
            let d1 = &clean_args[1];
            let d2 = &clean_args[2];

            let rule = engine
                .link(d1, d2, paths.as_deref(), EntangleDirection::Bidirectional)
                .map_err(|e| match e {
                    daft_sync::SyncError::DimensionNotFound(d) => CliError::DimensionNotFound(d),
                    other => CliError::General(other.to_string()),
                })?;

            let _ = engine.propagate_rule(&rule.id);
            println!("Entangled '{}' with '{}'", d1, d2);
        }
        "sever" | "break" => {
            if clean_args.len() < 3 {
                return Err(CliError::General(
                    "Usage: dft entangle sever <dim1> <dim2>".into(),
                ));
            }
            let d1 = &clean_args[1];
            let d2 = &clean_args[2];
            engine
                .sever(d1, d2)
                .map_err(|e| CliError::General(e.to_string()))?;
            println!("Severed entanglement between '{}' and '{}'", d1, d2);
        }
        "list" => {
            let rules = engine
                .list()
                .map_err(|e| CliError::General(e.to_string()))?;
            if is_json {
                print_output(true, &rules, "");
            } else {
                println!("Active Entanglements:");
                for r in &rules {
                    if let Some(ref p) = r.paths {
                        println!("  {} <-> {} (paths: {})", r.dim1, r.dim2, p);
                    } else {
                        println!("  {} <-> {}", r.dim1, r.dim2);
                    }
                }
            }
        }
        "log" => {
            let events = engine
                .read_log(None)
                .map_err(|e| CliError::General(e.to_string()))?;
            if is_json {
                print_output(true, &events, "");
            } else if events.is_empty() {
                println!("Entanglement Event Log: clean");
            } else {
                println!("=== Entanglement Event Log ===");
                for ev in events {
                    println!(
                        "[{}] {} -> {}: {} ({})",
                        ev.timestamp.to_rfc3339(),
                        ev.source_dim,
                        ev.target_dim,
                        ev.path,
                        ev.status
                    );
                }
            }
        }
        "sync" => {
            let results = engine
                .propagate_all()
                .map_err(|e| CliError::General(e.to_string()))?;
            let mut total_files = 0;
            for r in results {
                total_files += r.files_propagated.len();
            }
            println!("Entangled sync complete: {} files updated", total_files);
        }
        d1 => {
            if d1.starts_with('-') {
                return Err(CliError::General(format!(
                    "Unknown entangle option: '{}'",
                    d1
                )));
            }
            if clean_args.len() < 2 {
                return Err(CliError::General(
                    "Usage: dft entangle [link] <dim1> <dim2> | sever <dim1> <dim2> | list | log | sync".into(),
                ));
            }
            let d2 = &clean_args[1];

            let rule = engine
                .link(d1, d2, paths.as_deref(), EntangleDirection::Bidirectional)
                .map_err(|e| match e {
                    daft_sync::SyncError::DimensionNotFound(d) => CliError::DimensionNotFound(d),
                    other => CliError::General(other.to_string()),
                })?;

            let _ = engine.propagate_rule(&rule.id);
            println!("Entangled '{}' with '{}'", d1, d2);
        }
    }

    Ok(())
}

pub fn execute_cronos(args: CronosArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Arc::new(Repository::discover(&cwd)?);
    let daemon = CronosDaemon::new(Arc::clone(&repo));

    let cmd = args.args.first().map(|s| s.as_str()).unwrap_or("status");

    match cmd {
        "start" => {
            if daemon.is_running() {
                let pid = daemon.get_running_pid().unwrap_or(0);
                return Err(CliError::General(format!(
                    "Cronos daemon already running (PID {})",
                    pid
                )));
            }

            let mut interval_sec = 30u64;
            let mut strategy = SyncStrategy::Merge;

            let mut i = 1;
            while i < args.args.len() {
                if args.args[i] == "--interval" && i + 1 < args.args.len() {
                    let raw = &args.args[i + 1];
                    let trimmed = raw.trim_end_matches('s').trim_end_matches('m');
                    if let Ok(num) = trimmed.parse::<u64>() {
                        interval_sec = if raw.ends_with('m') { num * 60 } else { num };
                    }
                    i += 2;
                } else if args.args[i].starts_with("--interval=") {
                    let raw = &args.args[i]["--interval=".len()..];
                    let trimmed = raw.trim_end_matches('s').trim_end_matches('m');
                    if let Ok(num) = trimmed.parse::<u64>() {
                        interval_sec = if raw.ends_with('m') { num * 60 } else { num };
                    }
                    i += 1;
                } else if args.args[i] == "--strategy" && i + 1 < args.args.len() {
                    if let Ok(st) = args.args[i + 1].parse::<SyncStrategy>() {
                        strategy = st;
                    }
                    i += 2;
                } else if args.args[i].starts_with("--strategy=") {
                    let raw = &args.args[i]["--strategy=".len()..];
                    if let Ok(st) = raw.parse::<SyncStrategy>() {
                        strategy = st;
                    }
                    i += 1;
                } else {
                    i += 1;
                }
            }

            let current_exe = std::env::current_exe()?;
            let mut cmd = std::process::Command::new(current_exe);
            cmd.arg("cronos")
                .arg("_worker")
                .arg("--interval")
                .arg(format!("{}s", interval_sec))
                .arg("--strategy")
                .arg(format!("{:?}", strategy).to_lowercase());
            cmd.current_dir(&cwd);
            cmd.stdin(std::process::Stdio::null());
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());
            let _child = cmd.spawn()?;

            // Allow worker to acquire kernel lock and write PID
            std::thread::sleep(Duration::from_millis(50));
            println!("Cronos background daemon started");
        }

        "_worker" => {
            let mut interval_sec = 30u64;
            let mut strategy = SyncStrategy::Merge;

            let mut i = 1;
            while i < args.args.len() {
                if args.args[i] == "--interval" && i + 1 < args.args.len() {
                    let raw = &args.args[i + 1];
                    let trimmed = raw.trim_end_matches('s').trim_end_matches('m');
                    if let Ok(num) = trimmed.parse::<u64>() {
                        interval_sec = if raw.ends_with('m') { num * 60 } else { num };
                    }
                    i += 2;
                } else if args.args[i] == "--strategy" && i + 1 < args.args.len() {
                    if let Ok(st) = args.args[i + 1].parse::<SyncStrategy>() {
                        strategy = st;
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }

            if let Err(e) = daemon.start(Some(Duration::from_secs(interval_sec)), Some(strategy)) {
                return Err(CliError::General(e.to_string()));
            }

            let my_pid = std::process::id();
            let step = Duration::from_millis(100);
            let mut elapsed = Duration::from_secs(0);
            let interval = Duration::from_secs(interval_sec.max(1));

            loop {
                std::thread::sleep(step);
                elapsed += step;
                if daemon.get_running_pid() != Some(my_pid) {
                    break;
                }
                if elapsed >= interval {
                    let _ = daemon.run_tick();
                    elapsed = Duration::from_secs(0);
                }
            }
        }

        "status" => {
            let status = daemon
                .status()
                .map_err(|e| CliError::General(e.to_string()))?;
            if status.is_running {
                println!("Cronos status: running");
                if let Some(pid) = status.pid {
                    println!("  Daemon PID: {}", pid);
                }
                println!("  Interval: {}s", status.interval_secs);
                println!("  Default Strategy: {:?}", status.default_strategy);
                if let Some(next) = status.next_sync_time {
                    println!("  Next Sync: {}", next.to_rfc3339());
                }
            } else {
                println!("Cronos status: stopped");
            }
        }

        "stop" => {
            daemon
                .stop()
                .map_err(|e| CliError::General(e.to_string()))?;
            println!("Cronos background daemon stopped");
        }

        "log" => {
            let entries = daemon
                .read_sync_log(None)
                .map_err(|e| CliError::General(e.to_string()))?;
            if entries.is_empty() {
                println!("Cronos WAL Sync Log: verified intact");
            } else {
                println!("=== Cronos Sync & Conflict Log ===");
                for entry in entries {
                    println!(
                        "[{}] [{}] {} -> {}: {} ({})",
                        entry.timestamp.to_rfc3339(),
                        entry.level,
                        entry.source_dim,
                        entry.target_dim,
                        entry.action,
                        entry.message
                    );
                }
            }
        }

        "pause" => {
            let dim = args.args.get(1).map(|s| s.as_str()).unwrap_or("unknown");
            daemon
                .pause(dim)
                .map_err(|e| CliError::General(e.to_string()))?;
            println!("Paused Cronos sync for dimension '{}'", dim);
        }

        "resume" => {
            let dim = args.args.get(1).map(|s| s.as_str()).unwrap_or("unknown");
            daemon
                .resume(dim)
                .map_err(|e| CliError::General(e.to_string()))?;
            println!("Resumed Cronos sync for dimension '{}'", dim);
        }

        "config" => {
            let mut config = daemon
                .get_config()
                .map_err(|e| CliError::General(e.to_string()))?;
            let is_json = args.args.iter().any(|a| a == "--json");

            let mut strategy_opt = None;
            let mut interval_opt = None;
            let mut watch_dim_opt = None;
            let mut target_dim_opt = None;
            let mut paths_vec = Vec::new();
            let mut unwatch_dim_opt = None;
            let mut reset_watch = false;
            let mut auto_entangle_opt = None;
            let mut explicit_list = false;

            let sub_args = &args.args[1..];
            let mut i = 0;
            while i < sub_args.len() {
                let token = &sub_args[i];
                if token == "--strategy" && i + 1 < sub_args.len() {
                    let st = sub_args[i + 1]
                        .parse::<SyncStrategy>()
                        .map_err(CliError::General)?;
                    strategy_opt = Some(st);
                    i += 2;
                } else if token.starts_with("--strategy=") {
                    let raw = &token["--strategy=".len()..];
                    let st = raw.parse::<SyncStrategy>().map_err(CliError::General)?;
                    strategy_opt = Some(st);
                    i += 1;
                } else if token == "--interval" && i + 1 < sub_args.len() {
                    let raw = &sub_args[i + 1];
                    let trimmed = raw.trim_end_matches('s').trim_end_matches('m');
                    let num = trimmed.parse::<u64>().map_err(|_| {
                        CliError::General(format!("Invalid interval duration: '{}'", raw))
                    })?;
                    interval_opt = Some(if raw.ends_with('m') { num * 60 } else { num });
                    i += 2;
                } else if token.starts_with("--interval=") {
                    let raw = &token["--interval=".len()..];
                    let trimmed = raw.trim_end_matches('s').trim_end_matches('m');
                    let num = trimmed.parse::<u64>().map_err(|_| {
                        CliError::General(format!("Invalid interval duration: '{}'", raw))
                    })?;
                    interval_opt = Some(if raw.ends_with('m') { num * 60 } else { num });
                    i += 1;
                } else if token == "--watch" && i + 1 < sub_args.len() {
                    watch_dim_opt = Some(sub_args[i + 1].clone());
                    i += 2;
                } else if token.starts_with("--watch=") {
                    watch_dim_opt = Some(token["--watch=".len()..].to_string());
                    i += 1;
                } else if token == "--target" && i + 1 < sub_args.len() {
                    target_dim_opt = Some(sub_args[i + 1].clone());
                    i += 2;
                } else if token.starts_with("--target=") {
                    target_dim_opt = Some(token["--target=".len()..].to_string());
                    i += 1;
                } else if token == "--paths" && i + 1 < sub_args.len() {
                    paths_vec.push(sub_args[i + 1].clone());
                    i += 2;
                } else if token.starts_with("--paths=") {
                    paths_vec.push(token["--paths=".len()..].to_string());
                    i += 1;
                } else if token == "--unwatch" && i + 1 < sub_args.len() {
                    unwatch_dim_opt = Some(sub_args[i + 1].clone());
                    i += 2;
                } else if token.starts_with("--unwatch=") {
                    unwatch_dim_opt = Some(token["--unwatch=".len()..].to_string());
                    i += 1;
                } else if token == "--reset-watch" || token == "--clear-watch" || token == "--clear"
                {
                    reset_watch = true;
                    i += 1;
                } else if token == "--auto-entangle" {
                    if i + 1 < sub_args.len()
                        && (sub_args[i + 1] == "true" || sub_args[i + 1] == "false")
                    {
                        auto_entangle_opt = Some(sub_args[i + 1] == "true");
                        i += 2;
                    } else {
                        auto_entangle_opt = Some(true);
                        i += 1;
                    }
                } else if token == "--no-auto-entangle" {
                    auto_entangle_opt = Some(false);
                    i += 1;
                } else if token.starts_with("--auto-entangle=") {
                    let val = &token["--auto-entangle=".len()..];
                    auto_entangle_opt = Some(val == "true" || val == "1");
                    i += 1;
                } else if token == "--json" {
                    i += 1;
                } else if token == "list" || token == "show" {
                    explicit_list = true;
                    i += 1;
                } else if token == "watch" && i + 1 < sub_args.len() {
                    watch_dim_opt = Some(sub_args[i + 1].clone());
                    i += 2;
                    if i < sub_args.len() && !sub_args[i].starts_with('-') {
                        target_dim_opt = Some(sub_args[i].clone());
                        i += 1;
                    }
                } else if token == "unwatch" && i + 1 < sub_args.len() {
                    unwatch_dim_opt = Some(sub_args[i + 1].clone());
                    i += 2;
                } else if token == "reset" {
                    reset_watch = true;
                    i += 1;
                } else {
                    return Err(CliError::General(format!(
                        "Unknown config option: '{}'",
                        token
                    )));
                }
            }

            let has_updates = strategy_opt.is_some()
                || interval_opt.is_some()
                || watch_dim_opt.is_some()
                || unwatch_dim_opt.is_some()
                || reset_watch
                || auto_entangle_opt.is_some();

            if !has_updates || explicit_list {
                if is_json {
                    print_output(true, &config, "");
                } else {
                    println!("Cronos Configuration:");
                    println!("  Interval: {}s", config.interval_secs);
                    println!("  Default Strategy: {:?}", config.default_strategy);
                    println!("  Auto-Entangle: {}", config.auto_entangle);
                    if config.watched_dimensions.is_empty() {
                        println!("  Watched Dimensions: none");
                    } else {
                        println!(
                            "  Watched Dimensions ({}):",
                            config.watched_dimensions.len()
                        );
                        for w in &config.watched_dimensions {
                            let strat_str = w
                                .strategy
                                .map(|s| format!("{:?}", s).to_lowercase())
                                .unwrap_or_else(|| "default".to_string());
                            let interval_str = w
                                .interval_secs
                                .map(|sec| format!("{}s", sec))
                                .unwrap_or_else(|| "default".to_string());
                            let paths_str = if w.paths.is_empty() {
                                "all".to_string()
                            } else {
                                w.paths.join(", ")
                            };
                            let paused_str = if w.paused { " [paused]" } else { "" };
                            println!(
                                "    {} -> {} (strategy: {}, interval: {}, paths: {}){}",
                                w.dimension,
                                w.target,
                                strat_str,
                                interval_str,
                                paths_str,
                                paused_str
                            );
                        }
                    }
                }
                return Ok(());
            }

            // Apply updates
            if reset_watch {
                config.watched_dimensions.clear();
            }

            if let Some(dim) = unwatch_dim_opt {
                config.watched_dimensions.retain(|w| w.dimension != dim);
            }

            if let Some(dim) = watch_dim_opt {
                let target = target_dim_opt.unwrap_or_else(|| "mainline".to_string());
                if let Some(existing) = config
                    .watched_dimensions
                    .iter_mut()
                    .find(|w| w.dimension == dim)
                {
                    existing.target = target;
                    if let Some(s) = strategy_opt {
                        existing.strategy = Some(s);
                    }
                    if let Some(inv) = interval_opt {
                        existing.interval_secs = Some(inv);
                    }
                    if !paths_vec.is_empty() {
                        existing.paths = paths_vec.clone();
                    }
                } else {
                    config.watched_dimensions.push(WatchedDimension {
                        dimension: dim,
                        target,
                        strategy: strategy_opt,
                        interval_secs: interval_opt,
                        paths: paths_vec.clone(),
                        paused: false,
                        last_synced_at: None,
                        last_synced_commit: None,
                    });
                }
            } else {
                if let Some(s) = strategy_opt {
                    config.default_strategy = s;
                }
                if let Some(inv) = interval_opt {
                    config.interval_secs = inv;
                }
            }

            if let Some(ae) = auto_entangle_opt {
                config.auto_entangle = ae;
            }

            // Persist configuration atomically
            daemon
                .set_config(config.clone())
                .map_err(|e| CliError::General(e.to_string()))?;

            if is_json {
                print_output(true, &config, "");
            } else {
                println!("Cronos config updated");
            }
        }

        "wal" => {
            let count = daemon
                .verify_wal()
                .map_err(|e| CliError::General(e.to_string()))?;
            println!(
                "Cronos WAL verification: 0 errors detected ({} records)",
                count
            );
        }

        "sync" => {
            let tick = daemon
                .run_tick()
                .map_err(|e| CliError::General(e.to_string()))?;
            println!(
                "Cronos sync pass completed: {} dimensions synced, {} entangled files propagated",
                tick.synced_dimensions, tick.entanglements_propagated
            );
        }

        other => {
            return Err(CliError::General(format!(
                "Unknown cronos subcommand: {}",
                other
            )));
        }
    }

    Ok(())
}
