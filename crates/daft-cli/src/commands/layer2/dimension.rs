//! CLI handler for `dft dimension` commands.

use crate::cli::{DimensionAction, DimensionArgs};
use crate::error::CliError;
use crate::output::print_output;
use daft_core::Repository;
use daft_dimension::snapshot::SnapshotManager;
use daft_dimension::DimensionManager;
use std::env;
use std::fs;
use std::path::Path;
use std::sync::Arc;

#[allow(unused_imports)]
pub use daft_dimension::DimensionMetadata as DimensionMeta;

pub fn get_current_dimension(dft_dir: &Path) -> String {
    let file = dft_dir.join("current_dimension");
    if let Ok(content) = fs::read_to_string(file) {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    "mainline".to_string()
}

#[allow(dead_code)]
pub fn set_current_dimension(dft_dir: &Path, name: &str) -> std::io::Result<()> {
    fs::write(dft_dir.join("current_dimension"), format!("{}\n", name))
}

pub fn execute(args: DimensionArgs, json: bool) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let manager = DimensionManager::new(Arc::new(repo));
    manager.init()?;

    match args.action {
        DimensionAction::Create { name, from } => {
            manager.create_dimension(&name, from.as_deref(), None)?;
            println!("Created dimension '{}'", name);
        }

        DimensionAction::List => {
            let list = manager.list_dimensions()?;

            if json {
                print_output(true, &list, "");
            } else {
                for item in list {
                    let marker = if item.active { "* " } else { "  " };
                    println!("{}{}\t[{}]", marker, item.name, item.status);
                }
            }
        }

        DimensionAction::Enter { name } => {
            if name != "mainline" && !manager.dimensions_dir().join(&name).exists() {
                return Err(CliError::DimensionNotFound(name));
            }

            let current = manager.current_dimension_name();
            if current == name {
                println!("Already in dimension '{}'", name);
                return Ok(());
            }

            manager.switch_dimension(&name)?;
            println!("Entered dimension '{}'", name);
        }

        DimensionAction::Destroy { name, force } => {
            manager.delete_dimension(&name, force)?;
            println!("Destroyed dimension '{}'", name);
        }

        DimensionAction::Fork { name, from } => {
            manager.fork_dimension(&name, &from)?;
            println!("Forked dimension '{}' from '{}'", name, from);
        }

        DimensionAction::Snapshot { all, name, message } => {
            let repo_ref = manager.repo().as_ref();
            if all {
                let dims = manager.list_dimensions()?;
                let ts = chrono::Utc::now().timestamp_millis();
                for d in dims {
                    let snap_name = format!("snap_{}_{}", d.name, ts);
                    SnapshotManager::create_snapshot(
                        repo_ref,
                        &d.name,
                        &snap_name,
                        message.as_deref(),
                    )?;
                    println!("Snapshot created for dimension '{}'", d.name);
                }
            } else {
                let dim_name = name.unwrap_or_else(|| manager.current_dimension_name());
                let ts = chrono::Utc::now().timestamp_millis();
                let snap_name = format!("snap_{}_{}", dim_name, ts);
                SnapshotManager::create_snapshot(
                    repo_ref,
                    &dim_name,
                    &snap_name,
                    message.as_deref(),
                )?;
                println!("Snapshot created for dimension '{}'", dim_name);
            }
        }

        DimensionAction::Rename { old_name, new_name } => {
            manager.rename_dimension(&old_name, &new_name)?;
            println!("Renamed dimension '{}' to '{}'", old_name, new_name);
        }

        DimensionAction::Info { name } => {
            let meta = manager.read_dimension_metadata(&name)?;

            if json {
                print_output(true, &meta, "");
            } else {
                println!("Dimension: {}", meta.name);
                println!("Creator:   {}", meta.creator);
                println!("Branch:    {}", meta.branch);
                println!("CoW Mode:  {}", meta.cow_mode);
                if let Some(p) = meta.parent {
                    println!("Parent:    {}", p);
                }
            }
        }
    }

    Ok(())
}
