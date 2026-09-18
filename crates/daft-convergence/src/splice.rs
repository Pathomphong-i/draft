use crate::error::ConvergenceError;
use crate::strategy::MergeStrategy;
use daft_core::cas::{ObjectId, ObjectType, RawObject};
use daft_core::graph::{walk_commits, StoreCommitGraph};
use daft_core::merge::{checkout_tree, get_signature, merge_trees_3way};
use daft_core::object::Commit;
use daft_core::refs::ReferenceTarget;
use daft_core::worktree::resolve_commit;
use daft_core::Repository;
use daft_dimension::{DimensionManager, DimensionRepository};
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpliceOutcome {
    pub donor_dim: String,
    pub target_dim: String,
    pub spliced_commits: Vec<ObjectId>,
    pub new_target_head: ObjectId,
}

pub struct SpliceEngine {
    repo: Arc<Repository>,
}

impl SpliceEngine {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self { repo }
    }

    fn resolve_dim_commit(&self, dim: &str, rev: &str) -> Result<ObjectId, ConvergenceError> {
        let dim_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), dim)?;
        if rev == "HEAD" || rev.starts_with("HEAD~") || rev.starts_with("HEAD^") {
            let head = dim_repo.head_commit().ok_or_else(|| {
                ConvergenceError::CommitNotFound(rev.to_string(), dim.to_string())
            })?;
            if rev == "HEAD" {
                return Ok(head);
            }
            let graph = StoreCommitGraph::new(self.repo.cas().as_ref());
            let commits = walk_commits(&graph, &[head])?;
            let offset: usize = rev
                .trim_start_matches("HEAD~")
                .trim_start_matches("HEAD^")
                .parse()
                .unwrap_or(1);
            if offset < commits.len() {
                return Ok(commits[offset]);
            }
            return Err(ConvergenceError::CommitNotFound(
                rev.to_string(),
                dim.to_string(),
            ));
        }

        if let Ok(oid) = ObjectId::from_hex(rev) {
            return Ok(oid);
        }

        resolve_commit(self.repo.as_ref(), rev).map_err(ConvergenceError::from)
    }

    pub fn splice(
        &self,
        donor: &str,
        target: &str,
        range_spec: &str,
        _strategy: Option<MergeStrategy>,
    ) -> Result<SpliceOutcome, ConvergenceError> {
        let dim_mgr = DimensionManager::new(Arc::clone(&self.repo));
        let dims_dir = self.repo.dft_dir().join("dimensions");

        if donor != "mainline" && !dims_dir.join(donor).exists() {
            return Err(ConvergenceError::DimensionNotFound(donor.to_string()));
        }

        if target != "mainline" && !dims_dir.join(target).exists() {
            let _ = dim_mgr.create_dimension(target, None, None)?;
        }

        let _donor_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), donor)?;
        let target_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), target)?;

        let mut commits_to_splice = Vec::new();
        let graph = StoreCommitGraph::new(self.repo.cas().as_ref());

        if let Some((left, right)) = range_spec.split_once("..") {
            let left_oid = if left.is_empty() {
                None
            } else {
                self.resolve_dim_commit(donor, left).ok()
            };
            let right_oid = self.resolve_dim_commit(donor, right)?;

            let list = walk_commits(&graph, &[right_oid])?;
            let mut rev_list = Vec::new();
            for oid in list {
                if Some(oid) == left_oid {
                    break;
                }
                rev_list.push(oid);
            }
            rev_list.reverse();
            commits_to_splice = rev_list;
        } else {
            let oid = self.resolve_dim_commit(donor, range_spec)?;
            commits_to_splice.push(oid);
        }

        let mut current_tip: Option<ObjectId> = target_repo.head_commit();
        let mut spliced_oids = Vec::new();

        for oid in commits_to_splice {
            let commit_raw = self.repo.cas().read_raw(&oid)?;
            let commit = Commit::deserialize(&commit_raw.data)?;

            let (new_tree, parents) = if let Some(tip_oid) = current_tip {
                let parent_commit = commit.parents.first().and_then(|p| {
                    self.repo
                        .cas()
                        .read_raw(p)
                        .ok()
                        .and_then(|raw| Commit::deserialize(&raw.data).ok())
                });

                let parent_tree = parent_commit.as_ref().map(|c| &c.tree);

                let tip_raw = self.repo.cas().read_raw(&tip_oid)?;
                let tip_commit = Commit::deserialize(&tip_raw.data)?;

                let merge_res = merge_trees_3way(
                    self.repo.cas().as_ref(),
                    parent_tree,
                    &tip_commit.tree,
                    &commit.tree,
                    target,
                    donor,
                )?;

                let tree = merge_res.write_clean_tree(self.repo.cas().as_ref())?;
                (tree, vec![tip_oid])
            } else {
                (commit.tree, Vec::new())
            };

            let sig = get_signature();
            let msg = format!(
                "{}\n\n[spliced from {}:{}]",
                commit.message.trim(),
                donor,
                oid.to_hex()
            );

            let new_commit = Commit::new(new_tree, parents, commit.author.clone(), sig, &msg);
            let raw = RawObject::new(ObjectType::Commit, new_commit.serialize());
            let new_oid = self.repo.cas().write_raw(&raw)?;

            current_tip = Some(new_oid);
            spliced_oids.push(new_oid);
        }

        let final_tip =
            current_tip.ok_or_else(|| ConvergenceError::General("No commits spliced".into()))?;

        // Update target HEAD
        if target == "mainline" {
            let _ = self.repo.refs().write_ref(
                "refs/heads/main",
                &ReferenceTarget::Direct(final_tip),
                None,
                None,
            );
        } else {
            target_repo.set_head(&final_tip.to_hex())?;
        }

        // Synchronize target index and workspace
        let tip_raw = self.repo.cas().read_raw(&final_tip)?;
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

        Ok(SpliceOutcome {
            donor_dim: donor.to_string(),
            target_dim: target.to_string(),
            spliced_commits: spliced_oids,
            new_target_head: final_tip,
        })
    }
}
