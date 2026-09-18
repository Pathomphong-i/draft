//! Move or rename a file in working tree and index.

use crate::error::DaftError;
use crate::index::IndexEntry;
use crate::repo::Repository;
use std::fs;
use std::path::Path;

pub fn move_path(
    repo: &Repository,
    source: &Path,
    target: &Path,
    force: bool,
) -> Result<(), DaftError> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| DaftError::Config("Cannot move files in a bare repository".into()))?;
    let mut index = repo.index()?;

    let src_rel = if source.is_absolute() {
        source
            .strip_prefix(workdir)
            .map_err(|_| DaftError::Config("Source path is outside repository".into()))?
            .to_string_lossy()
            .replace('\\', "/")
    } else {
        source.to_string_lossy().replace('\\', "/")
    };

    let tgt_rel = if target.is_absolute() {
        target
            .strip_prefix(workdir)
            .map_err(|_| DaftError::Config("Target path is outside repository".into()))?
            .to_string_lossy()
            .replace('\\', "/")
    } else {
        target.to_string_lossy().replace('\\', "/")
    };

    let existing_entry = index
        .find_entry(&src_rel, crate::index::Stage::Normal)
        .cloned()
        .ok_or_else(|| DaftError::Config(format!("bad source, source={}", src_rel)))?;

    let full_src = workdir.join(&src_rel);
    let full_tgt = workdir.join(&tgt_rel);

    if full_tgt.exists() && !force {
        return Err(DaftError::Config(format!(
            "destination exists, source={}, destination={}",
            src_rel, tgt_rel
        )));
    }

    if let Some(parent) = full_tgt.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::rename(&full_src, &full_tgt)?;

    index.remove_path(&src_rel);
    let mut new_entry = IndexEntry::new(
        tgt_rel,
        existing_entry.oid,
        existing_entry.mode,
        existing_entry.stage(),
        existing_entry.file_size,
    )?;
    new_entry.mtime = existing_entry.mtime;
    index.add_entry(new_entry);

    index.write_to(&repo.index_path())?;
    Ok(())
}
