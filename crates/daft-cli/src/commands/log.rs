use crate::cli::LogArgs;
use crate::error::CliError;
use crate::output::print_output;
use chrono::DateTime;
use daft_core::history::get_log;
use daft_core::refs::peel_reference;
use daft_core::Repository;
use serde::Serialize;
use std::env;

#[derive(Serialize, Debug)]
pub struct LogJsonEntry {
    pub oid: String,
    pub author_name: String,
    pub author_email: String,
    pub date: String,
    pub message: String,
}

pub fn execute(args: LogArgs, json: bool) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;

    if peel_reference(repo.dft_dir(), "HEAD").is_err() {
        println!("No commits yet");
        return Ok(());
    }

    let entries = get_log(&repo, args.revision_range.as_deref(), args.max_count, false)?;

    if json {
        let json_entries: Vec<LogJsonEntry> = entries
            .iter()
            .map(|e| {
                let date_str = DateTime::from_timestamp(e.author.time, 0)
                    .map(|dt| dt.to_rfc2822())
                    .unwrap_or_default();
                LogJsonEntry {
                    oid: e.oid.to_hex(),
                    author_name: e.author.name.clone(),
                    author_email: e.author.email.clone(),
                    date: date_str,
                    message: e.message.clone(),
                }
            })
            .collect();
        print_output(true, &json_entries, "");
        return Ok(());
    }

    if args.oneline {
        for entry in entries {
            let short_hash = &entry.oid.to_hex()[..7];
            let title = entry.message.lines().next().unwrap_or("");
            println!("{} {}", short_hash, title);
        }
    } else {
        for entry in entries {
            let date_str = DateTime::from_timestamp(entry.author.time, 0)
                .map(|dt| dt.to_rfc2822())
                .unwrap_or_default();
            println!("commit {}", entry.oid.to_hex());
            println!("Author:     {} <{}>", entry.author.name, entry.author.email);
            println!("Date:       {}", date_str);
            println!();
            for line in entry.message.lines() {
                println!("    {}", line);
            }
            println!();
        }
    }

    Ok(())
}
