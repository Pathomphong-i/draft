use crate::cli::CommitArgs;
use crate::commands::add::is_path_fenced;
use crate::error::CliError;
use daft_core::cas::{ObjectType, RawObject};
use daft_core::diff::tree::flatten_tree;
use daft_core::merge::get_signature;
use daft_core::object::{Commit, FileMode, Signature};
use daft_core::plumbing::write_tree;
use daft_core::refs::{peel_reference, ReferenceTarget};
use daft_core::worktree::add_paths;
use daft_core::Repository;
use std::collections::BTreeMap;
use std::env;
use std::fs;

pub fn execute(args: CommitArgs, quiet: bool) -> Result<(), CliError> {
    let msg = match &args.message {
        Some(m) if !m.trim().is_empty() => m.clone(),
        _ => return Err(CliError::EmptyMessage),
    };

    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;

    // If -a flag, stage modified and deleted files
    if args.all {
        let _ = add_paths(&repo, &[], false, true);
    }

    let index = repo.index()?;

    let current_dim = crate::commands::layer2::dimension::get_current_dimension(repo.dft_dir());

    // Check fences
    for entry in index.entries() {
        if is_path_fenced(repo.dft_dir(), &entry.path, Some(&current_dim)) {
            return Err(CliError::FenceViolation(entry.path.clone()));
        }
    }

    let is_mainline = current_dim == "mainline";
    let dim_dir = repo.dft_dir().join("dimensions").join(&current_dim);

    let head_oid_opt = if !is_mainline && dim_dir.exists() {
        if let Ok(content) = fs::read_to_string(dim_dir.join("HEAD")) {
            let trimmed = content.trim();
            if let Ok(oid) = daft_core::cas::ObjectId::from_hex(trimmed) {
                Some(oid)
            } else if let Some(sym) = trimmed.strip_prefix("ref: ") {
                repo.refs().resolve(sym).ok()
            } else {
                peel_reference(repo.dft_dir(), "HEAD").ok()
            }
        } else {
            peel_reference(repo.dft_dir(), "HEAD").ok()
        }
    } else {
        peel_reference(repo.dft_dir(), "HEAD").ok()
    };

    // Check for empty commit
    if !args.allow_empty {
        let tree_matches = if let Some(head_oid) = head_oid_opt {
            let head_raw = repo.cas().read_raw(&head_oid)?;
            let head_commit = Commit::deserialize(&head_raw.data)?;

            let mut head_files = BTreeMap::new();
            flatten_tree(repo.cas().as_ref(), &head_commit.tree, "", &mut head_files)?;

            let mut index_files = BTreeMap::new();
            for e in index.entries() {
                index_files.insert(e.path.clone(), (FileMode(e.mode), e.oid));
            }

            head_files == index_files
        } else {
            index.entries().is_empty()
        };

        if tree_matches {
            return Err(CliError::EmptyCommit);
        }
    }

    // Resolve author signature
    let sig = if let Some(author_str) = &args.author {
        if let Some((name, email)) = parse_author_str(author_str) {
            Signature::now(name, email)
        } else {
            Signature::now(author_str.clone(), "unknown@draft-vcs.org".to_string())
        }
    } else {
        get_signature()
    };

    // Write tree from index
    let tree_oid = write_tree(&repo)?;

    // Parents
    let mut parents = Vec::new();
    if let Some(h) = head_oid_opt {
        parents.push(h);
    }

    // Check MERGE_HEAD
    let merge_head_path = repo.dft_dir().join("MERGE_HEAD");
    if merge_head_path.exists() {
        if let Ok(content) = fs::read_to_string(&merge_head_path) {
            if let Ok(p2) = daft_core::cas::ObjectId::from_hex(content.trim()) {
                if !parents.contains(&p2) {
                    parents.push(p2);
                }
            }
        }
    }

    let commit = Commit::new(tree_oid, parents, sig.clone(), sig, &msg);
    let serialized = commit.serialize();
    let raw = RawObject::new(ObjectType::Commit, serialized);
    let commit_oid = repo.cas().write_raw(&raw)?;

    // Update active ref or HEAD
    let branch_name = if !is_mainline && dim_dir.exists() {
        let _ = fs::write(dim_dir.join("HEAD"), format!("{}\n", commit_oid.to_hex()));
        let meta_path = dim_dir.join("meta.json");
        if meta_path.exists() {
            if let Ok(mut val) = fs::read_to_string(&meta_path).and_then(|s| {
                serde_json::from_str::<serde_json::Value>(&s)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
            }) {
                val["head_commit"] = serde_json::Value::String(commit_oid.to_hex());
                let _ = fs::write(
                    &meta_path,
                    serde_json::to_string_pretty(&val).unwrap_or_default(),
                );
            }
        }
        format!("dimension:{}", current_dim)
    } else {
        let head_ref = repo.head()?;
        match &head_ref.target {
            ReferenceTarget::Symbolic(sym) => {
                repo.refs()
                    .write_ref(sym, &ReferenceTarget::Direct(commit_oid), None, None)?;
                sym.strip_prefix("refs/heads/").unwrap_or(sym).to_string()
            }
            ReferenceTarget::Direct(_) => {
                repo.set_head(&ReferenceTarget::Direct(commit_oid))?;
                "detached".to_string()
            }
        }
    };

    // Clean up MERGE_HEAD and MERGE_MSG
    if merge_head_path.exists() {
        let _ = fs::remove_file(merge_head_path);
    }
    let merge_msg_path = repo.dft_dir().join("MERGE_MSG");
    if merge_msg_path.exists() {
        let _ = fs::remove_file(merge_msg_path);
    }

    if !quiet {
        let short_hash = &commit_oid.to_hex()[..7];
        let title = msg.lines().next().unwrap_or("");
        println!("[{} {}] {}", branch_name, short_hash, title);
    }

    Ok(())
}

fn parse_author_str(s: &str) -> Option<(String, String)> {
    if let Some(start) = s.find('<') {
        if let Some(end) = s.find('>') {
            if start < end {
                let name = s[..start].trim().to_string();
                let email = s[start + 1..end].trim().to_string();
                return Some((name, email));
            }
        }
    }
    None
}
