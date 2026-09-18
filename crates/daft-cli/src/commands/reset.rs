use crate::cli::ResetArgs;
use crate::error::CliError;
use daft_core::worktree::{reset, ResetMode};
use daft_core::Repository;
use std::env;

pub fn execute(args: ResetArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;

    let mode = if args.hard {
        ResetMode::Hard
    } else if args.soft {
        ResetMode::Soft
    } else {
        ResetMode::Mixed
    };

    reset(&repo, &args.target, mode)?;
    Ok(())
}
