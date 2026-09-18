use crate::cli::GrepArgs;
use crate::error::CliError;
use daft_core::history::grep;
use daft_core::Repository;
use std::env;

pub fn execute(args: GrepArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;

    let matches = grep(&repo, &args.pattern, None, args.ignore_case, false)?;

    for m in matches {
        if args.line_number {
            println!("{}:{}:{}", m.path, m.line_number, m.line);
        } else {
            println!("{}:{}", m.path, m.line);
        }
    }

    Ok(())
}
