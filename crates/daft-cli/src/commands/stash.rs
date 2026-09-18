use crate::cli::{StashAction, StashArgs};
use crate::error::CliError;
use daft_core::worktree::{
    stash_apply, stash_clear, stash_drop, stash_list, stash_pop, stash_push,
};
use daft_core::Repository;
use std::env;

pub fn execute(args: StashArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;

    match args.action {
        None => {
            stash_push(&repo, args.message.as_deref())?;
            println!("Saved working directory and index state");
        }
        Some(StashAction::Push { message }) => {
            stash_push(&repo, message.as_deref())?;
            println!("Saved working directory and index state");
        }
        Some(StashAction::Pop { index }) => {
            stash_pop(&repo, index)?;
            println!("Dropped stash@{{{}}} and restored state", index);
        }
        Some(StashAction::Apply { index }) => {
            stash_apply(&repo, index)?;
            println!("Applied stash@{{{}}}", index);
        }
        Some(StashAction::List) => {
            let stashes = stash_list(&repo)?;
            for s in stashes {
                println!("{}", s);
            }
        }
        Some(StashAction::Drop { index }) => {
            stash_drop(&repo, index)?;
            println!("Dropped stash@{{{}}}", index);
        }
        Some(StashAction::Clear) => {
            stash_clear(&repo)?;
            println!("Cleared all stashes");
        }
    }

    Ok(())
}
