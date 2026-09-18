//! Checkout and branch switching engine.

use crate::cas::ObjectId;
use crate::error::DaftError;
use crate::merge::checkout_tree;
use crate::object::Commit;
use crate::refs::ReferenceTarget;
use crate::repo::Repository;
use crate::worktree::reset::resolve_commit;
use std::path::Path;

pub struct CheckoutEngine<'a> {
    repo: &'a Repository,
}

impl<'a> CheckoutEngine<'a> {
    pub fn new(repo: &'a Repository) -> Self {
        Self { repo }
    }

    /// Switches to an existing branch or creates and switches with `-c`.
    pub fn switch_branch(&self, branch_name: &str, create: bool) -> Result<(), DaftError> {
        let branch_ref = format!("refs/heads/{}", branch_name);

        if create {
            let manager = crate::branch::BranchManager::new(self.repo);
            let _ = manager.create(branch_name, None, false);
        }

        let target_oid = self.repo.refs().resolve(&branch_ref)?;
        let raw = self.repo.cas().read_raw(&target_oid)?;
        let commit = Commit::deserialize(&raw.data)?;

        if let Some(workdir) = self.repo.workdir() {
            let mut index = self.repo.index()?;
            checkout_tree(self.repo.cas().as_ref(), &commit.tree, workdir, &mut index)?;
            index.write_to(&self.repo.index_path())?;
        }

        self.repo.set_head(&ReferenceTarget::Symbolic(branch_ref))?;
        Ok(())
    }

    /// Checks out a specific commit into detached HEAD state.
    pub fn checkout_commit(&self, rev: &str) -> Result<ObjectId, DaftError> {
        let commit_oid = resolve_commit(self.repo, rev)?;
        let raw = self.repo.cas().read_raw(&commit_oid)?;
        let commit = Commit::deserialize(&raw.data)?;

        if let Some(workdir) = self.repo.workdir() {
            let mut index = self.repo.index()?;
            checkout_tree(self.repo.cas().as_ref(), &commit.tree, workdir, &mut index)?;
            index.write_to(&self.repo.index_path())?;
        }

        self.repo.set_head(&ReferenceTarget::Direct(commit_oid))?;
        Ok(commit_oid)
    }

    /// Restores paths from a revision or index.
    pub fn checkout_paths(&self, paths: &[&Path], rev: Option<&str>) -> Result<(), DaftError> {
        let pathbufs: Vec<_> = paths.iter().map(|p| p.to_path_buf()).collect();
        crate::worktree::restore(self.repo, &pathbufs, false, true, rev)
    }
}
