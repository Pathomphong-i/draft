use crate::cli::RebaseArgs;
use crate::error::CliError;
use daft_core::history::rebase;
use daft_core::Repository;
use std::env;

pub fn execute(args: RebaseArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;

    rebase(&repo, &args.upstream)?;
    println!("Successfully rebased and updated branch.");
    Ok(())
}
