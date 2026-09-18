use crate::cli::BranchArgs;
use crate::error::CliError;
use crate::output::print_output;
use daft_core::branch::BranchManager;
use daft_core::Repository;
use serde::Serialize;
use std::env;

#[derive(Serialize, Debug)]
pub struct BranchJsonEntry {
    pub name: String,
    pub target: String,
    pub is_head: bool,
}

pub fn execute(args: BranchArgs, json: bool) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let manager = BranchManager::new(&repo);

    if let Some(rename) = &args.rename {
        if rename.len() >= 2 {
            manager.rename(&rename[0], &rename[1], false)?;
            return Ok(());
        } else {
            return Err(CliError::General(
                "Branch rename requires OLD and NEW branch names".into(),
            ));
        }
    }

    if let Some(del) = &args.delete {
        let _ = manager.delete(del, false)?;
        println!("Deleted branch {}.", del);
        return Ok(());
    }

    if let Some(fdel) = &args.force_delete {
        let _ = manager.delete(fdel, true)?;
        println!("Deleted branch {} (was forced).", fdel);
        return Ok(());
    }

    if let Some(name) = &args.name {
        let _ = manager.create(name, None, false)?;
        return Ok(());
    }

    // List branches
    let branches = manager.list()?;
    if json {
        let json_entries: Vec<BranchJsonEntry> = branches
            .into_iter()
            .map(|b| BranchJsonEntry {
                name: b.name,
                target: b.target.to_hex(),
                is_head: b.is_head,
            })
            .collect();
        print_output(true, &json_entries, "");
    } else {
        for b in branches {
            if b.is_head {
                println!("* {}", b.name);
            } else {
                println!("  {}", b.name);
            }
        }
    }

    Ok(())
}
