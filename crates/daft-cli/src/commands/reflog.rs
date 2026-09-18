use crate::cli::ReflogArgs;
use crate::error::CliError;
use daft_core::Repository;
use std::env;

pub fn execute(args: ReflogArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let target_ref = args.ref_name.as_deref().unwrap_or("HEAD");

    let entries = repo.reflog().read_all(target_ref)?;
    let limit = args.count.unwrap_or(entries.len());

    for (idx, entry) in entries.into_iter().take(limit).enumerate() {
        let short_hash = &entry.new_oid.to_hex()[..7];
        println!(
            "{} {}@{{{}}}: {}",
            short_hash, target_ref, idx, entry.message
        );
    }

    Ok(())
}
