use crate::cli::AddArgs;
use crate::error::CliError;
use daft_awareness::TerritoryManager;
use daft_core::index::Stage;
use daft_core::worktree::add_paths;
use daft_core::Repository;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn is_path_fenced(dft_dir: &Path, rel_path: &str, current_dim: Option<&str>) -> bool {
    let territory_mgr = TerritoryManager::new(dft_dir);
    if let Ok(fences) = territory_mgr.list_fences() {
        let norm = rel_path.trim_start_matches("./").replace('\\', "/");
        for f in fences {
            if f.hard {
                if let Some(dim) = current_dim {
                    if f.is_violated_by(&norm, dim) {
                        return true;
                    }
                } else if daft_awareness::matches_glob(&f.path_glob, &norm) {
                    return true;
                }
            }
        }
    }
    false
}

pub fn check_fence_violations(
    repo: &Repository,
    pathspecs: &[PathBuf],
    all: bool,
    current_dim: &str,
) -> Result<(), CliError> {
    let territory_mgr = TerritoryManager::new(repo.dft_dir());
    let fences = match territory_mgr.list_fences() {
        Ok(f) => f,
        Err(_) => return Ok(()),
    };
    if fences.is_empty() {
        return Ok(());
    }

    let workdir = match repo.workdir() {
        Some(w) => w,
        None => return Ok(()),
    };

    let check_path = |rel_str: &str| -> Result<(), CliError> {
        let norm = rel_str.trim_start_matches("./").replace('\\', "/");
        for f in &fences {
            if f.hard && f.is_violated_by(&norm, current_dim) {
                return Err(CliError::FenceViolation(norm));
            }
        }
        Ok(())
    };

    if all || pathspecs.iter().any(|p| p.as_os_str() == ".") {
        let index = repo.index().ok();
        for entry in WalkDir::new(workdir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !workdir.starts_with(repo.dft_dir()) && path.starts_with(repo.dft_dir()) {
                continue;
            }
            if entry.file_type().is_file() {
                if let Ok(rel) = path.strip_prefix(workdir) {
                    let rel_str = rel.to_string_lossy();
                    let norm = rel_str.trim_start_matches("./").replace('\\', "/");

                    let is_candidate = match &index {
                        Some(idx) => match idx.find_entry(&norm, Stage::Normal) {
                            Some(ie) => {
                                if let Ok(meta) = fs::metadata(path) {
                                    if meta.len() as u32 != ie.file_size {
                                        true
                                    } else if let Ok(data) = fs::read(path) {
                                        let raw = daft_core::cas::RawObject::new(
                                            daft_core::cas::ObjectType::Blob,
                                            data,
                                        );
                                        let oid = raw.compute_id();
                                        oid != ie.oid
                                    } else {
                                        true
                                    }
                                } else {
                                    true
                                }
                            }
                            None => true,
                        },
                        None => true,
                    };

                    if is_candidate {
                        check_path(&rel_str)?;
                    }
                }
            }
        }
    } else {
        for spec in pathspecs {
            let full = if spec.is_absolute() {
                spec.clone()
            } else {
                workdir.join(spec)
            };

            if full.is_dir() {
                for entry in WalkDir::new(&full).into_iter().filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if !workdir.starts_with(repo.dft_dir()) && path.starts_with(repo.dft_dir()) {
                        continue;
                    }
                    if entry.file_type().is_file() {
                        if let Ok(rel) = path.strip_prefix(workdir) {
                            check_path(&rel.to_string_lossy())?;
                        }
                    }
                }
            } else {
                let rel_str = if let Ok(rel) = full.strip_prefix(workdir) {
                    rel.to_string_lossy().to_string()
                } else {
                    spec.to_string_lossy().to_string()
                };
                check_path(&rel_str)?;
            }
        }
    }

    Ok(())
}

pub fn execute(args: AddArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let current_dim = repo.dimension_name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| crate::commands::layer2::dimension::get_current_dimension(repo.dft_dir()));

    check_fence_violations(&repo, &args.pathspecs, args.all, &current_dim)?;

    add_paths(&repo, &args.pathspecs, args.all, args.update)?;
    Ok(())
}
