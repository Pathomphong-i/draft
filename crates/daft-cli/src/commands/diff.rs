use crate::cli::DiffArgs;
use crate::error::CliError;
use crate::output::print_output;
use daft_core::diff::{
    diff_commits, diff_index_to_tree, diff_worktree_to_index, format_unified_diff, EditOpKind,
};
use daft_core::object::Commit;
use daft_core::refs::peel_reference;
use daft_core::worktree::resolve_commit;
use daft_core::Repository;
use serde::Serialize;
use std::env;

#[derive(Serialize, Debug)]
pub struct DiffJson {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub is_binary: bool,
    pub hunks: Vec<HunkJson>,
}

#[derive(Serialize, Debug)]
pub struct HunkJson {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub lines: Vec<String>,
}

pub fn execute(args: DiffArgs, json: bool) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;

    let patches = if args.revisions.len() == 2 {
        let oid1 = resolve_commit(&repo, &args.revisions[0])?;
        let oid2 = resolve_commit(&repo, &args.revisions[1])?;
        diff_commits(&oid1, &oid2, repo.cas().as_ref())?
    } else if args.staged {
        let index = repo.index()?;
        let head_tree = if let Ok(head_oid) = peel_reference(repo.dft_dir(), "HEAD") {
            let raw = repo.cas().read_raw(&head_oid)?;
            let commit = Commit::deserialize(&raw.data)?;
            Some(commit.tree)
        } else {
            None
        };
        diff_index_to_tree(&index, head_tree.as_ref(), repo.cas().as_ref())?
    } else {
        let workdir = repo.workdir().ok_or_else(|| {
            CliError::General("Cannot diff working tree in a bare repository".into())
        })?;
        let index = repo.index()?;
        diff_worktree_to_index(workdir, &index, repo.cas().as_ref())?
    };

    if json {
        let json_patches: Vec<DiffJson> = patches
            .iter()
            .map(|p| DiffJson {
                old_path: p.old_path.clone(),
                new_path: p.new_path.clone(),
                is_binary: p.is_binary,
                hunks: p
                    .hunks
                    .iter()
                    .map(|h| HunkJson {
                        old_start: h.old_start,
                        old_count: h.old_count,
                        new_start: h.new_start,
                        new_count: h.new_count,
                        lines: h
                            .lines
                            .iter()
                            .map(|l| {
                                let prefix = match l.kind {
                                    EditOpKind::Insert => "+",
                                    EditOpKind::Delete => "-",
                                    EditOpKind::Equal => " ",
                                };
                                format!("{}{}", prefix, l.content)
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect();
        print_output(true, &json_patches, "");
    } else {
        let formatted = format_unified_diff(&patches);
        if !formatted.trim().is_empty() {
            print!("{}", formatted);
        }
    }

    Ok(())
}
