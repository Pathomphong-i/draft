use crate::cli::SwitchArgs;
use crate::error::CliError;
use daft_core::checkout::CheckoutEngine;
use daft_core::Repository;
use std::env;

pub fn execute(args: SwitchArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let engine = CheckoutEngine::new(&repo);

    if let Some(c) = &args.create {
        engine.switch_branch(c, true)?;
        println!("Switched to a new branch '{}'", c);
        return Ok(());
    }

    if let Some(target) = &args.branch {
        if args.detach {
            let oid = engine.checkout_commit(target)?;
            println!("HEAD is now at {}...", &oid.to_hex()[..7]);
        } else {
            engine.switch_branch(target, false)?;
            println!("Switched to branch '{}'", target);
        }
        return Ok(());
    }

    Err(CliError::General("Missing branch name to switch to".into()))
}
