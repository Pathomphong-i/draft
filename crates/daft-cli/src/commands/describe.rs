use crate::cli::DescribeArgs;
use crate::error::CliError;
use daft_core::history::describe;
use daft_core::Repository;
use std::env;

pub fn execute(args: DescribeArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;

    let desc = describe(&repo, args.commit.as_deref(), true, false)?;
    println!("{}", desc);
    Ok(())
}
