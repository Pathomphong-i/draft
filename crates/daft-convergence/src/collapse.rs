use crate::error::ConvergenceError;
use crate::strategy::MergeStrategy;
use daft_core::cas::{ObjectId, ObjectType, RawObject};
use daft_core::diff::binary::is_binary;
use daft_core::diff::tree::flatten_tree;
use daft_core::merge::{build_hierarchical_tree, checkout_tree, get_signature};
use daft_core::object::{Commit, FileMode};
use daft_core::refs::ReferenceTarget;
use daft_core::Repository;
use daft_dimension::{DimensionManager, DimensionRepository};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollapseOptions {
    pub into: Option<String>,
    pub strategy: Option<MergeStrategy>,
    pub message: Option<String>,
}

impl Default for CollapseOptions {
    fn default() -> Self {
        Self {
            into: None,
            strategy: None,
            message: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CollapseOutcome {
    Clean {
        target_dimension: String,
        commit_oid: ObjectId,
        collapsed_dimensions: Vec<String>,
    },
    NoOp {
        target_dimension: String,
    },
    Conflict {
        target_dimension: String,
        conflicting_files: Vec<String>,
    },
}

pub struct CollapseEngine {
    repo: Arc<Repository>,
}

impl CollapseEngine {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self { repo }
    }

    pub fn collapse(
        &self,
        dimensions: &[String],
        options: CollapseOptions,
    ) -> Result<CollapseOutcome, ConvergenceError> {
        let _dim_mgr = DimensionManager::new(Arc::clone(&self.repo));
        let dims_dir = self.repo.dft_dir().join("dimensions");

        let target = options
            .into
            .clone()
            .unwrap_or_else(|| "mainline".to_string());

        // Resolve source dimensions
        let source_dims = if dimensions.is_empty() {
            let mut dims = Vec::new();
            if dims_dir.exists() {
                if let Ok(entries) = fs::read_dir(&dims_dir) {
                    for entry in entries.flatten() {
                        if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                            let name = entry.file_name().to_string_lossy().to_string();
                            if name != target && !dims.contains(&name) {
                                dims.push(name);
                            }
                        }
                    }
                }
            }
            dims.sort();
            dims
        } else {
            dimensions.to_vec()
        };

        if source_dims.is_empty() {
            return Ok(CollapseOutcome::NoOp {
                target_dimension: target,
            });
        }

        let target_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), &target)?;
        let target_head = target_repo.head_commit();

        // Target starting files
        let mut target_files: BTreeMap<String, (FileMode, ObjectId)> = BTreeMap::new();
        if let Some(th) = target_head {
            if let Ok(raw) = self.repo.cas().read_raw(&th) {
                if let Ok(commit) = Commit::deserialize(&raw.data) {
                    let _ = flatten_tree(
                        self.repo.cas().as_ref(),
                        &commit.tree,
                        "",
                        &mut target_files,
                    );
                }
            }
        }

        let strategy = options.strategy.unwrap_or(MergeStrategy::ManualMarkers);
        let mut accumulated_files = target_files.clone();
        let mut conflicting_files = Vec::new();
        let mut conflict_contents: BTreeMap<String, String> = BTreeMap::new();

