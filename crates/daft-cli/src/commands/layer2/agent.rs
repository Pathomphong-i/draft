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
                if (token == "--type" || token == "-t") && i + 1 < args.args.len() {
                    if raw_type.is_none() {
                        raw_type = Some(args.args[i + 1].clone());
                    }
                    i += 2;
                    continue;
                } else if let Some(stripped) = token.strip_prefix("--type=") {
                    if raw_type.is_none() {
                        raw_type = Some(stripped.to_string());
                    }
                    i += 1;
                    continue;
                } else if let Some(stripped) = token.strip_prefix("-t=") {
                    if raw_type.is_none() {
                        raw_type = Some(stripped.to_string());
                    }
                    i += 1;
                    continue;
                } else if token == "--json" {
                    i += 1;
                    continue;
                } else if !token.starts_with('-') && name_opt.is_none() {
                    name_opt = Some(token.clone());
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
            let target_agent = args
                .args
                .get(1)
                .filter(|s| !s.starts_with('-'))
                .map(|s| s.as_str());
            let status = manager
                .status(target_agent)
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
            let sent = manager
                .broadcast(&sender, "broadcast", &msg)
                .map_err(|e| CliError::General(e.to_string()))?;
            if json {
                print_output(true, &sent, "");
            } else {
                println!("Broadcast sent to all agents");
            }
        }

        "send" => {
            if args.args.len() < 3 {
                return Err(CliError::General(
                    "Usage: dft agent send <recipient> <message>".into(),
                ));
            }
            let recipient = &args.args[1];
            let msg = args.args[2..].join(" ");
            let sender = env::var("DFT_AGENT_NAME").unwrap_or_else(|_| "system".to_string());
            let sent = manager
                .send_message(&sender, recipient, "direct", &msg)
                .map_err(|e| CliError::General(e.to_string()))?;
            if json {
                print_output(true, &sent, "");
            } else {
                println!("Message sent to '{}'", recipient);
            }
        }

        "inbox" | "read" => {
            let is_json = json || args.args.iter().any(|a| a == "--json");
            let unread_only = args.args.iter().any(|a| a == "--unread");
            let agent = args
                .args
                .get(1)
                .filter(|s| !s.starts_with('-'))
                .map(|s| s.as_str());

            let msgs = manager
                .read_inbox(agent, unread_only)
                .map_err(|e| CliError::General(e.to_string()))?;

            if is_json {
                print_output(true, &msgs, "");
            } else {
                for m in msgs {
                    println!("[{}] {}: {}", m.timestamp_iso, m.sender, m.body);
                }
            }
        }

        "mark-read" => {
            let msg_id = match args.args.get(1) {
                Some(id) => id,
                None => {
                    return Err(CliError::General(
                        "Usage: dft agent mark-read <message_id> [agent]".into(),
                    ))
                }
            };
            let agent = args
                .args
                .get(2)
                .filter(|s| !s.starts_with('-'))
                .map(|s| s.as_str());
            manager
                .mark_read(msg_id, agent)
                .map_err(|e| CliError::General(e.to_string()))?;
            println!("Marked message '{}' as read", msg_id);
        }

        "info" | "get" => {
            let name = match args.args.get(1) {
                Some(n) => n,
                None => return Err(CliError::General("Usage: dft agent info <agent>".into())),
            };
            let identity = manager
                .get(name)
                .map_err(|e| CliError::General(e.to_string()))?;
            if json {
                print_output(true, &identity, "");
            } else {
                let dim = identity.assigned_dimension.as_deref().unwrap_or("mainline");
                println!(
                    "Agent: {} (id: {}, dimension: {})",
                    identity.name, identity.id, dim
                );
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
