//! Restore working tree or staged files from tree or index.

use super::reset::resolve_commit;
use crate::diff::tree::flatten_tree;
use crate::error::DaftError;
use crate::object::Commit;
use crate::repo::Repository;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

pub fn restore(
    repo: &Repository,
    pathspecs: &[PathBuf],
    staged: bool,
    worktree: bool,
    source: Option<&str>,
) -> Result<(), DaftError> {
    let repo_workdir = repo
        .workdir()
        .ok_or_else(|| DaftError::Config("Cannot restore in a bare repository".into()))?;

    let mut index = repo.index()?;

    // If neither --staged nor --worktree specified, default to --worktree
    let do_worktree = worktree || !staged;
    let do_staged = staged;

    // Load source tree if specified or if --staged
    let source_tree_files = if let Some(src) = source {
        let commit_oid = resolve_commit(repo, src)?;
        let raw = repo.cas().read_raw(&commit_oid)?;
        let commit = Commit::deserialize(&raw.data)?;
        let mut map = BTreeMap::new();
        flatten_tree(repo.cas().as_ref(), &commit.tree, "", &mut map)?;
        Some(map)
    } else if do_staged {
        if let Ok(head_oid) = crate::refs::peel_reference(repo.dft_dir(), "HEAD") {
            let raw = repo.cas().read_raw(&head_oid)?;
            let commit = Commit::deserialize(&raw.data)?;
            let mut map = BTreeMap::new();
            flatten_tree(repo.cas().as_ref(), &commit.tree, "", &mut map)?;
            Some(map)
        } else {
            // Unborn branch, source tree is empty
            Some(BTreeMap::new())
        }
    } else {
        None
    };

    for spec in pathspecs {
        let rel_str = if spec.is_absolute() {
            spec.strip_prefix(repo_workdir)
                .map_err(|_| DaftError::Config("Path is outside repository".into()))?
                .to_string_lossy()
                .replace('\\', "/")
        } else {
            spec.to_string_lossy().replace('\\', "/")
        };

        if do_staged {
            if let Some(ref tree_files) = source_tree_files {
                if let Some((mode, oid)) = tree_files.get(&rel_str) {
                    let raw = repo.cas().read_raw(oid)?;
                    let entry = crate::index::IndexEntry::new(
                        &rel_str,
                        *oid,
                        mode.0,
                        crate::index::Stage::Normal,
                        raw.data.len() as u32,
                    )?;
                    index.add_entry(entry);
                } else {
                    index.remove_path(&rel_str);
                }
            }
        }

        if do_worktree {
            if let Some(ref tree_files) = source_tree_files {
                if let Some((mode, oid)) = tree_files.get(&rel_str) {
                    let raw = repo.cas().read_raw(oid)?;
                    let full = repo_workdir.join(&rel_str);
                    if let Some(p) = full.parent() {
                        fs::create_dir_all(p)?;
                    }
                    fs::write(&full, &raw.data)?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if mode.is_executable() {
                            let _ = fs::set_permissions(&full, fs::Permissions::from_mode(0o755));
                        }
                    }
                } else {
                    let full = repo_workdir.join(&rel_str);
                    if full.exists() {
                        let _ = fs::remove_file(full);
                    }
                }
            } else {
                // Restore from Index
                if let Some(entry) = index.find_entry(&rel_str, crate::index::Stage::Normal) {
                    let raw = repo.cas().read_raw(&entry.oid)?;
                    let full = repo_workdir.join(&rel_str);
                    if let Some(p) = full.parent() {
                        fs::create_dir_all(p)?;
                    }
                    fs::write(&full, &raw.data)?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if (entry.mode & 0o111) != 0 {
                            let _ = fs::set_permissions(&full, fs::Permissions::from_mode(0o755));
                        }
                    }
                }
            }
        }
    }

    index.write_to(&repo.index_path())?;
    Ok(())
}
