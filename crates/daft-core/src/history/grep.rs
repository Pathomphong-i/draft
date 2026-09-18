//! Pattern search across tracked files or commits (`dft grep`).

use crate::diff::tree::flatten_tree;
use crate::error::DaftError;
use crate::object::Commit;
use crate::repo::Repository;
use crate::worktree::reset::resolve_commit;
use std::collections::BTreeMap;
use std::fs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrepMatch {
    pub path: String,
    pub line_number: usize,
    pub line: String,
}

pub fn grep(
    repo: &Repository,
    pattern: &str,
    rev: Option<&str>,
    ignore_case: bool,
    invert: bool,
) -> Result<Vec<GrepMatch>, DaftError> {
    let query = if ignore_case {
        pattern.to_lowercase()
    } else {
        pattern.to_string()
    };

    let mut matches = Vec::new();

    if let Some(r) = rev {
        // Search historical commit tree
        let commit_oid = resolve_commit(repo, r)?;
        let raw = repo.cas().read_raw(&commit_oid)?;
        let commit = Commit::deserialize(&raw.data)?;

        let mut tree_files = BTreeMap::new();
        flatten_tree(repo.cas().as_ref(), &commit.tree, "", &mut tree_files)?;

        for (path, (_, blob_oid)) in tree_files {
            let blob_raw = repo.cas().read_raw(&blob_oid)?;
            if crate::diff::binary::is_binary(&blob_raw.data) {
                continue;
            }
            if let Ok(text) = std::str::from_utf8(&blob_raw.data) {
                for (idx, line) in text.lines().enumerate() {
                    let search_target = if ignore_case {
                        line.to_lowercase()
                    } else {
                        line.to_string()
                    };
                    let matched = search_target.contains(&query);
                    if matched ^ invert {
                        matches.push(GrepMatch {
                            path: path.clone(),
                            line_number: idx + 1,
                            line: line.to_string(),
                        });
                    }
                }
            }
        }
    } else {
        // Search working tree tracked files
        let index = repo.index()?;
        let workdir = repo
            .workdir()
            .ok_or_else(|| DaftError::Config("Cannot grep worktree in a bare repository".into()))?;

        for entry in index.entries() {
            let full_path = workdir.join(&entry.path);
            if !full_path.is_file() {
                continue;
            }
            if let Ok(bytes) = fs::read(&full_path) {
                if crate::diff::binary::is_binary(&bytes) {
                    continue;
                }
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    for (idx, line) in text.lines().enumerate() {
                        let search_target = if ignore_case {
                            line.to_lowercase()
                        } else {
                            line.to_string()
                        };
                        let matched = search_target.contains(&query);
                        if matched ^ invert {
                            matches.push(GrepMatch {
                                path: entry.path.clone(),
                                line_number: idx + 1,
                                line: line.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(matches)
}
