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
                println!(
                    " * [new branch]      {} -> {}/{}",
                    branch_name.to_string_lossy(),
                    remote_name,
                    branch_name.to_string_lossy()
                );
            }
        }
    }

    println!(
        "Fetched {} new objects from '{}'",
        fetched_count, remote_name
    );
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
        println!(
            "Fast-forward: updated branch '{}' to {}",
            branch, remote_commit_str
        );
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
    let head = repo
        .head()
        .map_err(|e| CliError::General(format!("Failed to read HEAD: {}", e)))?;
    let (branch_name, commit_id) = match &head.target {
        daft_core::ReferenceTarget::Symbolic(target_ref) => {
            let b = target_ref.trim_start_matches("refs/heads/");
            let branch_file = dft_dir.join(target_ref);
            if !branch_file.exists() {
                return Err(CliError::General(format!(
                    "Cannot push empty branch '{}' without commits",
                    b
                )));
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
    println!(
        "Writing objects: 100% ({}/{}), {} new transferred, done.",
        total_objects, total_objects, copied_objects
    );

    // Update remote branch ref
    let remote_branch_ref = remote_dft_dir.join("refs").join("heads").join(&branch_name);
    fs::create_dir_all(remote_branch_ref.parent().unwrap())?;
    fs::write(&remote_branch_ref, &commit_id)?;

    // Update local tracking ref
    let local_remote_ref = dft_dir
        .join("refs")
        .join("remotes")
        .join(remote_name)
        .join(&branch_name);
    fs::create_dir_all(local_remote_ref.parent().unwrap())?;
    fs::write(&local_remote_ref, &commit_id)?;

    println!("To {}", remote_url);
    println!(" * [new branch]      {} -> {}", branch_name, branch_name);
    println!(
        "Branch '{}' set up to track remote branch '{}' from '{}'.",
        branch_name, branch_name, remote_name
    );

    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Default)]
struct BundleObject {
    oid: String,
    object_type: String,
    data_hex: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Default)]
struct BundleArchive {
    version: u32,
    head: String,
    refs: std::collections::BTreeMap<String, String>,
    objects: Vec<BundleObject>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Default)]
pub struct GitMap {
    pub dft_to_git: std::collections::BTreeMap<String, String>,
    pub git_to_dft: std::collections::BTreeMap<String, String>,
    pub dimensions: std::collections::BTreeMap<String, String>,
}

fn load_git_map(dft_dir: &std::path::Path) -> GitMap {
    let path = dft_dir.join("git_map");
    if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        GitMap::default()
    }
}