        // Process each dimension
        for dim in &source_dims {
            let dim_repo = match DimensionRepository::for_dimension(Arc::clone(&self.repo), dim) {
                Ok(r) => r,
                Err(_) => continue,
            };

            let mut dim_files: BTreeMap<String, (FileMode, ObjectId)> = BTreeMap::new();
            if let Some(h) = dim_repo.head_commit() {
                if let Ok(raw) = self.repo.cas().read_raw(&h) {
                    if let Ok(commit) = Commit::deserialize(&raw.data) {
                        let _ = flatten_tree(
                            self.repo.cas().as_ref(),
                            &commit.tree,
                            "",
                            &mut dim_files,
                        );
                    }
                }
            }

            // Also check workspace for uncommitted files
            let workdir = dim_repo.workdir();
            if workdir.exists() {
                for entry in WalkDir::new(workdir)
                    .into_iter()
                    .filter_entry(|e| {
                        let name = e.file_name().to_string_lossy();
                        name != ".dft" && name != ".git"
                    })
                    .filter_map(|e| e.ok())
                {
                    if entry.file_type().is_file() {
                        if let Ok(rel) = entry.path().strip_prefix(workdir) {
                            let rel_str = rel.to_string_lossy().replace('\\', "/");
                            if let Ok(data) = fs::read(entry.path()) {
                                let raw = RawObject::new(ObjectType::Blob, data);
                                if let Ok(oid) = self.repo.cas().write_raw(&raw) {
                                    dim_files.insert(rel_str, (FileMode::REGULAR, oid));
                                }
                            }
                        }
                    }
                }
            }

            // Merge dim_files into accumulated_files
            for (path, (mode, oid)) in dim_files {
                match accumulated_files.get(&path) {
                    None => {
                        accumulated_files.insert(path, (mode, oid));
                    }
                    Some((_exist_mode, exist_oid)) => {
                        if *exist_oid == oid {
                            continue; // Identical
                        }

                        // Conflict between accumulated and this dimension
                        match strategy {
                            MergeStrategy::Ours => {
                                // Keep existing accumulated
                            }
                            MergeStrategy::Theirs => {
                                accumulated_files.insert(path, (mode, oid));
                            }
                            MergeStrategy::Union => {
                                let mut combined_lines = Vec::new();
                                let mut is_bin = false;
                                if let Ok(raw1) = self.repo.cas().read_raw(exist_oid) {
                                    if is_binary(&raw1.data) {
                                        is_bin = true;
                                    } else {
                                        combined_lines.extend(
                                            String::from_utf8_lossy(&raw1.data)
                                                .lines()
                                                .map(String::from),
                                        );
                                    }
                                }
                                if let Ok(raw2) = self.repo.cas().read_raw(&oid) {
                                    if is_binary(&raw2.data) {
                                        is_bin = true;
                                    } else {
                                        for line in String::from_utf8_lossy(&raw2.data).lines() {
                                            let s = line.to_string();
                                            if !combined_lines.contains(&s) {
                                                combined_lines.push(s);
                                            }
                                        }
                                    }
                                }
                                if !is_bin {
                                    let combined = combined_lines.join("\n") + "\n";
                                    let raw =
                                        RawObject::new(ObjectType::Blob, combined.into_bytes());
                                    if let Ok(new_oid) = self.repo.cas().write_raw(&raw) {
                                        accumulated_files
                                            .insert(path, (FileMode::REGULAR, new_oid));
                                    }
                                }
                            }
                            MergeStrategy::ManualMarkers => {
                                let mut marker = format!("<<<<<<< ours ({})\n", target);
                                if let Ok(raw) = self.repo.cas().read_raw(exist_oid) {
                                    marker.push_str(&String::from_utf8_lossy(&raw.data));
                                    if !marker.ends_with('\n') {
                                        marker.push('\n');
                                    }
                                }
                                marker.push_str("=======\n");
                                if let Ok(raw) = self.repo.cas().read_raw(&oid) {
                                    marker.push_str(&String::from_utf8_lossy(&raw.data));
                                    if !marker.ends_with('\n') {
                                        marker.push('\n');
                                    }
                                }
                                marker.push_str(&format!(">>>>>>> {}\n", dim));
                                conflicting_files.push(path.clone());
                                conflict_contents.insert(path, marker);
                            }
                        }
                    }
                }
            }
        }

        let target_ws = target_repo.workdir();
        fs::create_dir_all(target_ws)?;

        if !conflicting_files.is_empty() {
            for (p, marker) in &conflict_contents {
                let full = target_ws.join(p);
                if let Some(parent) = full.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::write(full, marker);
            }
            conflicting_files.sort();
            return Ok(CollapseOutcome::Conflict {
                target_dimension: target,
                conflicting_files,
            });
        }

        // Check if no changes occurred
        if accumulated_files == target_files && target_head.is_some() {
            return Ok(CollapseOutcome::NoOp {
                target_dimension: target,
            });
        }

        let tree_oid = build_hierarchical_tree(self.repo.cas().as_ref(), &accumulated_files)?;

        // Write files to target workspace and update index
        let mut index = target_repo.index().unwrap_or_default();
        checkout_tree(self.repo.cas().as_ref(), &tree_oid, target_ws, &mut index)?;
        target_repo.write_index(&index)?;

        // Atomic single-parent commit
        let sig = get_signature();
        let mut parents = Vec::new();
        if let Some(th) = target_head {
            parents.push(th);
        }
        let msg = options.message.unwrap_or_else(|| {
            format!(
                "Collapse: synthesis commit merging parallel dimensions into {}",
                target
            )
        });
        let commit = Commit::new(tree_oid, parents, sig.clone(), sig, &msg);
        let raw = RawObject::new(ObjectType::Commit, commit.serialize());
        let commit_oid = self.repo.cas().write_raw(&raw)?;

        if target == "mainline" {
            if let Ok(head_ref) = self.repo.head() {
                match &head_ref.target {
                    ReferenceTarget::Symbolic(sym) => {
                        let _ = self.repo.refs().write_ref(
                            sym,
                            &ReferenceTarget::Direct(commit_oid),
                            None,
                            None,
                        );
                    }
                    ReferenceTarget::Direct(_) => {
                        let _ = self.repo.set_head(&ReferenceTarget::Direct(commit_oid));
                    }
                }
            } else {
                let _ = self.repo.refs().write_ref(
                    "refs/heads/main",
                    &ReferenceTarget::Direct(commit_oid),
                    None,
                    None,
                );
            }
            let dim_head = self.repo.dft_dir().join("dimensions/mainline/HEAD");
            if dim_head.exists() {
                let _ = fs::write(dim_head, format!("{}\n", commit_oid.to_hex()));
            }
            let dim_ws = self.repo.dft_dir().join("dimensions/mainline/workspace");
            if dim_ws.exists() {
                let mut dummy_idx = daft_core::Index::new();
                let _ = checkout_tree(self.repo.cas().as_ref(), &tree_oid, &dim_ws, &mut dummy_idx);
            }
        } else {
            target_repo.set_head(&commit_oid.to_hex())?;
        }

        Ok(CollapseOutcome::Clean {
            target_dimension: target,
            commit_oid,
            collapsed_dimensions: source_dims,
        })
    }
}
