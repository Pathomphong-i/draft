use crate::cli::MvArgs;
use crate::error::CliError;
use daft_core::worktree::move_path;
use daft_core::Repository;
use std::env;

pub fn execute(args: MvArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;

    move_path(&repo, &args.source, &args.destination, args.force)?;
    Ok(())
}