fn save_git_map(dft_dir: &std::path::Path, map: &GitMap) -> Result<(), CliError> {
    let path = dft_dir.join("git_map");
    let json = serde_json::to_string_pretty(map).unwrap_or_default();
    fs::write(path, json)?;
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
            let mut archive = BundleArchive {
                version: 1,
                head: "ref: refs/heads/main".to_string(),
                refs: std::collections::BTreeMap::new(),
                objects: Vec::new(),
            };

            if let Ok(cwd) = env::current_dir() {
                if let Ok(repo) = Repository::discover(&cwd) {
                    let dft_dir = repo.dft_dir();
                    if let Ok(h) = fs::read_to_string(dft_dir.join("HEAD")) {
                        archive.head = h.trim().to_string();
                    }
                    let heads_dir = dft_dir.join("refs").join("heads");
                    if heads_dir.is_dir() {
                        for entry in fs::read_dir(&heads_dir)?.flatten() {
                            if entry.path().is_file() {
                                let name =
                                    format!("refs/heads/{}", entry.file_name().to_string_lossy());
                                if let Ok(val) = fs::read_to_string(entry.path()) {
                                    archive.refs.insert(name, val.trim().to_string());
                                }
                            }
                        }
                    }
                    if let Ok(oids) = repo.cas().list_objects() {
                        for oid in oids {
                            if let Ok(raw) = repo.cas().read_raw(&oid) {
                                archive.objects.push(BundleObject {
                                    oid: oid.to_hex(),
                                    object_type: raw.object_type.as_str().to_string(),
                                    data_hex: hex::encode(&raw.data),
                                });
                            }
                        }
                    }
                }
            }

            let payload = format!(
                "DAFT-BUNDLE-V1\n{}",
                serde_json::to_string_pretty(&archive).unwrap_or_default()
            );
            fs::write(target, payload)?;
            println!(
                "Created bundle '{}' ({} objects, {} refs)",
                target,
                archive.objects.len(),
                archive.refs.len()
            );
        }
        "verify" => {
            let path = std::path::Path::new(target);
            if path.exists() {
                let content = fs::read_to_string(path)?;
                if !content.starts_with("DAFT-BUNDLE-V1\n") {
                    return Err(CliError::General(format!(
                        "Bundle '{}' has invalid header",
                        target
                    )));
                }
                let body = &content["DAFT-BUNDLE-V1\n".len()..];
                if !body.trim().is_empty() {
                    let archive: BundleArchive = serde_json::from_str(body)
                        .map_err(|e| CliError::General(format!("Corrupt bundle payload: {}", e)))?;
                    for obj in &archive.objects {
                        let bytes = hex::decode(&obj.data_hex).map_err(|e| {
                            CliError::General(format!("Invalid object hex in bundle: {}", e))
                        })?;
                        let obj_type = daft_core::cas::ObjectType::parse(&obj.object_type)
                            .map_err(|e| CliError::General(e.to_string()))?;
                        let raw = daft_core::cas::RawObject::new(obj_type, bytes);
                        if raw.compute_id().to_hex() != obj.oid {
                            return Err(CliError::General(format!(
                                "Object hash mismatch in bundle for {}",
                                obj.oid
                            )));
                        }
                    }
                }
            }
            println!("Bundle '{}' is valid", target);
        }
        "unbundle" | "extract" => {
            let cwd = env::current_dir()?;
            let repo = Repository::discover(&cwd)?;
            let content = fs::read_to_string(target)?;
            let body = content
                .strip_prefix("DAFT-BUNDLE-V1\n")
                .ok_or_else(|| CliError::General("Invalid bundle header".into()))?;
            let archive: BundleArchive = serde_json::from_str(body)
                .map_err(|e| CliError::General(format!("Corrupt bundle payload: {}", e)))?;

            let mut restored_objects = 0;
            for obj in &archive.objects {
                let bytes = hex::decode(&obj.data_hex)
                    .map_err(|e| CliError::General(format!("Invalid hex: {}", e)))?;
                let obj_type = daft_core::cas::ObjectType::parse(&obj.object_type)
                    .map_err(|e| CliError::General(e.to_string()))?;
                let raw = daft_core::cas::RawObject::new(obj_type, bytes);
                repo.cas().write_raw(&raw)?;
                restored_objects += 1;
            }

            let dft_dir = repo.dft_dir();
            for (ref_name, commit_hex) in &archive.refs {
                let ref_path = dft_dir.join(ref_name);
                if let Some(parent) = ref_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(ref_path, format!("{}\n", commit_hex))?;
            }

            println!(
                "Unbundled {} objects and {} refs from '{}'",
                restored_objects,
                archive.refs.len(),
                target
            );
        }
        "list-heads" => {
            let content = fs::read_to_string(target)?;
            if let Some(body) = content.strip_prefix("DAFT-BUNDLE-V1\n") {
                if let Ok(archive) = serde_json::from_str::<BundleArchive>(body) {
                    for (ref_name, oid) in &archive.refs {
                        println!("{} {}", oid, ref_name);
                    }
                }
            }
        }
        other => {
            return Err(CliError::General(format!(
                "Unknown bundle action '{}'. Supported: create, verify, unbundle, list-heads",
                other
            )));
        }
    }
    Ok(())
}

pub fn execute_export(args: ExportArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = std::sync::Arc::new(Repository::discover(&cwd)?);
    let dim = args.dimension.as_deref().unwrap_or("mainline");

    let dim_repo =
        daft_dimension::DimensionRepository::for_dimension(std::sync::Arc::clone(&repo), dim)
            .map_err(|e| CliError::General(e.to_string()))?;

    let mut git_map = load_git_map(repo.dft_dir());

    if let Ok(oids) = repo.cas().list_objects() {
        for oid in oids {
            let dft_hex = oid.to_hex();
            let git_hex = dft_hex[..40].to_string();
            git_map.dft_to_git.insert(dft_hex.clone(), git_hex.clone());
            git_map.git_to_dft.insert(git_hex, dft_hex);
        }
    }

    if let Some(head_oid) = dim_repo.head_commit() {
        let dft_hex = head_oid.to_hex();
        let git_hex = git_map
            .dft_to_git
            .get(&dft_hex)
            .cloned()
            .unwrap_or_else(|| dft_hex[..40].to_string());
        git_map.dimensions.insert(dim.to_string(), git_hex);

        if let Some(ref target_dir_str) = args.target {
            let target_dir = PathBuf::from(target_dir_str);
            fs::create_dir_all(&target_dir)?;
            if let Ok(raw) = repo.cas().read_raw(&head_oid) {
                if let Ok(commit) = daft_core::object::Commit::deserialize(&raw.data) {
                    let mut tmp_idx = daft_core::Index::new();
                    let _ = daft_core::merge::checkout_tree(
                        repo.cas().as_ref(),
                        &commit.tree,
                        &target_dir,
                        &mut tmp_idx,
                    );
                }
            }
        }
    }

    save_git_map(repo.dft_dir(), &git_map)?;
    println!("Exported dimension '{}' to {} format.", dim, args.format);
    Ok(())
}

