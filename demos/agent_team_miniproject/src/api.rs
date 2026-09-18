//! Command Parser and Execution API for AetherDB.

use crate::storage::AetherStorage;

#[derive(Debug, PartialEq)]
pub enum Command {
    Set(String, String),
    Get(String),
    Del(String),
    Scan(String),
    Ping,
    Quit,
    Unknown(String),
}

pub fn parse_command(input: &str) -> Command {
    let parts: Vec<&str> = input.trim().split_whitespace().collect();
    if parts.is_empty() {
        return Command::Unknown("Empty command".into());
    }

    match parts[0].to_uppercase().as_str() {
        "SET" => {
            if parts.len() >= 3 {
                Command::Set(parts[1].to_string(), parts[2..].join(" "))
            } else {
                Command::Unknown("Usage: SET <key> <value>".into())
            }
        }
        "GET" => {
            if parts.len() == 2 {
                Command::Get(parts[1].to_string())
            } else {
                Command::Unknown("Usage: GET <key>".into())
            }
        }
        "DEL" => {
            if parts.len() == 2 {
                Command::Del(parts[1].to_string())
            } else {
                Command::Unknown("Usage: DEL <key>".into())
            }
        }
        "SCAN" => {
            let prefix = if parts.len() > 1 { parts[1] } else { "" };
            Command::Scan(prefix.to_string())
        }
        "PING" => Command::Ping,
        "QUIT" | "EXIT" => Command::Quit,
        other => Command::Unknown(format!("Unknown command: {}", other)),
    }
}

pub fn execute_command(storage: &mut AetherStorage, cmd: Command) -> String {
    match cmd {
        Command::Set(k, v) => match storage.set(&k, &v) {
            Ok(_) => "OK".to_string(),
            Err(e) => format!("ERR: {}", e),
        },
        Command::Get(k) => match storage.get(&k) {
            Some(v) => format!("\"{}\"", v),
            None => "(nil)".to_string(),
        },
        Command::Del(k) => match storage.del(&k) {
            Ok(true) => "(integer) 1".to_string(),
            Ok(false) => "(integer) 0".to_string(),
            Err(e) => format!("ERR: {}", e),
        },
        Command::Scan(prefix) => {
            let entries = storage.scan(&prefix);
            if entries.is_empty() {
                "(empty list or set)".to_string()
            } else {
                entries
                    .into_iter()
                    .enumerate()
                    .map(|(i, (k, v))| format!("{}. \"{}\" -> \"{}\"", i + 1, k, v))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        Command::Ping => "PONG".to_string(),
        Command::Quit => "BYE".to_string(),
        Command::Unknown(err) => format!("ERR: {}", err),
    }
}
