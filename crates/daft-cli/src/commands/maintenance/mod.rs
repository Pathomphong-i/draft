use crate::cli::{CountObjectsArgs, FsckArgs, GcArgs, PruneArgs, RepackArgs};
use crate::error::CliError;
use daft_core::maintenance::{count_objects, fsck, gc, prune, repack};
use daft_core::Repository;
use std::env;

pub fn execute_gc(args: GcArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let report = gc(&repo, args.prune)?;
    if args.prune {
        println!(
            "Pruned {} objects, reclaimed {} bytes",
            report.objects_pruned, report.bytes_reclaimed
        );
    }
    Ok(())
}

pub fn execute_fsck(args: FsckArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let report = fsck(&repo, args.full)?;
    if !report.corrupt_objects.is_empty() {
        for (oid, err) in &report.corrupt_objects {
            eprintln!("error: corrupt object {}: {}", oid.to_hex(), err);
        }
        return Err(CliError::General("Repository corruption detected".into()));
    }
    println!(
        "Checked {} objects, repository clean",
        report.objects_checked
    );
    Ok(())
}

pub fn execute_prune(_args: PruneArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let pruned = prune(&repo)?;
    println!("Pruned {} objects", pruned);
    Ok(())
}

pub fn execute_repack(_args: RepackArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    repack(&repo)?;
    println!("Repacking complete.");
    Ok(())
}

pub fn execute_count_objects(_args: CountObjectsArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let rep = count_objects(&repo)?;
    println!("{} objects, {} kilobytes", rep.count, rep.size_bytes / 1024);
    Ok(())
}
