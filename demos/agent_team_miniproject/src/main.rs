//! AetherDB CLI REPL.

use aether_db::api::{execute_command, parse_command, Command};
use aether_db::storage::AetherStorage;
use std::io::{self, BufRead, Write};

fn main() -> io::Result<()> {
    println!("==================================================");
    println!("⚡ AETHER-DB v0.1.0 (Built via Daft Multi-Agent Swarm)");
    println!("Commands: SET <k> <v> | GET <k> | DEL <k> | SCAN [pfx] | PING | QUIT");
    println!("==================================================");

    let mut storage = AetherStorage::open("aether.wal")?;
    let stdin = io::stdin();
    let mut reader = stdin.lock();

    loop {
        print!("aether> ");
        io::stdout().flush()?;

        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }

        let cmd = parse_command(&line);
        if cmd == Command::Quit {
            println!("Bye!");
            break;
        }

        let resp = execute_command(&mut storage, cmd);
        println!("{}", resp);
    }

    Ok(())
}
