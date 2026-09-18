use crate::cli::StatusArgs;
use crate::error::CliError;
use crate::output::print_output;
use daft_core::worktree::get_status;
use daft_core::Repository;
use serde::Serialize;
use std::env;

#[derive(Serialize, Debug)]
pub struct StatusFileEntry {
    pub path: String,
    pub status: String,
}

#[derive(Serialize, Debug)]
pub struct StatusJsonOutput {
    pub branch: String,
    pub clean: bool,
    pub is_empty_repo: bool,
    pub staged: Vec<StatusFileEntry>,
    pub unstaged: Vec<StatusFileEntry>,
    pub untracked: Vec<String>,
}

pub fn execute(args: StatusArgs, json: bool) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let report = get_status(&repo)?;

    let mut staged_entries = Vec::new();
    for p in &report.staged_added {
        staged_entries.push(StatusFileEntry {
            path: p.clone(),
            status: "new file".to_string(),
        });
    }
    for p in &report.staged_modified {
        staged_entries.push(StatusFileEntry {
            path: p.clone(),
            status: "modified".to_string(),
        });
    }
    for p in &report.staged_deleted {
        staged_entries.push(StatusFileEntry {
            path: p.clone(),
            status: "deleted".to_string(),
        });
    }

    let mut unstaged_entries = Vec::new();
    for p in &report.unstaged_modified {
        unstaged_entries.push(StatusFileEntry {
            path: p.clone(),
            status: "modified".to_string(),
        });
    }
    for p in &report.unstaged_deleted {
        unstaged_entries.push(StatusFileEntry {
            path: p.clone(),
            status: "deleted".to_string(),
        });
    }

    let json_output = StatusJsonOutput {
        branch: report.branch.clone(),
        clean: report.is_clean(),
        is_empty_repo: report.is_empty_repo,
        staged: staged_entries,
        unstaged: unstaged_entries,
        untracked: report.untracked.clone(),
    };

    if json {
        print_output(true, &json_output, "");
        return Ok(());
    }

    if args.short {
        for p in &report.staged_added {
            println!("A  {}", p);
        }
        for p in &report.staged_modified {
            println!("M  {}", p);
        }
        for p in &report.staged_deleted {
            println!("D  {}", p);
        }
        for p in &report.unstaged_modified {
            println!(" M {}", p);
        }
        for p in &report.unstaged_deleted {
            println!(" D {}", p);
        }
        for p in &report.untracked {
            println!("?? {}", p);
        }
        return Ok(());
    }

    let mut out = String::new();
    out.push_str(&format!("On branch {}\n", report.branch));

    if report.is_empty_repo {
        out.push_str("\nNo commits yet\n");
    }

    if report.is_clean() {
        out.push_str("\nnothing to commit, working tree clean\n");
    } else {
        if !report.staged_added.is_empty()
            || !report.staged_modified.is_empty()
            || !report.staged_deleted.is_empty()
        {
            out.push_str("\nChanges to be committed:\n");
            for p in &report.staged_added {
                out.push_str(&format!("\tnew file:   {}\n", p));
            }
            for p in &report.staged_modified {
                out.push_str(&format!("\tmodified:   {}\n", p));
            }
            for p in &report.staged_deleted {
                out.push_str(&format!("\tdeleted:    {}\n", p));
            }
        }

        if !report.unstaged_modified.is_empty() || !report.unstaged_deleted.is_empty() {
            out.push_str("\nChanges not staged for commit:\n");
            for p in &report.unstaged_modified {
                out.push_str(&format!("\tmodified:   {}\n", p));
            }
            for p in &report.unstaged_deleted {
                out.push_str(&format!("\tdeleted:    {}\n", p));
            }
        }

        if !report.untracked.is_empty() {
            out.push_str("\nUntracked files:\n");
            for p in &report.untracked {
                out.push_str(&format!("\t{}\n", p));
            }
        }
    }

    print!("{}", out);
    Ok(())
}
