use crate::cli::CloneArgs;
use crate::error::CliError;
use daft_core::checkout::CheckoutEngine;
use daft_core::Repository;
use std::env;
use std::fs;
use std::path::PathBuf;

pub fn execute(args: CloneArgs) -> Result<(), CliError> {
    let source = PathBuf::from(&args.repository);
    let target = args.directory.unwrap_or_else(|| {
        let name = source
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "cloned_repo".to_string());
        env::current_dir().unwrap_or_default().join(name)
    });

    if target.exists() && fs::read_dir(&target)?.next().is_some() {
        return Err(CliError::General(format!(
            "Destination path '{}' already exists and is not an empty directory.",
            target.display()
        )));
    }

    fs::create_dir_all(&target)?;

    let source_repo = Repository::discover(&source)?;
    let target_dft = target.join(".dft");

    // Copy .dft directory
    copy_dir_recursive(source_repo.dft_dir(), &target_dft)?;

    let cloned_repo = Repository::open(&target_dft)?;
    let engine = CheckoutEngine::new(&cloned_repo);
    let _ = engine.checkout_commit("HEAD");

    println!("Cloned into '{}'...", target.display());
    Ok(())
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
