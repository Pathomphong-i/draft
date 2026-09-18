use crate::cli::RestoreArgs;
use crate::error::CliError;
use daft_core::worktree::restore;
use daft_core::Repository;
use std::env;

pub fn execute(args: RestoreArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;

    let worktree = args.worktree || !args.staged;
    restore(
        &repo,
        &args.paths,
        args.staged,
        worktree,
        args.source.as_deref(),
    )?;
    Ok(())
}
