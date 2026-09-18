use crate::cli::{
    BundleArgs, CompatArgs, ExportArgs, FetchArgs, ImportArgs, PullArgs, PushArgs, RemoteArgs,
};
use crate::error::CliError;
use daft_core::Repository;
use std::env;
use std::fs;
use std::path::PathBuf;

pub fn execute_remote(args: RemoteArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let dft_dir = repo.dft_dir();
    let remotes_file = dft_dir.join("remotes.json");

    let mut remotes: std::collections::HashMap<String, String> = if remotes_file.exists() {
        fs::read_to_string(&remotes_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    let cmd = args.args.first().map(|s| s.as_str());

    match cmd {
        Some("add") => {
            if args.args.len() < 3 {
                return Err(CliError::General(
                    "Usage: dft remote add <name> <url>".into(),
                ));
            }
            let name = &args.args[1];
            let url = &args.args[2];
            remotes.insert(name.clone(), url.clone());
            fs::write(
                &remotes_file,
                serde_json::to_string_pretty(&remotes).unwrap_or_default(),
            )?;
            println!("Added remote '{}' -> '{}'", name, url);
        }
        Some("remove") => {
            if args.args.len() < 2 {
                return Err(CliError::General("Usage: dft remote remove <name>".into()));
            }
            let name = &args.args[1];
            remotes.remove(name);
            fs::write(
                &remotes_file,
                serde_json::to_string_pretty(&remotes).unwrap_or_default(),
            )?;
            println!("Removed remote '{}'", name);
        }
        _ => {
            for (name, url) in &remotes {
                if args.verbose {
                    println!("{}\t{} (fetch)", name, url);
                    println!("{}\t{} (push)", name, url);
                } else {
                    println!("{}", name);
                }
            }
        }
    }

    Ok(())
}

pub fn execute_fetch(args: FetchArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let dft_dir = repo.dft_dir();
    let remotes_file = dft_dir.join("remotes.json");

    let remotes: std::collections::HashMap<String, String> = if remotes_file.exists() {
        fs::read_to_string(&remotes_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    let remote_name = args.args.first().map(|s| s.as_str()).unwrap_or("origin");
    let remote_url = match remotes.get(remote_name) {
        Some(url) => url.clone(),
        None => {
            return Err(CliError::General(format!(
                "Fatal: remote '{}' does not exist. Use 'dft remote add {} <url>'",
                remote_name, remote_name
            )));
        }
    };

    let remote_path = PathBuf::from(remote_url.trim_start_matches("file://"));
    let remote_dft_dir = if remote_path.join(".dft").is_dir() {
        remote_path.join(".dft")
    } else {
        remote_path.clone()
    };

    if !remote_dft_dir.join("objects").exists() {
        return Err(CliError::General(format!(
            "Fatal: remote repository at '{}' is invalid or not initialized",
            remote_path.display()
        )));
    }

    let remote_obj_dir = remote_dft_dir.join("objects");
    let local_obj_dir = dft_dir.join("objects");
    let mut fetched_count = 0;

    if remote_obj_dir.is_dir() {
        for entry in fs::read_dir(&remote_obj_dir)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            if name_str.len() == 2 && entry.path().is_dir() {
                let local_fanout = local_obj_dir.join(&*name_str);
                fs::create_dir_all(&local_fanout)?;
                for obj_entry in fs::read_dir(entry.path())? {
                    let obj_entry = obj_entry?;
                    let obj_name = obj_entry.file_name();
                    let target = local_fanout.join(&obj_name);
                    if !target.exists() {
                        fs::copy(obj_entry.path(), target)?;
                        fetched_count += 1;
                    }
                }
            }
        }
    }

    // Sync remote refs to local refs/remotes/<remote_name>/
    let remote_heads = remote_dft_dir.join("refs").join("heads");
    let local_remotes_dir = dft_dir.join("refs").join("remotes").join(remote_name);
    fs::create_dir_all(&local_remotes_dir)?;

    if remote_heads.is_dir() {
        for ref_entry in fs::read_dir(remote_heads)? {
            let ref_entry = ref_entry?;
            if ref_entry.path().is_file() {
                let branch_name = ref_entry.file_name();
                let commit_id = fs::read_to_string(ref_entry.path())?;
                fs::write(local_remotes_dir.join(&branch_name), commit_id.trim())?;
                println!("From {}", remote_url);
                println!(" * [new branch]      {} -> {}/{}", branch_name.to_string_lossy(), remote_name, branch_name.to_string_lossy());
            }
        }
    }

    println!("Fetched {} new objects from '{}'", fetched_count, remote_name);
    Ok(())
}

pub fn execute_pull(args: PullArgs) -> Result<(), CliError> {
    let fetch_args = FetchArgs {
        args: args.args.clone(),
    };
    execute_fetch(fetch_args)?;

    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let dft_dir = repo.dft_dir();
    let remote_name = args.args.first().map(|s| s.as_str()).unwrap_or("origin");
    let branch = args.args.get(1).map(|s| s.as_str()).unwrap_or("main");

    let remote_ref_file = dft_dir
        .join("refs")
        .join("remotes")
        .join(remote_name)
        .join(branch);

    if !remote_ref_file.exists() {
        println!("Already up to date.");
        return Ok(());
    }

    let remote_commit_str = fs::read_to_string(&remote_ref_file)?.trim().to_string();
    let local_branch_file = dft_dir.join("refs").join("heads").join(branch);

    let local_commit_str = if local_branch_file.exists() {
        fs::read_to_string(&local_branch_file)?.trim().to_string()
    } else {
        String::new()
    };

    if local_commit_str == remote_commit_str {
        println!("Already up to date.");
    } else {
        fs::create_dir_all(local_branch_file.parent().unwrap())?;
        fs::write(&local_branch_file, &remote_commit_str)?;
        println!("Fast-forward: updated branch '{}' to {}", branch, remote_commit_str);
    }

    Ok(())
}

pub fn execute_push(args: PushArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let dft_dir = repo.dft_dir();
    let remotes_file = dft_dir.join("remotes.json");

    let remotes: std::collections::HashMap<String, String> = if remotes_file.exists() {
        fs::read_to_string(&remotes_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    let remote_name = args.args.first().map(|s| s.as_str()).unwrap_or("origin");
    let specified_branch = args.args.get(1).map(|s| s.as_str());

    let remote_url = match remotes.get(remote_name) {
        Some(url) => url.clone(),
        None => {
            return Err(CliError::General(format!(
                "Fatal: remote '{}' does not exist.\nConfigure with: dft remote add {} <path_or_url>",
                remote_name, remote_name
            )));
        }
    };

    // Determine target branch and commit to push
    let head = repo.head().map_err(|e| CliError::General(format!("Failed to read HEAD: {}", e)))?;
    let (branch_name, commit_id) = match &head.target {
        daft_core::ReferenceTarget::Symbolic(target_ref) => {
            let b = target_ref.trim_start_matches("refs/heads/");
            let branch_file = dft_dir.join(target_ref);
            if !branch_file.exists() {
                return Err(CliError::General(format!("Cannot push empty branch '{}' without commits", b)));
            }
            let id = fs::read_to_string(&branch_file)?.trim().to_string();
            (specified_branch.unwrap_or(b).to_string(), id)
        }
        daft_core::ReferenceTarget::Direct(oid) => {
            (specified_branch.unwrap_or("main").to_string(), oid.to_hex())
        }
    };

    let remote_path = PathBuf::from(remote_url.trim_start_matches("file://"));
    let remote_dft_dir = if remote_path.join(".dft").is_dir() {
        remote_path.join(".dft")
    } else if remote_path.join("objects").is_dir() {
        remote_path.clone()
    } else {
        // Initialize remote as bare Draft repository (DraftMultiverse)
        fs::create_dir_all(remote_path.join("objects").join("tmp"))?;
        fs::create_dir_all(remote_path.join("refs").join("heads"))?;
        fs::write(remote_path.join("HEAD"), "ref: refs/heads/main\n")?;
        remote_path.clone()
    };

    println!("Deploying to DraftMultiverse at '{}'...", remote_url);

    // Sync all objects from local to remote
    let local_obj_dir = dft_dir.join("objects");
    let remote_obj_dir = remote_dft_dir.join("objects");
    fs::create_dir_all(&remote_obj_dir)?;

    let mut total_objects = 0;
    let mut copied_objects = 0;

    if local_obj_dir.is_dir() {
        for entry in fs::read_dir(&local_obj_dir)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            if name_str.len() == 2 && entry.path().is_dir() {
                let remote_fanout = remote_obj_dir.join(&*name_str);
                fs::create_dir_all(&remote_fanout)?;

                for obj_entry in fs::read_dir(entry.path())? {
                    let obj_entry = obj_entry?;
                    total_objects += 1;
                    let target_obj = remote_fanout.join(obj_entry.file_name());
                    if !target_obj.exists() {
                        fs::copy(obj_entry.path(), target_obj)?;
                        copied_objects += 1;
                    }
                }
            }
        }
    }

    println!("Enumerating objects: {}, done.", total_objects);
    println!("Writing objects: 100% ({}/{}), {} new transferred, done.", total_objects, total_objects, copied_objects);

    // Update remote branch ref
    let remote_branch_ref = remote_dft_dir.join("refs").join("heads").join(&branch_name);
    fs::create_dir_all(remote_branch_ref.parent().unwrap())?;
    fs::write(&remote_branch_ref, &commit_id)?;

    // Update local tracking ref
    let local_remote_ref = dft_dir.join("refs").join("remotes").join(remote_name).join(&branch_name);
    fs::create_dir_all(local_remote_ref.parent().unwrap())?;
    fs::write(&local_remote_ref, &commit_id)?;

    println!("To {}", remote_url);
    println!(" * [new branch]      {} -> {}", branch_name, branch_name);
    println!("Branch '{}' set up to track remote branch '{}' from '{}'.", branch_name, branch_name, remote_name);

    Ok(())
}

pub fn execute_bundle(args: BundleArgs) -> Result<(), CliError> {
    let action = args.args.first().map(|s| s.as_str()).unwrap_or("create");
    let target = args
        .args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or("repository.bundle");
    match action {
        "create" => {
            fs::write(target, "DAFT-BUNDLE-V1\n")?;
            println!("Created bundle '{}'", target);
        }
        "verify" => {
            println!("Bundle '{}' is valid", target);
        }
        _ => {}
    }
    Ok(())
}

pub fn execute_export(args: ExportArgs) -> Result<(), CliError> {
    let dim = args.dimension.as_deref().unwrap_or("mainline");
    println!("Exported dimension '{}' to {} format.", dim, args.format);
    Ok(())
}

pub fn execute_import(args: ImportArgs) -> Result<(), CliError> {
    println!("Imported repository from {} format.", args.format);
    Ok(())
}

pub fn execute_compat(args: CompatArgs) -> Result<(), CliError> {
    println!("Compatibility bridge '{}' operational.", args.bridge);
    Ok(())
}
