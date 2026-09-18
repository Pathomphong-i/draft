//! File removal from index and working tree.

use crate::error::DaftError;
use crate::repo::Repository;
use std::fs;
use std::path::PathBuf;

pub fn remove_paths(
    repo: &Repository,
    pathspecs: &[PathBuf],
    cached: bool,
    _force: bool,
) -> Result<(), DaftError> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| DaftError::Config("Cannot remove files in a bare repository".into()))?;
    let mut index = repo.index()?;

    for spec in pathspecs {
        let rel_str = if spec.is_absolute() {
            spec.strip_prefix(workdir)
                .map_err(|_| DaftError::Config("Path is outside repository".into()))?
                .to_string_lossy()
                .replace('\\', "/")
        } else {
            spec.to_string_lossy().replace('\\', "/")
        };

        let removed_from_index = index.remove_path(&rel_str);
        if !removed_from_index {
            return Err(DaftError::Config(format!(
                "pathspec '{}' did not match any files",
                rel_str
            )));
        }

        if !cached {
            let full_path = workdir.join(&rel_str);
            if full_path.is_file() {
                fs::remove_file(&full_path)?;
            } else if full_path.is_dir() {
                fs::remove_dir_all(&full_path)?;
            }
        }
    }

    index.write_to(&repo.index_path())?;
    Ok(())
}
