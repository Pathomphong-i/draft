use crate::error::ConvergenceError;
use crate::lock_order::MultiDimensionLockGuard;
use crate::strategy::MergeStrategy;
use daft_core::cas::{ObjectId, ObjectType, RawObject};
use daft_core::diff::binary::is_binary;
use daft_core::diff::tree::flatten_tree;
use daft_core::graph::{merge_base_octopus, StoreCommitGraph};
use daft_core::merge::{build_hierarchical_tree, checkout_tree, get_signature};
use daft_core::object::{Commit, FileMode};
use daft_core::refs::ReferenceTarget;
use daft_core::Repository;
use daft_dimension::{DimensionManager, DimensionRepository};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergeOptions {
    pub into: Option<String>,
    pub strategy: MergeStrategy,
    pub message: Option<String>,
    pub no_commit: bool,
}

impl Default for ConvergeOptions {
    fn default() -> Self {
        Self {
            into: None,
            strategy: MergeStrategy::ManualMarkers,
            message: None,
            no_commit: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConvergeOutcome {
    Clean {
        target_dimension: String,
        commit_oid: Option<ObjectId>,
        merged_dimensions: Vec<String>,
    },
    Conflict {
        target_dimension: String,
        conflicting_files: Vec<String>,
    },
}

pub struct ConvergeEngine {
    repo: Arc<Repository>,
}

impl ConvergeEngine {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self { repo }
    }

    pub fn converge(
        &self,
        dimensions: &[String],
        options: ConvergeOptions,
    ) -> Result<ConvergeOutcome, ConvergenceError> {
        if dimensions.is_empty() {
            return Err(ConvergenceError::EmptyDimensionList);
        }

        let dim_mgr = DimensionManager::new(Arc::clone(&self.repo));
        let dims_dir = self.repo.dft_dir().join("dimensions");

        // Validate source dimensions exist
        for dim in dimensions {
            if dim != "mainline" && !dims_dir.join(dim).exists() {
                return Err(ConvergenceError::DimensionNotFound(dim.clone()));
            }
        }

        let target = options
            .into
            .clone()
            .unwrap_or_else(|| dim_mgr.current_dimension_name());

        // Create target dimension if it does not exist
        if target != "mainline" && !dims_dir.join(&target).exists() {
            let _ = dim_mgr.create_dimension(&target, None, None)?;
        }

        // Deadlock-free locking
        let mut lock_dims = dimensions.to_vec();
        lock_dims.push(target.clone());
        let _guard = MultiDimensionLockGuard::acquire(&dim_mgr, lock_dims)?;

        let target_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), &target)?;
        let target_head = target_repo.head_commit();

        let mut source_heads = Vec::new();
        for dim in dimensions {
            let dim_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), dim)?;
            if let Some(h) = dim_repo.head_commit() {
                source_heads.push((dim.clone(), h));
            }
        }

        let graph = StoreCommitGraph::new(self.repo.cas().as_ref());
        let mut all_commit_oids = Vec::new();
        if let Some(th) = target_head {
            all_commit_oids.push(th);
        }
        for (_d, h) in &source_heads {
            if !all_commit_oids.contains(h) {
                all_commit_oids.push(*h);
            }
        }

        let lca_base = if all_commit_oids.is_empty() {
            None
        } else {
            merge_base_octopus(&graph, &all_commit_oids).ok().flatten()
        };

        // Flatten trees
        let mut base_files: BTreeMap<String, (FileMode, ObjectId)> = BTreeMap::new();
        if let Some(b_oid) = lca_base {
            if let Ok(raw) = self.repo.cas().read_raw(&b_oid) {
                if let Ok(commit) = Commit::deserialize(&raw.data) {
                    let _ =
                        flatten_tree(self.repo.cas().as_ref(), &commit.tree, "", &mut base_files);
                }
            }
        }

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

        let mut source_files: Vec<(String, BTreeMap<String, (FileMode, ObjectId)>)> = Vec::new();
        for (dim, h) in &source_heads {
            let mut s_files = BTreeMap::new();
            if let Ok(raw) = self.repo.cas().read_raw(h) {
                if let Ok(commit) = Commit::deserialize(&raw.data) {
                    let _ = flatten_tree(self.repo.cas().as_ref(), &commit.tree, "", &mut s_files);
                }
            }
            source_files.push((dim.clone(), s_files));
        }

        let mut universe = BTreeSet::new();
        universe.extend(base_files.keys().cloned());
        universe.extend(target_files.keys().cloned());
        for (_d, s_map) in &source_files {
            universe.extend(s_map.keys().cloned());
        }

        let mut clean_files: BTreeMap<String, (FileMode, ObjectId)> = BTreeMap::new();
        let mut conflicted_files = Vec::new();
        let mut conflict_contents: BTreeMap<String, String> = BTreeMap::new();

        for path in universe {
            let v_base = base_files.get(&path).copied();
            let v_tgt = target_files.get(&path).copied();

            let mut unique_source_versions: Vec<(String, Option<(FileMode, ObjectId)>)> =
                Vec::new();
            for (dim, s_map) in &source_files {
                unique_source_versions.push((dim.clone(), s_map.get(&path).copied()));
            }

            // Did target change relative to base?
            let tgt_changed = v_tgt != v_base;
            let mut changed_sources = Vec::new();
            for (dim, s_val) in &unique_source_versions {
                if *s_val != v_base {
                    changed_sources.push((dim.clone(), *s_val));
                }
            }

            if !tgt_changed && changed_sources.is_empty() {
                // Unchanged in all
                if let Some(v) = v_tgt.or(v_base) {
                    clean_files.insert(path, v);
                }
                continue;
            }

            if !tgt_changed && changed_sources.len() == 1 {
                // Exactly 1 source changed
                if let Some(v) = changed_sources[0].1 {
                    clean_files.insert(path, v);
                }
                continue;
            }

            if tgt_changed && changed_sources.is_empty() {
                // Only target changed
                if let Some(v) = v_tgt {
                    clean_files.insert(path, v);
                }
                continue;
            }

            // Check if all changed versions are identical
            let mut distinct_changes: Vec<Option<(FileMode, ObjectId)>> = Vec::new();
            if tgt_changed {
                distinct_changes.push(v_tgt);
            }
            for (_d, s_val) in &changed_sources {
                if !distinct_changes.contains(s_val) {
                    distinct_changes.push(*s_val);
                }
            }

            if distinct_changes.len() == 1 {
                if let Some(v) = distinct_changes[0] {
                    clean_files.insert(path, v);
                }
                continue;
            }

            // Multiple distinct changes => Conflict!
            match options.strategy {
                MergeStrategy::Ours => {
                    if let Some(v) = v_tgt {
                        clean_files.insert(path, v);
                    }
                }
                MergeStrategy::Theirs => {
                    if let Some((_d, s_val)) = changed_sources.last() {
                        if let Some(v) = s_val {
                            clean_files.insert(path, *v);
                        }
                    } else if let Some(v) = v_tgt {
                        clean_files.insert(path, v);
                    }
                }
                MergeStrategy::Union => {
                    // Combine lines sequentially
                    let mut combined_lines = Vec::new();
                    let mut is_bin = false;

                    if let Some((_m, oid)) = v_tgt {
                        if let Ok(raw) = self.repo.cas().read_raw(&oid) {
                            if is_binary(&raw.data) {
                                is_bin = true;
                            } else {
                                combined_lines.extend(
                                    String::from_utf8_lossy(&raw.data).lines().map(String::from),
                                );
                            }
                        }
                    }

                    for (_d, s_val) in &changed_sources {
                        if let Some((_m, oid)) = s_val {
                            if let Ok(raw) = self.repo.cas().read_raw(oid) {
                                if is_binary(&raw.data) {
                                    is_bin = true;
                                } else {
                                    for line in String::from_utf8_lossy(&raw.data).lines() {
                                        let s = line.to_string();
                                        if !combined_lines.contains(&s) {
                                            combined_lines.push(s);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if is_bin {
                        // Binary cannot union, fall back to target
                        if let Some(v) = v_tgt {
                            clean_files.insert(path, v);
                        }
                    } else {
                        let combined_text = combined_lines.join("\n") + "\n";
                        let raw = RawObject::new(ObjectType::Blob, combined_text.into_bytes());
                        let blob_oid = self.repo.cas().write_raw(&raw)?;
                        clean_files.insert(path, (FileMode::REGULAR, blob_oid));
                    }
                }
                MergeStrategy::ManualMarkers => {
                    let mut marker = format!("<<<<<<< ours ({})\n", target);
                    if let Some((_m, oid)) = v_tgt {
                        if let Ok(raw) = self.repo.cas().read_raw(&oid) {
                            marker.push_str(&String::from_utf8_lossy(&raw.data));
                            if !marker.ends_with('\n') {
                                marker.push('\n');
                            }
                        }
                    }
                    if let Some((_m, oid)) = v_base {
                        marker.push_str("||||||| base\n");
                        if let Ok(raw) = self.repo.cas().read_raw(&oid) {
                            marker.push_str(&String::from_utf8_lossy(&raw.data));
                            if !marker.ends_with('\n') {
                                marker.push('\n');
                            }
                        }
                    }
                    marker.push_str("=======\n");
                    for (d, s_val) in &changed_sources {
                        if let Some((_m, oid)) = s_val {
                            marker.push_str(&format!("// [{}]\n", d));
                            if let Ok(raw) = self.repo.cas().read_raw(oid) {
                                marker.push_str(&String::from_utf8_lossy(&raw.data));
                                if !marker.ends_with('\n') {
                                    marker.push('\n');
                                }
                            }
                        }
                    }
                    marker.push_str(&format!(">>>>>>> {}\n", dimensions.join(",")));

                    conflicted_files.push(path.clone());
                    conflict_contents.insert(path, marker);
                }
            }
        }

        // Materialize changes into target
        let target_ws = target_repo.workdir();
        fs::create_dir_all(target_ws)?;

        if !conflicted_files.is_empty() {
            // Write conflict markers to target workspace
            for (p, marker) in &conflict_contents {
                let full = target_ws.join(p);
                if let Some(parent) = full.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::write(full, marker);
            }
            conflicted_files.sort();
            return Ok(ConvergeOutcome::Conflict {
                target_dimension: target,
                conflicting_files: conflicted_files,
            });
        }

        // Build clean tree
        let merged_tree_oid = build_hierarchical_tree(self.repo.cas().as_ref(), &clean_files)?;

        // Write files to target workspace and update index
        let mut index = target_repo.index().unwrap_or_default();
        checkout_tree(
            self.repo.cas().as_ref(),
            &merged_tree_oid,
            target_ws,
            &mut index,
        )?;
        target_repo.write_index(&index)?;

        let mut commit_oid = None;
        if !options.no_commit {
            let sig = get_signature();
            let mut parents = Vec::new();
            if let Some(th) = target_head {
                parents.push(th);
            }
            for (_d, sh) in &source_heads {
                if !parents.contains(sh) {
                    parents.push(*sh);
                }
            }
            let msg = options.message.unwrap_or_else(|| {
                format!(
                    "Converge dimensions: {} into {}",
                    dimensions.join(", "),
                    target
                )
            });
            let commit = Commit::new(merged_tree_oid, parents, sig.clone(), sig, &msg);
            let raw = RawObject::new(ObjectType::Commit, commit.serialize());
            let c_oid = self.repo.cas().write_raw(&raw)?;
            commit_oid = Some(c_oid);

            if target == "mainline" {
                if let Ok(head_ref) = self.repo.head() {
                    match &head_ref.target {
                        ReferenceTarget::Symbolic(sym) => {
                            let _ = self.repo.refs().write_ref(
                                sym,
                                &ReferenceTarget::Direct(c_oid),
                                None,
                                None,
                            );
                        }
                        ReferenceTarget::Direct(_) => {
                            let _ = self.repo.set_head(&ReferenceTarget::Direct(c_oid));
                        }
                    }
                } else {
                    let _ = self.repo.refs().write_ref(
                        "refs/heads/main",
                        &ReferenceTarget::Direct(c_oid),
                        None,
                        None,
                    );
                }
                let dim_head = self.repo.dft_dir().join("dimensions/mainline/HEAD");
                if dim_head.exists() {
                    let _ = fs::write(dim_head, format!("{}\n", c_oid.to_hex()));
                }
                let dim_ws = self.repo.dft_dir().join("dimensions/mainline/workspace");
                if dim_ws.exists() {
                    let mut dummy_idx = daft_core::Index::new();
                    let _ = checkout_tree(
                        self.repo.cas().as_ref(),
                        &merged_tree_oid,
                        &dim_ws,
                        &mut dummy_idx,
                    );
                }
            } else {
                target_repo.set_head(&c_oid.to_hex())?;
            }
        }

        Ok(ConvergeOutcome::Clean {
            target_dimension: target,
            commit_oid,
            merged_dimensions: dimensions.to_vec(),
        })
    }
}