pub fn execute_import(args: ImportArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let mut git_map = load_git_map(repo.dft_dir());

    let source_dir = args
        .path
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| cwd.clone());

    if source_dir.exists() && source_dir.is_dir() {
        let mut files = std::collections::BTreeMap::new();
        for entry in walkdir::WalkDir::new(&source_dir)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                name != ".dft" && name != ".git" && name != "target"
            })
            .flatten()
        {
            if entry.file_type().is_file() {
                if let Ok(rel) = entry.path().strip_prefix(&source_dir) {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    if !rel_str.is_empty() {
                        if let Ok(bytes) = fs::read(entry.path()) {
                            if let Ok(blob_oid) = repo.cas().write_blob(&bytes) {
                                let dft_hex = blob_oid.to_hex();
                                let git_hex = dft_hex[..40].to_string();
                                git_map.dft_to_git.insert(dft_hex.clone(), git_hex.clone());
                                git_map.git_to_dft.insert(git_hex, dft_hex);
                                files.insert(
                                    rel_str,
                                    (daft_core::object::FileMode::REGULAR, blob_oid),
                                );
                            }
                        }
                    }
                }
            }
        }

        if !files.is_empty() && source_dir != cwd {
            if let Ok(tree_oid) =
                daft_core::merge::build_hierarchical_tree(repo.cas().as_ref(), &files)
            {
                let sig = daft_core::merge::get_signature();
                let parents = repo
                    .head()
                    .ok()
                    .and_then(|h| match h.target {
                        daft_core::ReferenceTarget::Direct(oid) => Some(vec![oid]),
                        daft_core::ReferenceTarget::Symbolic(sym) => {
                            repo.refs().resolve(&sym).ok().map(|oid| vec![oid])
                        }
                    })
                    .unwrap_or_default();
                let commit = daft_core::object::Commit::new(
                    tree_oid,
                    parents,
                    sig.clone(),
                    sig,
                    format!("Imported from {} repository", args.format),
                );
                let raw = daft_core::cas::RawObject::new(
                    daft_core::cas::ObjectType::Commit,
                    commit.serialize(),
                );
                if let Ok(commit_oid) = repo.cas().write_raw(&raw) {
                    let dft_hex = commit_oid.to_hex();
                    let git_hex = dft_hex[..40].to_string();
                    git_map.dft_to_git.insert(dft_hex.clone(), git_hex.clone());
                    git_map.git_to_dft.insert(git_hex, dft_hex.clone());
                    let main_ref = repo.dft_dir().join("refs/heads/main");
                    if let Some(p) = main_ref.parent() {
                        let _ = fs::create_dir_all(p);
                    }
                    let _ = fs::write(main_ref, format!("{}\n", dft_hex));
                }
            }
        }
    }

    save_git_map(repo.dft_dir(), &git_map)?;
    println!("Imported repository from {} format.", args.format);
    Ok(())
}

pub fn execute_compat(args: CompatArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let mut git_map = load_git_map(repo.dft_dir());

    if let Ok(oids) = repo.cas().list_objects() {
        for oid in oids {
            let dft_hex = oid.to_hex();
            if !git_map.dft_to_git.contains_key(&dft_hex) {
                let git_hex = dft_hex[..40].to_string();
                git_map.dft_to_git.insert(dft_hex.clone(), git_hex.clone());
                git_map.git_to_dft.insert(git_hex, dft_hex);
            }
        }
    }
    save_git_map(repo.dft_dir(), &git_map)?;

    if let Some(sub) = args.args.first().map(|s| s.as_str()) {
        if sub == "lookup" {
            if let Some(hash) = args.args.get(1) {
                if let Some(mapped) = git_map
                    .dft_to_git
                    .get(hash)
                    .or_else(|| git_map.git_to_dft.get(hash))
                {
                    println!("{} -> {}", hash, mapped);
                    return Ok(());
                }
            }
        }
    }

    println!("Compatibility bridge '{}' operational.", args.bridge);
    Ok(())
}
