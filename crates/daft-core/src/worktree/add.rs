//! Recursive staging of working tree files into the index.

use crate::cas::{ObjectType, RawObject};
use crate::error::DaftError;
use crate::index::{IndexEntry, IndexTime, Stage};
use crate::object::FileMode;
use crate::repo::Repository;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

pub fn add_paths(
    repo: &Repository,
    pathspecs: &[PathBuf],
    all: bool,
    update: bool,
) -> Result<(), DaftError> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| DaftError::Config("Cannot add files in a bare repository".into()))?;
    let mut index = repo.index()?;
    let dft_dir = repo.dft_dir();

    let mut files_to_stage = Vec::new();

    if all || pathspecs.iter().any(|p| p.as_os_str() == ".") {
        // Stage entire worktree
        for entry in WalkDir::new(workdir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.starts_with(dft_dir) {
                continue;
            }
            if entry.file_type().is_file() {
                if let Ok(rel) = path.strip_prefix(workdir) {
                    files_to_stage.push(rel.to_path_buf());
                }
            }
        }

        // Also check if any tracked files in index were deleted
        let mut deleted_paths = Vec::new();
        for entry in index.entries() {
            let full_path = workdir.join(&entry.path);
            if !full_path.exists() {
                deleted_paths.push(entry.path.clone());
            }
        }
        for del in deleted_paths {
            index.remove_path(&del);
        }
    } else {
        for spec in pathspecs {
            let full = if spec.is_absolute() {
                spec.clone()
            } else {
                workdir.join(spec)
            };

            if !full.exists() {
                // Check if it was a tracked file deleted on disk
                let rel = if spec.is_absolute() {
                    spec.strip_prefix(workdir).ok().map(|p| p.to_path_buf())
                } else {
                    Some(spec.clone())
                };

                if let Some(r) = rel {
                    let rel_str = r.to_string_lossy().replace('\\', "/");
                    if index.find_entry(&rel_str, Stage::Normal).is_some()
                        || index.find_entry(&rel_str, Stage::Ours).is_some()
                    {
                        index.remove_path(&rel_str);
                        continue;
                    }
                }
                return Err(DaftError::Config(format!(
                    "pathspec '{}' did not match any files",
                    spec.display()
                )));
            }

            if full.is_dir() {
                for entry in WalkDir::new(&full).into_iter().filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.starts_with(dft_dir) {
                        continue;
                    }
                    if entry.file_type().is_file() {
                        if let Ok(rel) = path.strip_prefix(workdir) {
                            files_to_stage.push(rel.to_path_buf());
                        }
                    }
                }
            } else if full.is_file() {
                if let Ok(rel) = full.strip_prefix(workdir) {
                    files_to_stage.push(rel.to_path_buf());
                }
            }
        }
    }

    // Process all files to stage
    for rel_path in files_to_stage {
        let rel_str = rel_path.to_string_lossy().replace('\\', "/");
        let full_path = workdir.join(&rel_path);

        if update && index.find_entry(&rel_str, Stage::Normal).is_none() {
            // Update mode only stages tracked files
            continue;
        }

        let data = fs::read(&full_path)?;
        let raw = RawObject::new(ObjectType::Blob, data.clone());
        let oid = repo.cas().write_raw(&raw)?;

        let meta = fs::metadata(&full_path)?;
        let is_exec = {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                meta.permissions().mode() & 0o111 != 0
            }
            #[cfg(not(unix))]
            {
                false
            }
        };

        let mode = if is_exec {
            FileMode::EXECUTABLE.0
        } else {
            FileMode::REGULAR.0
        };

        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| IndexTime {
                sec: d.as_secs() as u32,
                nsec: d.subsec_nanos(),
            })
            .unwrap_or_default();

        let mut entry = IndexEntry::new(rel_str, oid, mode, Stage::Normal, data.len() as u32)?;
        entry.mtime = mtime;
        index.add_entry(entry);
    }

    index.write_to(&repo.index_path())?;
    Ok(())
}
