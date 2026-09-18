use crate::error::ConvergenceError;
use crate::strategy::MergeStrategy;
use daft_core::cas::{ObjectId, ObjectType, RawObject};
use daft_core::graph::{merge_base, walk_commits, StoreCommitGraph};
use daft_core::merge::{checkout_tree, get_signature, merge_trees_3way};
use daft_core::object::Commit;
use daft_core::refs::ReferenceTarget;
use daft_core::Repository;
use daft_dimension::{DimensionManager, DimensionRepository};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaveOutcome {
    pub target_dimension: String,
    pub woven_head: ObjectId,
    pub commit_count: usize,
    pub woven_commits: Vec<ObjectId>,
}

pub struct WeaveEngine {
    repo: Arc<Repository>,
}

impl WeaveEngine {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self { repo }
    }

    pub fn weave(
        &self,
        dim1: &str,
        dim2: &str,
        target_override: Option<&str>,
        _strategy: Option<MergeStrategy>,
    ) -> Result<WeaveOutcome, ConvergenceError> {
        let dim_mgr = DimensionManager::new(Arc::clone(&self.repo));

        let dim1_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), dim1)?;
        let dim2_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), dim2)?;

        let h1 = dim1_repo.head_commit();
        let h2 = dim2_repo.head_commit();

        let (c1, c2) = match (h1, h2) {
            (Some(a), Some(b)) => (a, b),
            (Some(a), None) => {
                return Ok(WeaveOutcome {
                    target_dimension: dim1.to_string(),
                    woven_head: a,
                    commit_count: 0,
                    woven_commits: Vec::new(),
                });
            }
            (None, Some(b)) => {
                return Ok(WeaveOutcome {
                    target_dimension: dim2.to_string(),
                    woven_head: b,
                    commit_count: 0,
                    woven_commits: Vec::new(),
                });
            }
            (None, None) => {
                return Err(ConvergenceError::General(
                    "Both dimensions have no commits".into(),
                ));
            }
        };

        let graph = StoreCommitGraph::new(self.repo.cas().as_ref());
        let lca = merge_base(&graph, &c1, &c2)?;

        // Collect commits from LCA to c1
        let list1 = walk_commits(&graph, &[c1])?;
        let mut s1 = Vec::new();
        for oid in list1 {
            if Some(oid) == lca {
                break;
            }
            s1.push((dim1.to_string(), oid));
        }

        // Collect commits from LCA to c2
        let list2 = walk_commits(&graph, &[c2])?;
        let mut s2 = Vec::new();
        for oid in list2 {
            if Some(oid) == lca {
                break;
            }
            s2.push((dim2.to_string(), oid));
        }

        // Merge and sort in causal & chronological order
        let mut all_commits: Vec<(String, ObjectId, Commit)> = Vec::new();
        let mut seen = HashSet::new();

        for (d, oid) in s1.into_iter().chain(s2.into_iter()) {
            if seen.insert(oid) {
                if let Ok(raw) = self.repo.cas().read_raw(&oid) {
                    if let Ok(commit) = Commit::deserialize(&raw.data) {
                        all_commits.push((d, oid, commit));
                    }
                }
            }
        }

        // Chronological order: committer time ascending
        all_commits.sort_by(|a, b| {
            a.2.committer
                .time
                .cmp(&b.2.committer.time)
                .then_with(|| a.1.cmp(&b.1))
        });

        let target = target_override.unwrap_or(dim1);
        if target != "mainline" && !self.repo.dft_dir().join("dimensions").join(target).exists() {
            let _ = dim_mgr.create_dimension(target, None, None)?;
        }

        let target_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), target)?;
        let mut current_tip = lca.or(h1).unwrap_or(c1);
        let mut woven_oids = Vec::new();

        for (origin_dim, orig_oid, orig_commit) in &all_commits {
            let tip_raw = self.repo.cas().read_raw(&current_tip)?;
            let tip_commit = Commit::deserialize(&tip_raw.data)?;

            let parent_commit = orig_commit.parents.first().and_then(|p| {
                self.repo
                    .cas()
                    .read_raw(p)
                    .ok()
                    .and_then(|raw| Commit::deserialize(&raw.data).ok())
            });

            let parent_tree = parent_commit.as_ref().map(|c| &c.tree);

            let merge_res = merge_trees_3way(
                self.repo.cas().as_ref(),
                parent_tree,
                &tip_commit.tree,
                &orig_commit.tree,
                target,
                origin_dim,
            )?;

            let new_tree = merge_res.write_clean_tree(self.repo.cas().as_ref())?;

            let sig = get_signature();
            let msg = format!(
                "{}\n\n[woven from {}:{}]",
                orig_commit.message.trim(),
                origin_dim,
                orig_oid.to_hex()
            );

            let new_commit = Commit::new(
                new_tree,
                vec![current_tip],
                orig_commit.author.clone(),
                sig,
                &msg,
            );
            let raw = RawObject::new(ObjectType::Commit, new_commit.serialize());
            let new_oid = self.repo.cas().write_raw(&raw)?;

            current_tip = new_oid;
            woven_oids.push(new_oid);
        }

        // Materialize target
        if target == "mainline" {
            let _ = self.repo.refs().write_ref(
                "refs/heads/main",
                &ReferenceTarget::Direct(current_tip),
                None,
                None,
            );
        } else {
            target_repo.set_head(&current_tip.to_hex())?;
        }

        // Update target index and workspace
        let tip_raw = self.repo.cas().read_raw(&current_tip)?;
        let tip_commit = Commit::deserialize(&tip_raw.data)?;

        let target_ws = target_repo.workdir();
        fs::create_dir_all(target_ws)?;
        let mut index = target_repo.index().unwrap_or_default();
        checkout_tree(
            self.repo.cas().as_ref(),
            &tip_commit.tree,
            target_ws,
            &mut index,
        )?;
        target_repo.write_index(&index)?;

        Ok(WeaveOutcome {
            target_dimension: target.to_string(),
            woven_head: current_tip,
            commit_count: woven_oids.len(),
            woven_commits: woven_oids,
        })
    }
}
