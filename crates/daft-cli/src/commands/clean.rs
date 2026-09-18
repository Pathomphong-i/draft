use crate::cli::CleanArgs;
use crate::error::CliError;
use daft_core::worktree::{clean_untracked, CleanOptions};
use daft_core::Repository;
use std::env;

pub fn execute(args: CleanArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;

    let removed = clean_untracked(
        &repo,
        CleanOptions {
            force: args.force,
            dry_run: args.dry_run,
            directories: args.directories,
        },
    )?;

    for path in removed {
        if args.dry_run {
            println!("Would remove {}", path.display());
        } else {
            println!("Removing {}", path.display());
        }
    }

    Ok(())
}
