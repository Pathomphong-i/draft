use crate::cli::BlameArgs;
use crate::error::CliError;
use chrono::DateTime;
use daft_core::history::blame_file;
use daft_core::Repository;
use std::env;

pub fn execute(args: BlameArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;

    let blame_lines = blame_file(&repo, &args.file, None)?;

    for line in blame_lines {
        let short_hash = &line.commit_id.to_hex()[..8];
        let date_str = DateTime::from_timestamp(line.author.time, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default();
        let content_trimmed = line.content.trim_end_matches('\n');
        println!(
            "{} ({} {}) {}",
            short_hash, line.author.name, date_str, content_trimmed
        );
    }

    Ok(())
}
