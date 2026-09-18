use crate::cli::RmArgs;
use crate::error::CliError;
use daft_core::worktree::remove_paths;
use daft_core::Repository;
use std::env;

pub fn execute(args: RmArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;

    remove_paths(&repo, &args.paths, args.cached, args.force)?;
    Ok(())
}
