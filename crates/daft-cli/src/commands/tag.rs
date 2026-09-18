use crate::cli::TagArgs;
use crate::error::CliError;
use crate::output::print_output;
use daft_core::tag::TagManager;
use daft_core::Repository;
use std::env;

pub fn execute(args: TagArgs, json: bool) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let manager = TagManager::new(&repo);

    if let Some(del) = &args.delete {
        manager.delete(del)?;
        println!("Deleted tag '{}'", del);
        return Ok(());
    }

    if let Some(name) = &args.name {
        if args.annotate {
            let msg = args.message.as_deref().unwrap_or("");
            let _ = manager.create_annotated(name, msg, args.target.as_deref())?;
        } else {
            let _ = manager.create_lightweight(name, args.target.as_deref())?;
        }
        return Ok(());
    }

    let tags = manager.list()?;
    if json {
        print_output(true, &tags, "");
    } else {
        for t in tags {
            println!("{}", t);
        }
    }

    Ok(())
}
