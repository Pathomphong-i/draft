//! Clean untracked files and directories from the working tree.

use super::status::find_untracked_files;
use crate::error::DaftError;
use crate::repo::Repository;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Default)]
pub struct CleanOptions {
    pub force: bool,
    pub directories: bool,
    pub dry_run: bool,
}

pub fn clean_untracked(repo: &Repository, opts: CleanOptions) -> Result<Vec<PathBuf>, DaftError> {
    if !opts.force && !opts.dry_run {
        return Err(DaftError::Config(
            "clean.requireForce defaults to true and neither -i, -n, nor -f given; refusing to clean".into(),
        ));
    }

    let workdir = repo
        .workdir()
        .ok_or_else(|| DaftError::Config("Cannot clean in a bare repository".into()))?;
    let index = repo.index()?;
    let untracked = find_untracked_files(workdir, &index, repo.dft_dir())?;

    let mut removed = Vec::new();

    for rel in untracked {
        let full = workdir.join(&rel);
        if full.is_file() {
            removed.push(PathBuf::from(&rel));
            if !opts.dry_run {
                fs::remove_file(&full)?;
            }
        }
    }

    if opts.directories {
        // Look for empty untracked directories
        for entry in walkdir::WalkDir::new(workdir)
            .contents_first(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.starts_with(repo.dft_dir()) || path == workdir {
                continue;
            }
            if entry.file_type().is_dir() {
                // If directory is empty
                if let Ok(mut read) = fs::read_dir(path) {
                    if read.next().is_none() {
                        if let Ok(rel) = path.strip_prefix(workdir) {
                            removed.push(rel.to_path_buf());
                            if !opts.dry_run {
                                let _ = fs::remove_dir(path);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(removed)
}
