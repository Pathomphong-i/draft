use crate::cli::CherryPickArgs;
use crate::error::CliError;
use daft_core::history::cherry_pick;
use daft_core::Repository;
use std::env;

pub fn execute(args: CherryPickArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;

    let new_oid = cherry_pick(&repo, &args.commit)?;
    println!("[cherry-pick {}] applied", &new_oid.to_hex()[..7]);
    Ok(())
}
