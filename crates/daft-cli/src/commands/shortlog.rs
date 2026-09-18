use crate::cli::ShortlogArgs;
use crate::error::CliError;
use daft_core::history::get_shortlog;
use daft_core::Repository;
use std::env;

pub fn execute(args: ShortlogArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;

    let authors = get_shortlog(&repo, args.numbered, false)?;

    if args.summary {
        for a in authors {
            println!("{:>6}\t{}", a.count, a.author);
        }
    } else {
        for a in authors {
            println!("{} ({}):", a.author, a.count);
            for s in a.subjects {
                println!("      {}", s);
            }
            println!();
        }
    }

    Ok(())
}
