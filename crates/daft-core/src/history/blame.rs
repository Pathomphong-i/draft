//! Line-by-line ancestry attribution (`dft blame`).

use crate::cas::ObjectId;
use crate::diff::myers::{myers_diff, EditOpKind};
use crate::diff::tree::flatten_tree;
use crate::error::DaftError;
use crate::object::{Commit, Signature};
use crate::repo::Repository;
use crate::worktree::reset::resolve_commit;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct BlameLine {
    pub line_number: usize,
    pub commit_id: ObjectId,
    pub author: Signature,
    pub content: String,
}

pub fn blame_file(
    repo: &Repository,
    path: &Path,
    commit_rev: Option<&str>,
) -> Result<Vec<BlameLine>, DaftError> {
    let start_oid = if let Some(rev) = commit_rev {
        resolve_commit(repo, rev)?
    } else {
        crate::refs::peel_reference(repo.dft_dir(), "HEAD").map_err(DaftError::Ref)?
    };

    let rel_str = path.to_string_lossy().replace('\\', "/");

    // Load file at start commit
    let raw = repo.cas().read_raw(&start_oid)?;
    let commit = Commit::deserialize(&raw.data)?;

    let mut tree_files = BTreeMap::new();
    flatten_tree(repo.cas().as_ref(), &commit.tree, "", &mut tree_files)?;

    let (_, blob_oid) = tree_files
        .get(&rel_str)
        .ok_or_else(|| DaftError::Config(format!("no such path '{}' in {}", rel_str, start_oid)))?;

    let blob_raw = repo.cas().read_raw(blob_oid)?;
    let text = std::str::from_utf8(&blob_raw.data)
        .map_err(|e| DaftError::Config(format!("file is binary or not UTF-8: {}", e)))?;

    let lines: Vec<String> = text.lines().map(|l| format!("{}\n", l)).collect();
    let num_lines = lines.len();
    if num_lines == 0 {
        return Ok(Vec::new());
    }

    let mut line_commits: Vec<Option<(ObjectId, Signature)>> = vec![None; num_lines];
    let mut current_oid = start_oid;
    let mut current_lines = lines.clone();
    let mut line_map: Vec<usize> = (0..num_lines).collect(); // current line idx -> original line idx

    while line_commits.iter().any(|c| c.is_none()) {
        let cur_raw = repo.cas().read_raw(&current_oid)?;
        let cur_commit = Commit::deserialize(&cur_raw.data)?;

        if cur_commit.parents.is_empty() {
            // Root commit: attributes all remaining lines
            for &orig_idx in &line_map {
                if line_commits[orig_idx].is_none() {
                    line_commits[orig_idx] = Some((current_oid, cur_commit.author.clone()));
                }
            }
            break;
        }

        let parent_oid = cur_commit.parents[0];
        let parent_raw = repo.cas().read_raw(&parent_oid)?;
        let parent_commit = Commit::deserialize(&parent_raw.data)?;

        let mut parent_tree_files = BTreeMap::new();
        flatten_tree(
            repo.cas().as_ref(),
            &parent_commit.tree,
            "",
            &mut parent_tree_files,
        )?;

        let parent_blob_info = parent_tree_files.get(&rel_str);
        if parent_blob_info.is_none() {
            // File was introduced in cur_commit
            for &orig_idx in &line_map {
                if line_commits[orig_idx].is_none() {
                    line_commits[orig_idx] = Some((current_oid, cur_commit.author.clone()));
                }
            }
            break;
        }

        let (_, p_blob_oid) = parent_blob_info.unwrap();
        let p_raw = repo.cas().read_raw(p_blob_oid)?;
        let p_text = std::str::from_utf8(&p_raw.data).unwrap_or("");
        let p_lines: Vec<String> = p_text.lines().map(|l| format!("{}\n", l)).collect();

        // Run diff between parent and current
        let cur_text = current_lines.concat();
        let ops = myers_diff(p_text, &cur_text);

        let mut next_line_map = Vec::new();
        let mut cur_line_idx = 0;

        for op in ops {
            match op.kind {
                EditOpKind::Equal => {
                    if cur_line_idx < line_map.len() {
                        next_line_map.push(line_map[cur_line_idx]);
                        cur_line_idx += 1;
                    }
                }
                EditOpKind::Insert => {
                    // Added in cur_commit
                    if cur_line_idx < line_map.len() {
                        let orig_idx = line_map[cur_line_idx];
                        if line_commits[orig_idx].is_none() {
                            line_commits[orig_idx] = Some((current_oid, cur_commit.author.clone()));
                        }
                        cur_line_idx += 1;
                    }
                }
                EditOpKind::Delete => {
                    // Existed in parent, deleted in current: does not affect current lines
                }
            }
        }

        current_oid = parent_oid;
        current_lines = p_lines;
        line_map = next_line_map;
    }

    let default_sig = Signature::now("Unknown", "unknown@draft-vcs.org");
    let mut result = Vec::new();
    for (i, content) in lines.into_iter().enumerate() {
        let (cid, author) = line_commits[i]
            .clone()
            .unwrap_or((start_oid, default_sig.clone()));
        result.push(BlameLine {
            line_number: i + 1,
            commit_id: cid,
            author,
            content,
        });
    }

    Ok(result)
}
