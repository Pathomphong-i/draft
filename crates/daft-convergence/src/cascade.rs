use crate::error::ConvergenceError;
use daft_core::cas::{ObjectId, ObjectType, RawObject};
use daft_core::graph::{is_ancestor, merge_base, StoreCommitGraph};
use daft_core::index::Index;
use daft_core::merge::{checkout_tree, get_signature, merge_trees_3way};
use daft_core::object::Commit;
use daft_core::Repository;
use daft_dimension::{DimensionMetadata, DimensionRepository};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeStep {
    pub from_dimension: String,
    pub to_dimension: String,
    pub commit_oid: Option<ObjectId>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CascadeOutcome {
    Success {
        propagated_chain: Vec<String>,
        steps: Vec<CascadeStep>,
    },
    AbortedOnConflict {
        at_dimension: String,
        conflicting_files: Vec<String>,
        completed_chain: Vec<String>,
    },
}

pub struct CascadeEngine {
    repo: Arc<Repository>,
}

impl CascadeEngine {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self { repo }
    }

    /// Resolves the sequence of dimensions for the cascade.
    pub fn resolve_chain(
        &self,
        source: &str,
        target: Option<&str>,
    ) -> Result<Vec<String>, ConvergenceError> {
        if let Some(t) = target {
            return Ok(vec![source.to_string(), t.to_string()]);
        }

        // Implicit fork hierarchy
        let dims_dir = self.repo.dft_dir().join("dimensions");
        let mut children_map: HashMap<String, Vec<String>> = HashMap::new();

        if dims_dir.exists() {
            if let Ok(entries) = fs::read_dir(&dims_dir) {
                for e in entries.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    let meta_path = e.path().join("meta.json");
                    if meta_path.exists() {
                        if let Ok(content) = fs::read_to_string(&meta_path) {
                            if let Ok(meta) = serde_json::from_str::<DimensionMetadata>(&content) {
                                if let Some(parent) = meta.parent {
                                    children_map.entry(parent).or_default().push(name);
                                }
                            }
                        }
                    }
                }
            }
        }

        // BFS from source
        let mut chain = vec![source.to_string()];
        let mut queue = VecDeque::new();
        queue.push_back(source.to_string());

        while let Some(curr) = queue.pop_front() {
            if let Some(mut children) = children_map.remove(&curr) {
                children.sort();
                for child in children {
                    if !chain.contains(&child) {
                        chain.push(child.clone());
                        queue.push_back(child);
                    }
                }
            }
        }

        Ok(chain)
    }

    pub fn cascade(
        &self,
        source: &str,
        target: Option<&str>,
        abort_on_conflict: bool,
    ) -> Result<CascadeOutcome, ConvergenceError> {
        let chain = self.resolve_chain(source, target)?;
        if chain.len() < 2 {
            return Ok(CascadeOutcome::Success {
                propagated_chain: chain,
                steps: Vec::new(),
            });
        }

        let mut completed_chain = vec![chain[0].clone()];
        let mut steps = Vec::new();
        let graph = StoreCommitGraph::new(self.repo.cas().as_ref());

        for i in 0..(chain.len() - 1) {
            let from_dim = &chain[i];
            let to_dim = &chain[i + 1];

            let from_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), from_dim)?;
            let to_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), to_dim)?;

            let from_head = match from_repo.head_commit() {
                Some(h) => h,
                None => {
                    steps.push(CascadeStep {
                        from_dimension: from_dim.clone(),
                        to_dimension: to_dim.clone(),
                        commit_oid: None,
                        status: "UpToDate".to_string(),
                    });
                    completed_chain.push(to_dim.clone());
                    continue;
                }
            };

            let to_head = to_repo.head_commit();
            let to_ws = to_repo.workdir();
            fs::create_dir_all(to_ws)?;

            if to_head.is_none() {
                to_repo.set_head(&from_head.to_hex())?;
                let from_raw = self.repo.cas().read_raw(&from_head)?;
                let from_commit = Commit::deserialize(&from_raw.data)?;
                let mut index = Index::new();
                checkout_tree(
                    self.repo.cas().as_ref(),
                    &from_commit.tree,
                    to_ws,
                    &mut index,
                )?;
                to_repo.write_index(&index)?;

                steps.push(CascadeStep {
                    from_dimension: from_dim.clone(),
                    to_dimension: to_dim.clone(),
                    commit_oid: Some(from_head),
                    status: "FastForward".to_string(),
                });
                completed_chain.push(to_dim.clone());
                continue;
            }

            let th = to_head.unwrap();
            if th == from_head || is_ancestor(&graph, &from_head, &th)? {
                steps.push(CascadeStep {
                    from_dimension: from_dim.clone(),
                    to_dimension: to_dim.clone(),
                    commit_oid: None,
                    status: "UpToDate".to_string(),
                });
                completed_chain.push(to_dim.clone());
                continue;
            }

            // Find merge base
            let lca = merge_base(&graph, &th, &from_head)?;
            let base_tree = lca.and_then(|b| {
                self.repo
                    .cas()
                    .read_raw(&b)
                    .ok()
                    .and_then(|raw| Commit::deserialize(&raw.data).ok().map(|c| c.tree))
            });

            let to_raw = self.repo.cas().read_raw(&th)?;
            let to_commit = Commit::deserialize(&to_raw.data)?;
            let from_raw = self.repo.cas().read_raw(&from_head)?;
            let from_commit = Commit::deserialize(&from_raw.data)?;

            let merge_res = merge_trees_3way(
                self.repo.cas().as_ref(),
                base_tree.as_ref(),
                &to_commit.tree,
                &from_commit.tree,
                to_dim,
                from_dim,
            )?;

            if merge_res.has_conflicts() {
                let conflicts: Vec<String> = merge_res.conflicted_files.keys().cloned().collect();
                if abort_on_conflict {
                    return Ok(CascadeOutcome::AbortedOnConflict {
                        at_dimension: to_dim.clone(),
                        conflicting_files: conflicts,
                        completed_chain,
                    });
                }
            }

            let new_tree_oid = merge_res.write_clean_tree(self.repo.cas().as_ref())?;

            // Create cascade commit
            let sig = get_signature();
            let mut parents = vec![th];
            if !parents.contains(&from_head) {
                parents.push(from_head);
            }

            let msg = format!(
                "Cascade: propagated changes from '{}' into '{}'",
                from_dim, to_dim
            );
            let commit = Commit::new(new_tree_oid, parents, sig.clone(), sig, &msg);
            let raw = RawObject::new(ObjectType::Commit, commit.serialize());
            let c_oid = self.repo.cas().write_raw(&raw)?;

            to_repo.set_head(&c_oid.to_hex())?;

            let mut index = to_repo.index().unwrap_or_default();
            checkout_tree(self.repo.cas().as_ref(), &new_tree_oid, to_ws, &mut index)?;
            to_repo.write_index(&index)?;

            steps.push(CascadeStep {
                from_dimension: from_dim.clone(),
                to_dimension: to_dim.clone(),
                commit_oid: Some(c_oid),
                status: "Success".to_string(),
            });
            completed_chain.push(to_dim.clone());
        }

        Ok(CascadeOutcome::Success {
            propagated_chain: completed_chain,
            steps,
        })
    }
}
