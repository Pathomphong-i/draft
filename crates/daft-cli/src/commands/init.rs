use crate::cli::InitArgs;
use crate::error::CliError;
use daft_core::{init, InitOptions};
use std::env;

pub fn execute(args: InitArgs, quiet: bool) -> Result<(), CliError> {
    let current_dir = env::current_dir()?;
    let target = args.directory.unwrap_or(current_dir);
    let options = InitOptions {
        bare: args.bare,
        initial_branch: args.initial_branch,
        reinit: false,
    };

    let repo = init(&target, &options)?;

    if !quiet {
        let dft_dir = repo.dft_dir();
        println!("Initialized empty Draft repository in {}", dft_dir.display());
    }

    Ok(())
}
