use crate::cli::CloneArgs;
use crate::commands::http_wire::{
    extract_repo_name, http_post_json, is_http_url, normalize_repo_http_url, FetchPackRequest,
    FetchPackResponse,
};
use crate::error::CliError;
use daft_core::cas::{ObjectId, ObjectType, RawObject};
use daft_core::checkout::CheckoutEngine;
use daft_core::init::{init, InitOptions};
use daft_core::object::Commit;
use daft_core::Repository;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

pub fn execute(args: CloneArgs) -> Result<(), CliError> {
    if is_http_url(&args.repository) {
        return execute_http_clone(args);
    }

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

fn execute_http_clone(args: CloneArgs) -> Result<(), CliError> {
    let repo_name = extract_repo_name(&args.repository);
    let target = args
        .directory
        .unwrap_or_else(|| env::current_dir().unwrap_or_default().join(&repo_name));

    if target.exists() && fs::read_dir(&target)?.next().is_some() {
        return Err(CliError::General(format!(
            "Destination path '{}' already exists and is not an empty directory.",
            target.display()
        )));
    }

    println!("Cloning into '{}'...", target.display());
    let base_url = normalize_repo_http_url(&args.repository);
    let fetch_url = format!("{}/dft/fetch-pack", base_url);

    let pack: FetchPackResponse = http_post_json(
        &fetch_url,
        &FetchPackRequest {
            have: Vec::new(),
            wants: Vec::new(),
        },
        None,
    )?;

    fs::create_dir_all(&target)?;

    let default_branch = if pack.default_branch.trim().is_empty() {
        "main".to_string()
    } else {
        pack.default_branch.trim().to_string()
    };

    let cloned_repo = init(
        &target,
        &InitOptions {
            bare: false,
            initial_branch: default_branch.clone(),
            reinit: false,
        },
    )?;
    let dft_dir = cloned_repo.dft_dir();

    // Save remote 'origin'
    let mut remotes = HashMap::new();
    remotes.insert("origin".to_string(), args.repository.clone());
    fs::write(
        dft_dir.join("remotes.json"),
        serde_json::to_string_pretty(&remotes).unwrap_or_default(),
    )?;

    // Write verified CAS objects
    let total_objs = pack.objects.len();
    for obj in &pack.objects {
        let bytes = hex::decode(&obj.data_hex)
            .map_err(|e| CliError::General(format!("Invalid object payload hex: {}", e)))?;
        let obj_type =
            ObjectType::parse(&obj.object_type).map_err(|e| CliError::General(e.to_string()))?;
        let raw = RawObject::new(obj_type, bytes);
        if raw.compute_id().to_hex() != obj.oid {
            return Err(CliError::General(format!(
                "CAS integrity mismatch for object {}",
                obj.oid
            )));
        }
        cloned_repo.cas().write_raw(&raw)?;
    }

    // Write remote & local refs
    let remotes_origin_dir = dft_dir.join("refs/remotes/origin");
    fs::create_dir_all(&remotes_origin_dir)?;

    let mut chosen_branch = default_branch.clone();
    let mut chosen_commit: Option<String> = None;

    for (ref_name, commit_hex) in &pack.refs {
        if let Some(branch) = ref_name.strip_prefix("refs/heads/") {
            let remote_ref_path = remotes_origin_dir.join(branch);
            if let Some(p) = remote_ref_path.parent() {
                fs::create_dir_all(p)?;
            }
            fs::write(&remote_ref_path, format!("{}\n", commit_hex.trim()))?;
            if branch == default_branch || chosen_commit.is_none() {
                chosen_branch = branch.to_string();
                chosen_commit = Some(commit_hex.trim().to_string());
            }
        } else if ref_name.starts_with("refs/tags/") || ref_name.starts_with("refs/dimensions/") {
            let local_ref_path = dft_dir.join(ref_name);
            if let Some(p) = local_ref_path.parent() {
                fs::create_dir_all(p)?;
            }
            fs::write(&local_ref_path, format!("{}\n", commit_hex.trim()))?;
        }
    }

    if let Some(ref commit_hex) = chosen_commit {
        let head_branch_path = dft_dir.join("refs/heads").join(&chosen_branch);
        if let Some(p) = head_branch_path.parent() {
            fs::create_dir_all(p)?;
        }
        fs::write(&head_branch_path, format!("{}\n", commit_hex))?;
        fs::write(
            dft_dir.join("HEAD"),
            format!("ref: refs/heads/{}\n", chosen_branch),
        )?;

        if let Ok(commit_oid) = ObjectId::from_hex(commit_hex) {
            if let Ok(raw) = cloned_repo.cas().read_raw(&commit_oid) {
                if let Ok(commit) = Commit::deserialize(&raw.data) {
                    let mut index = cloned_repo.index()?;
                    daft_core::merge::checkout_tree(
                        cloned_repo.cas().as_ref(),
                        &commit.tree,
                        &target,
                        &mut index,
                    )?;
                    index.write_to(&cloned_repo.index_path())?;
                }
            }
        }
        println!(
            "remote: Enumerating objects: {}, done.\nReceiving objects: 100% ({}/{}), done.\nBranch '{}' set up to track 'origin/{}'.",
            total_objs, total_objs, total_objs, chosen_branch, chosen_branch
        );
    } else {
        println!("warning: You appear to have cloned an empty Draft repository.");
    }

    if let Some(territory) = pack.territory {
        let terr_dir = dft_dir.join("territory");
        let _ = fs::create_dir_all(&terr_dir);
        if let Some(claims) = territory.get("claims") {
            let _ = fs::write(
                terr_dir.join("claims.json"),
                serde_json::to_string_pretty(claims).unwrap_or_default(),
            );
        }
        if let Some(fences) = territory.get("fences") {
            let _ = fs::write(
                terr_dir.join("fences.json"),
                serde_json::to_string_pretty(fences).unwrap_or_default(),
            );
        }
    }

    if !pack.agents.is_empty() {
        let agents_dir = dft_dir.join("agents");
        let _ = fs::create_dir_all(&agents_dir);
        for agent in &pack.agents {
            if let Some(id) = agent
                .get("id")
                .or_else(|| agent.get("name"))
                .and_then(|v| v.as_str())
            {
                let _ = fs::write(
                    agents_dir.join(format!("{}.json", id)),
                    serde_json::to_string_pretty(agent).unwrap_or_default(),
                );
            }
        }
    }

    if !pack.entanglements.is_empty() {
        let ent_dir = dft_dir.join("entangle");
        let _ = fs::create_dir_all(&ent_dir);
        let _ = fs::write(
            ent_dir.join("rules.json"),
            serde_json::to_string_pretty(&pack.entanglements).unwrap_or_default(),
        );
    }

    // Materialize remote Parallel Dimensions as isolated CoW workspaces with exact commit trees
    let repo_arc = std::sync::Arc::new(cloned_repo);
    let dim_mgr = daft_dimension::DimensionManager::new(std::sync::Arc::clone(&repo_arc));
    let _ = dim_mgr.init();
    let mut synced_dims = 0usize;
    for (ref_name, commit_hex) in &pack.refs {
        if let Some(dim_name) = ref_name.strip_prefix("refs/dimensions/") {
            if dim_name != "mainline" && dim_name != chosen_branch {
                if dim_mgr
                    .create_dimension(dim_name, Some(commit_hex.trim()), Some("remote-sync"))
                    .is_ok()
                {
                    if let Ok(d_oid) = ObjectId::from_hex(commit_hex.trim()) {
                        if let Ok(raw) = repo_arc.cas().read_raw(&d_oid) {
                            if let Ok(d_commit) = Commit::deserialize(&raw.data) {
                                let dim_dir = dim_mgr.dimensions_dir().join(dim_name);
                                let ws_dir = dim_dir.join("workspace");
                                let _ = fs::remove_dir_all(&ws_dir);
                                let _ = fs::create_dir_all(&ws_dir);
                                let mut dim_idx = daft_core::Index::new();
                                let _ = daft_core::merge::checkout_tree(
                                    repo_arc.cas().as_ref(),
                                    &d_commit.tree,
                                    &ws_dir,
                                    &mut dim_idx,
                                );
                                let _ = dim_idx.write_to(&dim_dir.join("index"));
                            }
                        }
                    }
                    synced_dims += 1;
                }
            }
        }
    }
    if synced_dims > 0 {
        println!(
            "Materialized {} parallel dimension workspace(s) via CoW.",
            synced_dims
        );
    }

    Ok(())
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    let cow_strategy = daft_dimension::cow::detect_best_strategy(dst);
    daft_dimension::cow::CowEngine::clone_dir(src, dst, cow_strategy)
}
