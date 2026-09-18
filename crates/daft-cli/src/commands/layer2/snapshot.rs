//! CLI handler for `dft snapshot` commands.

use crate::cli::{SnapshotAction, SnapshotArgs};
use crate::error::CliError;
use crate::output::print_output;
use daft_core::Repository;
use daft_dimension::snapshot::{SnapshotListItem, SnapshotManager};
use std::env;

pub fn execute(args: SnapshotArgs, json: bool) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let dft_dir = repo.dft_dir();
    let current_dim = crate::commands::layer2::dimension::get_current_dimension(dft_dir);

    match args.action {
        SnapshotAction::Create {
            name,
            dimension,
            message,
        } => {
            let target_dim = dimension.as_deref().unwrap_or(&current_dim);
            let snap =
                SnapshotManager::create_snapshot(&repo, target_dim, &name, message.as_deref())?;

            if json {
                print_output(true, &snap, "");
            } else {
                println!(
                    "Created snapshot '{}' [{}] for dimension '{}' (tree: {})",
                    snap.name,
                    snap.snapshot_id,
                    snap.dimension_id,
                    &snap.tree_oid.to_hex()[..8]
                );
            }
        }

        SnapshotAction::List {
            dimension,
            porcelain,
        } => {
            let snaps = SnapshotManager::list_snapshots(&repo, dimension.as_deref())?;

            if json {
                let items: Vec<SnapshotListItem> = snaps.iter().map(|s| s.to_list_item()).collect();
                print_output(true, &items, "");
            } else if porcelain {
                for s in &snaps {
                    println!(
                        "{}\t{}\t{}\t{}\t{}\t{}",
                        s.snapshot_id,
                        s.name,
                        s.dimension_id,
                        s.tree_oid.to_hex(),
                        s.created_at.to_rfc3339(),
                        s.description
                    );
                }
            } else if snaps.is_empty() {
                println!("No snapshots found.");
            } else {
                println!(
                    "{:<28} {:<16} {:<14} {:<10} {:<24} DESCRIPTION",
                    "SNAPSHOT ID", "NAME", "DIMENSION", "TREE", "CREATED"
                );
                for s in &snaps {
                    println!(
                        "{:<28} {:<16} {:<14} {:<10} {:<24} {}",
                        s.snapshot_id,
                        s.name,
                        s.dimension_id,
                        &s.tree_oid.to_hex()[..8],
                        s.created_at.format("%Y-%m-%d %H:%M:%S"),
                        s.description
                    );
                }
            }
        }

        SnapshotAction::Restore {
            name,
            dimension,
            force,
        } => {
            let target_dim = dimension.as_deref().unwrap_or(&current_dim);
            let snap = SnapshotManager::restore_snapshot(&repo, target_dim, &name, force)?;

            if json {
                print_output(true, &snap, "");
            } else {
                println!(
                    "Restored dimension '{}' to snapshot '{}' (tree: {})",
                    target_dim,
                    snap.name,
                    &snap.tree_oid.to_hex()[..8]
                );
            }
        }

        SnapshotAction::Delete { name, dimension } => {
            let target_dim = dimension.as_deref().unwrap_or(&current_dim);
            let snap = SnapshotManager::delete_snapshot(&repo, target_dim, &name)?;

            if json {
                print_output(true, &snap, "");
            } else {
                println!(
                    "Deleted snapshot '{}' from dimension '{}'",
                    snap.name, target_dim
                );
            }
        }
    }

    Ok(())
}
