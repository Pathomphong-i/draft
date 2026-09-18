use crate::cli::ShowArgs;
use crate::error::CliError;
use chrono::DateTime;
use daft_core::history::{show_object, ShowResult};
use daft_core::Repository;
use std::env;

pub fn execute(args: ShowArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let target = args.object.as_deref().unwrap_or("HEAD");

    let res = show_object(&repo, target)?;
    match res {
        ShowResult::Commit { oid, commit, diff } => {
            let date_str = DateTime::from_timestamp(commit.author.time, 0)
                .map(|dt| dt.to_rfc2822())
                .unwrap_or_default();
            println!("commit {}", oid.to_hex());
            println!(
                "Author:     {} <{}>",
                commit.author.name, commit.author.email
            );
            println!("Date:       {}", date_str);
            println!();
            for line in commit.message.lines() {
                println!("    {}", line);
            }
            println!();
            if !diff.trim().is_empty() {
                print!("{}", diff);
            }
        }
        ShowResult::Tag { oid, tag } => {
            println!("tag {}", tag.name);
            println!("Tagger:     {:?}", tag.tagger);
            println!();
            for line in tag.message.lines() {
                println!("    {}", line);
            }
            println!();
            println!("commit {}", oid.to_hex());
        }
        ShowResult::Tree { oid, tree } => {
            println!("tree {}", oid.to_hex());
            for e in tree.entries() {
                println!("{} {}", e.oid, e.name);
            }
        }
        ShowResult::Blob { oid, content } => {
            println!("blob {}", oid.to_hex());
            if let Ok(text) = std::str::from_utf8(&content) {
                print!("{}", text);
            } else {
                println!("Binary blob ({} bytes)", content.len());
            }
        }
    }

    Ok(())
}
