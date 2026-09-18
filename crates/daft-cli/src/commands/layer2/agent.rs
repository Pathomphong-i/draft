use crate::cli::AgentArgs;
use crate::error::CliError;
use crate::output::print_output;
use daft_agent::{AgentManager, AgentType};
use daft_core::Repository;
use std::env;
use std::sync::Arc;

pub fn execute(args: AgentArgs, json: bool) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Arc::new(Repository::discover(&cwd)?);
    let manager = AgentManager::new(Arc::clone(&repo));

    let cmd = args.args.first().map(|s| s.as_str()).unwrap_or("list");

    match cmd {
        "register" => {
            let mut raw_type = args.agent_type.clone();
            let mut name_opt = None;
            let mut i = 1;
            while i < args.args.len() {
                let token = &args.args[i];
                if token == "--type" || token == "-t" {
                    if i + 1 < args.args.len() {
                        if raw_type.is_none() {
                            raw_type = Some(args.args[i + 1].clone());
                        }
                        i += 2;
                        continue;
                    }
                } else if token.starts_with("--type=") {
                    if raw_type.is_none() {
                        raw_type = Some(token["--type=".len()..].to_string());
                    }
                    i += 1;
                    continue;
                } else if token.starts_with("-t=") {
                    if raw_type.is_none() {
                        raw_type = Some(token["-t=".len()..].to_string());
                    }
                    i += 1;
                    continue;
                } else if token == "--json" {
                    i += 1;
                    continue;
                } else if !token.starts_with('-') {
                    if name_opt.is_none() {
                        name_opt = Some(token.clone());
                    }
                }
                i += 1;
            }

            let name = match name_opt {
                Some(n) => n,
                None => return Err(CliError::General("Missing agent name".into())),
            };

            let agent_type: AgentType = raw_type
                .as_deref()
                .unwrap_or("ai")
                .parse()
                .map_err(CliError::General)?;

            let identity = manager
                .register(&name, agent_type, Vec::new())
                .map_err(|e| CliError::General(e.to_string()))?;

            if json {
                print_output(true, &identity, "");
            } else {
                println!("Registered agent '{}'", identity.name);
            }
        }

        "list" => {
            let list = manager
                .list()
                .map_err(|e| CliError::General(e.to_string()))?;
            if json {
                print_output(true, &list, "");
            } else {
                for a in list {
                    let dim = a.assigned_dimension.as_deref().unwrap_or("mainline");
                    let type_str = match a.agent_type {
                        AgentType::Human => "human",
                        AgentType::Ai => "ai",
                    };
                    println!("{} [{}] ({})", a.name, type_str, dim);
                }
            }
        }

        "assign" => {
            let agent_name = match args.args.get(1) {
                Some(n) => n,
                None => {
                    return Err(CliError::General(
                        "Usage: dft agent assign <agent> <dim>".into(),
                    ))
                }
            };
            let dim = match args.args.get(2) {
                Some(d) => d,
                None => {
                    return Err(CliError::General(
                        "Usage: dft agent assign <agent> <dim>".into(),
                    ))
                }
            };

            manager
                .assign(agent_name, dim)
                .map_err(|e| CliError::General(e.to_string()))?;
            println!("Assigned agent '{}' to dimension '{}'", agent_name, dim);
        }

        "status" => {
            let status = manager
                .status(None)
                .map_err(|e| CliError::General(e.to_string()))?;

            if json {
                print_output(true, &status, "");
            } else {
                println!("Agent subsystem status: operational");
                println!(
                    "Total agents: {} ({} active, {} idle, {} offline)",
                    status.total_agents,
                    status.active_agents,
                    status.idle_agents,
                    status.offline_agents
                );
                for a in &status.agents {
                    let type_str = match a.agent_type {
                        AgentType::Human => "human",
                        AgentType::Ai => "ai",
                    };
                    println!("  {} [{}] -> {}", a.name, type_str, a.assigned_dimension);
                }
            }
        }

        "broadcast" => {
            let msg = if args.args.len() > 1 {
                args.args[1..].join(" ")
            } else {
                "system broadcast".to_string()
            };
            let sender = env::var("DFT_AGENT_NAME").unwrap_or_else(|_| "system".to_string());
            manager
                .broadcast(&sender, "broadcast", &msg)
                .map_err(|e| CliError::General(e.to_string()))?;
            println!("Broadcast sent to all agents");
        }

        "inbox" => {
            let is_json = json || args.args.iter().any(|a| a == "--json");
            let agent = args
                .args
                .get(1)
                .filter(|s| !s.starts_with('-'))
                .map(|s| s.as_str());

            let msgs = manager
                .read_inbox(agent, false)
                .map_err(|e| CliError::General(e.to_string()))?;

            if is_json {
                print_output(true, &msgs, "");
            } else {
                for m in msgs {
                    println!("[{}] {}: {}", m.timestamp_iso, m.sender, m.body);
                }
            }
        }

        "heartbeat" => {
            let agent = args.args.get(1).map(|s| s.as_str());
            execute_heartbeat(agent, json)?;
        }

        other => {
            return Err(CliError::General(format!(
                "Unknown agent subcommand: {}",
                other
            )));
        }
    }

    Ok(())
}

pub fn execute_heartbeat(agent: Option<&str>, json: bool) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Arc::new(Repository::discover(&cwd)?);
    let manager = AgentManager::new(Arc::clone(&repo));

    let env_name = env::var("DFT_AGENT_NAME").ok();
    let agent_name = agent.or(env_name.as_deref()).unwrap_or("default");

    let record = manager
        .heartbeat(agent_name, None, None)
        .map_err(|e| CliError::General(e.to_string()))?;

    if json {
        print_output(true, &record, "");
    } else {
        println!("Heartbeat recorded for agent '{}'", agent_name);
    }

    Ok(())
}
