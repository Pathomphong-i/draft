use crate::cli::MergeArgs;
use crate::error::CliError;
use daft_core::merge::{merge_abort, merge_commits, MergeOutcome};
use daft_core::worktree::resolve_commit;
use daft_core::Repository;
use std::env;

pub fn execute(args: MergeArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;

    if args.abort {
        merge_abort(&repo)?;
        return Ok(());
    }

    let target_name = match &args.target {
        Some(t) => t.as_str(),
        None => {
            return Err(CliError::General(
                "No commit or branch specified to merge".into(),
            ))
        }
    };

    let target_oid = resolve_commit(&repo, target_name)?;
    let outcome = merge_commits(&repo, target_name, &target_oid, args.message.as_deref())?;

    match outcome {
        MergeOutcome::AlreadyUpToDate => {
            println!("Already up to date.");
        }
        MergeOutcome::FastForward { new_head } => {
            println!("Updating {}... Fast-forward", &new_head.to_hex()[..7]);
        }
        MergeOutcome::Clean { commit_oid } => {
            println!("Merge made by the '{}' strategy.", args.strategy);
            println!("Created commit {}", &commit_oid.to_hex()[..7]);
        }
        MergeOutcome::Conflict { conflicting_files } => {
            for cf in &conflicting_files {
                println!("CONFLICT (content): Merge conflict in {}", cf);
            }
            return Err(CliError::MergeConflict);
        }
    }

    Ok(())
}
