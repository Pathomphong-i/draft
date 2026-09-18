use crate::cli::CheckoutArgs;
use crate::error::CliError;
use daft_core::checkout::CheckoutEngine;
use daft_core::Repository;
use std::env;

pub fn execute(args: CheckoutArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let engine = CheckoutEngine::new(&repo);

    if let Some(b) = &args.create_branch {
        engine.switch_branch(b, true)?;
        println!("Switched to a new branch '{}'", b);
        return Ok(());
    }

    if !args.paths.is_empty() {
        let refs: Vec<&std::path::Path> = args.paths.iter().map(|p| p.as_path()).collect();
        engine.checkout_paths(&refs, args.target.as_deref())?;
        return Ok(());
    }

    if let Some(target) = &args.target {
        let branch_ref = format!("refs/heads/{}", target);
        if repo.refs().read_ref(&branch_ref).is_ok() {
            engine.switch_branch(target, false)?;
            println!("Switched to branch '{}'", target);
        } else {
            let oid = engine.checkout_commit(target)?;
            println!("Note: switching to '{}'.", target);
            println!("HEAD is now at {}...", &oid.to_hex()[..7]);
        }
    }

    Ok(())
}
