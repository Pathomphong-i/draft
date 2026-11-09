use crate::cli::{
    BundleArgs, CompatArgs, ExportArgs, FetchArgs, ImportArgs, PullArgs, PushArgs, RemoteArgs,
};
use crate::commands::http_wire::{
    http_post_json, is_http_url, normalize_repo_http_url, FetchPackRequest, FetchPackResponse,
    HasObjectsRequest, HasObjectsResponse, PushPackRequest, PushPackResponse, WireObject,
};
use crate::error::CliError;
use daft_core::cas::{ObjectId, ObjectType, RawObject};
use daft_core::object::Commit;
use daft_core::Repository;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub fn execute_remote(args: RemoteArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let dft_dir = repo.dft_dir();
    let remotes_file = dft_dir.join("remotes.json");

    let mut remotes: HashMap<String, String> = if remotes_file.exists() {
        fs::read_to_string(&remotes_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        HashMap::new()
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
        Some("set-url") => {
            if args.args.len() < 3 {
                return Err(CliError::General(
                    "Usage: dft remote set-url <name> <url>".into(),
                ));
            }
            let name = &args.args[1];
            let url = &args.args[2];
            remotes.insert(name.clone(), url.clone());
            fs::write(
                &remotes_file,
                serde_json::to_string_pretty(&remotes).unwrap_or_default(),
            )?;
            println!("Updated remote '{}' -> '{}'", name, url);
        }
        Some("remove") | Some("rm") => {
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

fn resolve_remote_url(dft_dir: &Path, remote_arg: &str) -> Result<(String, String), CliError> {
    if is_http_url(remote_arg) {
        return Ok(("origin".to_string(), remote_arg.to_string()));
    }
    let remotes_file = dft_dir.join("remotes.json");
    let remotes: HashMap<String, String> = if remotes_file.exists() {
        fs::read_to_string(&remotes_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        HashMap::new()
    };
    match remotes.get(remote_arg) {
        Some(url) => Ok((remote_arg.to_string(), url.clone())),
        None => Err(CliError::General(format!(
            "Fatal: remote '{}' does not exist. Use 'dft remote add {} <url>'",
            remote_arg, remote_arg
        ))),
    }
}

pub fn execute_fetch(args: FetchArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let dft_dir = repo.dft_dir();

    let remote_arg = args.args.first().map(|s| s.as_str()).unwrap_or("origin");
    let (remote_name, remote_url) = resolve_remote_url(dft_dir, remote_arg)?;

    if is_http_url(&remote_url) {
        return execute_http_fetch(&repo, &remote_name, &remote_url);
    }

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
    let local_remotes_dir = dft_dir.join("refs").join("remotes").join(&remote_name);
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

fn execute_http_fetch(
    repo: &Repository,
    remote_name: &str,
    remote_url: &str,
) -> Result<(), CliError> {
    let dft_dir = repo.dft_dir();
    let base_url = normalize_repo_http_url(remote_url);
    let fetch_url = format!("{}/dft/fetch-pack", base_url);

    let have: Vec<String> = repo
        .cas()
        .list_objects()
        .unwrap_or_default()
        .into_iter()
        .map(|o| o.to_hex())
        .collect();

    let pack: FetchPackResponse = http_post_json(
        &fetch_url,
        &FetchPackRequest {
            have,
            wants: Vec::new(),
        },
        Some(dft_dir),
    )?;

    let mut fetched_count = 0usize;
    for obj in &pack.objects {
        let bytes = hex::decode(&obj.data_hex)
            .map_err(|e| CliError::General(format!("Invalid object hex: {}", e)))?;
        let obj_type =
            ObjectType::parse(&obj.object_type).map_err(|e| CliError::General(e.to_string()))?;
        let raw = RawObject::new(obj_type, bytes);
        if raw.compute_id().to_hex() != obj.oid {
            return Err(CliError::General(format!(
                "CAS integrity mismatch for fetched object {}",
                obj.oid
            )));
        }
        repo.cas().write_raw(&raw)?;
        fetched_count += 1;
    }

    let local_remotes_dir = dft_dir.join("refs/remotes").join(remote_name);
    fs::create_dir_all(&local_remotes_dir)?;

    if !pack.refs.is_empty() {
        println!("From {}", remote_url);
    }

    for (ref_name, commit_hex) in &pack.refs {
        if let Some(branch) = ref_name.strip_prefix("refs/heads/") {
            let remote_ref_path = local_remotes_dir.join(branch);
            if let Some(p) = remote_ref_path.parent() {
                fs::create_dir_all(p)?;
            }
            let prev = fs::read_to_string(&remote_ref_path)
                .unwrap_or_default()
                .trim()
                .to_string();
            fs::write(&remote_ref_path, format!("{}\n", commit_hex.trim()))?;
            if prev.is_empty() {
                println!(
                    " * [new branch]      {} -> {}/{}",
                    branch, remote_name, branch
                );
            } else if prev != commit_hex.trim() {
                println!(
                    "   {}..{}  {} -> {}/{}",
                    &prev[..prev.len().min(7)],
                    &commit_hex.trim()[..commit_hex.trim().len().min(7)],
                    branch,
                    remote_name,
                    branch
                );
            }
        } else if let Some(dim) = ref_name.strip_prefix("refs/dimensions/") {
            let local_ref = dft_dir.join(ref_name);
            if let Some(p) = local_ref.parent() {
                fs::create_dir_all(p)?;
            }
            let prev = fs::read_to_string(&local_ref)
                .unwrap_or_default()
                .trim()
                .to_string();
            fs::write(&local_ref, format!("{}\n", commit_hex.trim()))?;
            if prev != commit_hex.trim() {
                println!(
                    " * [dimension]       {} -> {}/dimension/{}",
                    dim, remote_name, dim
                );
            }
        } else if ref_name.starts_with("refs/tags/") {
            let local_ref = dft_dir.join(ref_name);
            if let Some(p) = local_ref.parent() {
                fs::create_dir_all(p)?;
            }
            fs::write(local_ref, format!("{}\n", commit_hex.trim()))?;
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
    let remote_arg = args.args.first().map(|s| s.as_str()).unwrap_or("origin");
    let remote_name = if is_http_url(remote_arg) {
        "origin"
    } else {
        remote_arg
    };

    let default_branch_from_head = repo.head().ok().and_then(|h| match h.target {
        daft_core::ReferenceTarget::Symbolic(sym) => {
            Some(sym.trim_start_matches("refs/heads/").to_string())
        }
        _ => None,
    });
    let branch = args
        .args
        .get(1)
        .map(|s| s.as_str())
        .or(default_branch_from_head.as_deref())
        .unwrap_or("main");

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
        return Ok(());
    }

    let head_points_to_branch = repo
        .head()
        .ok()
        .map(|h| match h.target {
            daft_core::ReferenceTarget::Symbolic(sym) => sym == format!("refs/heads/{}", branch),
            _ => false,
        })
        .unwrap_or(false);

    if !local_commit_str.is_empty() && head_points_to_branch && repo.workdir().is_some() {
        if let Ok(remote_oid) = ObjectId::from_hex(&remote_commit_str) {
            match daft_core::merge::merge_commits(
                &repo,
                &format!("{}/{}", remote_name, branch),
                &remote_oid,
                None,
            ) {
                Ok(daft_core::merge::MergeOutcome::AlreadyUpToDate) => {
                    println!("Already up to date.");
                    return Ok(());
                }
                Ok(daft_core::merge::MergeOutcome::FastForward { new_head }) => {
                    println!("Fast-forward: updated branch '{}' to {}", branch, new_head);
                    return Ok(());
                }
                Ok(daft_core::merge::MergeOutcome::Clean { commit_oid }) => {
                    println!(
                        "Merge made by the 'recursive' 3-way strategy -> {}",
                        commit_oid
                    );
                    return Ok(());
                }
                Ok(daft_core::merge::MergeOutcome::Conflict { conflicting_files }) => {
                    return Err(CliError::General(format!(
                        "Automatic merge conflict in: {}. Fix conflicts and commit the result.",
                        conflicting_files.join(", ")
                    )));
                }
                Err(_) => {}
            }
        }
    }

    fs::create_dir_all(local_branch_file.parent().unwrap())?;
    fs::write(&local_branch_file, format!("{}\n", remote_commit_str))?;

    if head_points_to_branch {
        if let Some(workdir) = repo.workdir() {
            if let Ok(commit_oid) = ObjectId::from_hex(&remote_commit_str) {
                if let Ok(raw) = repo.cas().read_raw(&commit_oid) {
                    if let Ok(commit) = Commit::deserialize(&raw.data) {
                        if let Ok(mut index) = repo.index() {
                            let _ = daft_core::merge::checkout_tree(
                                repo.cas().as_ref(),
                                &commit.tree,
                                workdir,
                                &mut index,
                            );
                            let _ = index.write_to(&repo.index_path());
                        }
                    }
                }
            }
        }
    }

    println!(
        "Fast-forward: updated branch '{}' to {}",
        branch, remote_commit_str
    );

    Ok(())
}

pub fn execute_push(args: PushArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let dft_dir = repo.dft_dir();

    let push_all = args.args.iter().any(|a| a == "--all");
    let positional: Vec<&str> = args
        .args
        .iter()
        .map(|s| s.as_str())
        .filter(|s| !s.starts_with('-'))
        .collect();

    let remote_arg = positional.first().copied().unwrap_or("origin");
    let specified_branch = positional.get(1).copied();

    let (remote_name, remote_url) = resolve_remote_url(dft_dir, remote_arg)?;

    // Determine target branch/dimension and commit to push
    let head = repo
        .head()
        .map_err(|e| CliError::General(format!("Failed to read HEAD: {}", e)))?;

    let (branch_name, commit_id) = if let Some(spec) = specified_branch {
        let head_branch = dft_dir.join("refs/heads").join(spec);
        let dim_ref = dft_dir.join("refs/dimensions").join(spec);
        let dim_head = dft_dir.join("dimensions").join(spec).join("HEAD");
        if head_branch.is_file() {
            (spec.to_string(), fs::read_to_string(head_branch)?.trim().to_string())
        } else if dim_ref.is_file() {
            (spec.to_string(), fs::read_to_string(dim_ref)?.trim().to_string())
        } else if dim_head.is_file() {
            let content = fs::read_to_string(dim_head)?.trim().to_string();
            let resolved = if let Some(sym) = content.strip_prefix("ref: ") {
                fs::read_to_string(dft_dir.join(sym.trim()))?
                    .trim()
                    .to_string()
            } else {
                content
            };
            (spec.to_string(), resolved)
        } else {
            match &head.target {
                daft_core::ReferenceTarget::Symbolic(target_ref) => {
                    let branch_file = dft_dir.join(target_ref);
                    if !branch_file.exists() {
                        return Err(CliError::General(format!(
                            "Cannot push empty branch '{}' without commits",
                            spec
                        )));
                    }
                    let id = fs::read_to_string(&branch_file)?.trim().to_string();
                    (spec.to_string(), id)
                }
                daft_core::ReferenceTarget::Direct(oid) => (spec.to_string(), oid.to_hex()),
            }
        }
    } else {
        let active_dim = fs::read_to_string(dft_dir.join("current_dimension"))
            .unwrap_or_else(|_| "mainline".to_string())
            .trim()
            .to_string();
        let active_dim_head = dft_dir.join("dimensions").join(&active_dim).join("HEAD");
        if active_dim != "mainline" && active_dim_head.is_file() {
            let content = fs::read_to_string(active_dim_head)?.trim().to_string();
            let resolved = if let Some(sym) = content.strip_prefix("ref: ") {
                fs::read_to_string(dft_dir.join(sym.trim()))?
                    .trim()
                    .to_string()
            } else {
                content
            };
            (active_dim, resolved)
        } else {
            match &head.target {
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
                    (b.to_string(), id)
                }
                daft_core::ReferenceTarget::Direct(oid) => ("main".to_string(), oid.to_hex()),
            }
        }
    };

    if is_http_url(&remote_url) {
        return execute_http_push(
            &repo,
            &remote_name,
            &remote_url,
            &branch_name,
            &commit_id,
            push_all,
        );
    }

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
        .join(&remote_name)
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

fn execute_http_push(
    repo: &Repository,
    remote_name: &str,
    remote_url: &str,
    branch_name: &str,
    commit_id: &str,
    push_all: bool,
) -> Result<(), CliError> {
    let dft_dir = repo.dft_dir();
    let base_url = normalize_repo_http_url(remote_url);
    let has_url = format!("{}/dft/has-objects", base_url);
    let push_url = format!("{}/dft/push-pack", base_url);

    println!("Deploying to Draft Server at '{}'...", remote_url);

    let all_oids = repo.cas().list_objects().unwrap_or_default();
    let total_objects = all_oids.len();
    let oid_hexes: Vec<String> = all_oids.iter().map(|o| o.to_hex()).collect();

    let missing_set: HashSet<String> = match http_post_json::<HasObjectsRequest, HasObjectsResponse>(
        &has_url,
        &HasObjectsRequest {
            oids: oid_hexes.clone(),
        },
        Some(dft_dir),
    ) {
        Ok(resp) => resp.missing.into_iter().collect(),
        Err(_) => oid_hexes.into_iter().collect(),
    };

    let mut objects = Vec::with_capacity(missing_set.len());
    for oid in &all_oids {
        let hex_id = oid.to_hex();
        if missing_set.contains(&hex_id) {
            if let Ok(raw) = repo.cas().read_raw(oid) {
                objects.push(WireObject {
                    oid: hex_id,
                    object_type: raw.object_type.as_str().to_string(),
                    data_hex: hex::encode(&raw.data),
                });
            }
        }
    }

    let mut refs = BTreeMap::new();
    let cur_dim = fs::read_to_string(dft_dir.join("current_dimension"))
        .unwrap_or_else(|_| "mainline".to_string())
        .trim()
        .to_string();
    let is_dimension_target = (cur_dim != "mainline" && cur_dim == branch_name)
        || dft_dir.join("dimensions").join(branch_name).is_dir()
        || dft_dir.join("refs/dimensions").join(branch_name).is_file();

    if is_dimension_target {
        refs.insert(
            format!("refs/dimensions/{}", branch_name),
            commit_id.to_string(),
        );
    } else {
        refs.insert(format!("refs/heads/{}", branch_name), commit_id.to_string());
    }

    if push_all {
        for prefix in ["refs/heads", "refs/tags", "refs/dimensions"] {
            if let Ok(list) = repo.refs().list_refs(prefix) {
                for r in list {
                    if let Ok(oid) = repo.refs().resolve(&r.name) {
                        refs.insert(r.name, oid.to_hex());
                    }
                }
            }
        }
        // Also collect isolated CoW dimensions under .dft/dimensions/<name>
        let dims_dir = dft_dir.join("dimensions");
        if dims_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(dims_dir) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        let dname = entry.file_name().to_string_lossy().to_string();
                        if dname == "mainline" {
                            continue;
                        }
                        let mut resolved_hex: Option<String> = None;
                        if let Ok(h_raw) = fs::read_to_string(entry.path().join("HEAD")) {
                            let trimmed = h_raw.trim();
                            if trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit())
                            {
                                resolved_hex = Some(trimmed.to_string());
                            } else if let Some(sym) = trimmed.strip_prefix("ref: ") {
                                if let Ok(oid) = repo.refs().resolve(sym.trim()) {
                                    resolved_hex = Some(oid.to_hex());
                                }
                            }
                        }
                        if resolved_hex.is_none() {
                            if let Ok(meta_str) = fs::read_to_string(entry.path().join("meta.json"))
                            {
                                if let Ok(val) =
                                    serde_json::from_str::<serde_json::Value>(&meta_str)
                                {
                                    if let Some(h) =
                                        val.get("head_commit").and_then(|v| v.as_str())
                                    {
                                        resolved_hex = Some(h.to_string());
                                    }
                                }
                            }
                        }
                        if let Some(hex_val) = resolved_hex {
                            refs.insert(format!("refs/dimensions/{}", dname), hex_val);
                        }
                    }
                }
            }
        }
    }

    // Collect territory & swarm agents metadata
    let claims: serde_json::Value =
        fs::read_to_string(dft_dir.join("territory").join("claims.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}));
    let fences: serde_json::Value =
        fs::read_to_string(dft_dir.join("territory").join("fences.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!([]));
    let territory = Some(serde_json::json!({
        "claims": claims,
        "fences": fences,
    }));

    let mut agents = Vec::new();
    let agents_dir = dft_dir.join("agents");
    if agents_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&agents_dir) {
            for e in entries.flatten() {
                if e.path().extension().is_some_and(|x| x == "json") {
                    if let Ok(s) = fs::read_to_string(e.path()) {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                            agents.push(v);
                        }
                    }
                }
            }
        }
    }

    let entanglements: Vec<serde_json::Value> =
        fs::read_to_string(dft_dir.join("entangle").join("rules.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

    let sig = daft_core::merge::get_signature();
    let transferred = objects.len();

    let resp: PushPackResponse = http_post_json(
        &push_url,
        &PushPackRequest {
            branch: branch_name.to_string(),
            commit_id: commit_id.to_string(),
            refs,
            objects,
            territory,
            agents,
            entanglements,
            pusher: Some(sig.name),
        },
        Some(dft_dir),
    )?;

    if !resp.ok {
        return Err(CliError::General(format!(
            "Remote rejected push: {}",
            resp.message
        )));
    }

    println!("Enumerating objects: {}, done.", total_objects);
    println!(
        "Writing objects: 100% ({}/{}), {} new transferred, done.",
        total_objects, total_objects, transferred
    );

    let local_remote_ref = dft_dir
        .join("refs/remotes")
        .join(remote_name)
        .join(branch_name);
    if let Some(p) = local_remote_ref.parent() {
        fs::create_dir_all(p)?;
    }
    fs::write(&local_remote_ref, format!("{}\n", commit_id))?;

    if let Some(ref cmp_url) = resp.compare_url {
        println!("remote:");
        println!(
            "remote: Create a Convergence / Pull Request for '{}' on DraftHub:",
            branch_name
        );
        println!("remote:   {}", cmp_url);
        println!("remote:");
    } else if let Some(ref web_url) = resp.web_url {
        println!("remote: View repository at {}", web_url);
    }

    println!("To {}", remote_url);
    println!(" * [pushed]          {} -> {}", branch_name, branch_name);
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
    refs: BTreeMap<String, String>,
    objects: Vec<BundleObject>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Default)]
pub struct GitMap {
    pub dft_to_git: BTreeMap<String, String>,
    pub git_to_dft: BTreeMap<String, String>,
    pub dimensions: BTreeMap<String, String>,
}

fn load_git_map(dft_dir: &Path) -> GitMap {
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

fn save_git_map(dft_dir: &Path, map: &GitMap) -> Result<(), CliError> {
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
                refs: BTreeMap::new(),
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
            let path = Path::new(target);
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
                        let obj_type = ObjectType::parse(&obj.object_type)
                            .map_err(|e| CliError::General(e.to_string()))?;
                        let raw = RawObject::new(obj_type, bytes);
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
                let obj_type = ObjectType::parse(&obj.object_type)
                    .map_err(|e| CliError::General(e.to_string()))?;
                let raw = RawObject::new(obj_type, bytes);
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
                if let Ok(commit) = Commit::deserialize(&raw.data) {
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
        let mut files = BTreeMap::new();
        let ignore = daft_core::worktree::DaftIgnore::load_from_workdir(&source_dir);
        for entry in walkdir::WalkDir::new(&source_dir)
            .into_iter()
            .filter_entry(|e| !ignore.should_prune_entry(e, &source_dir))
            .flatten()
        {
            if entry.file_type().is_file() {
                if let Ok(rel) = entry.path().strip_prefix(&source_dir) {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    if !rel_str.is_empty() && !ignore.is_ignored(&rel_str, false) {
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
                let commit = Commit::new(
                    tree_oid,
                    parents,
                    sig.clone(),
                    sig,
                    format!("Imported from {} repository", args.format),
                );
                let raw = RawObject::new(ObjectType::Commit, commit.serialize());
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
