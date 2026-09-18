//! Branch management engine.

use crate::cas::ObjectId;
use crate::error::DaftError;
use crate::graph::is_ancestor;
use crate::object::Commit;
use crate::refs::{validate_ref_name, ReferenceTarget};
use crate::repo::Repository;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchInfo {
    pub name: String,
    pub target: ObjectId,
    pub is_head: bool,
    pub summary: String,
}

pub struct BranchManager<'a> {
    repo: &'a Repository,
}

impl<'a> BranchManager<'a> {
    pub fn new(repo: &'a Repository) -> Self {
        Self { repo }
    }

    /// Lists all branches in `refs/heads/`, marking the currently checked-out branch.
    pub fn list(&self) -> Result<Vec<BranchInfo>, DaftError> {
        let refs = self.repo.refs().list_refs("refs/heads")?;
        let head_ref = self.repo.head().ok();
        let current_branch = head_ref.as_ref().and_then(|h| {
            if let ReferenceTarget::Symbolic(sym) = &h.target {
                sym.strip_prefix("refs/heads/").map(String::from)
            } else {
                None
            }
        });

        let mut branches = Vec::new();

        for r in refs {
            let branch_name = r
                .name
                .strip_prefix("refs/heads/")
                .unwrap_or(&r.name)
                .to_string();
            let is_head = current_branch.as_deref() == Some(&branch_name);

            let target_oid = match r.target {
                ReferenceTarget::Direct(oid) => oid,
                ReferenceTarget::Symbolic(sym) => self.repo.refs().resolve(&sym)?,
            };

            let summary = if let Ok(raw) = self.repo.cas().read_raw(&target_oid) {
                if let Ok(commit) = Commit::deserialize(&raw.data) {
                    commit.message.lines().next().unwrap_or("").to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            branches.push(BranchInfo {
                name: branch_name,
                target: target_oid,
                is_head,
                summary,
            });
        }

        Ok(branches)
    }

    /// Creates a new branch pointing to `start_point` (defaults to HEAD).
    pub fn create(
        &self,
        name: &str,
        start_point: Option<&str>,
        force: bool,
    ) -> Result<ObjectId, DaftError> {
        validate_ref_name(name)?;
        let ref_path = format!("refs/heads/{}", name);

        if !force && self.repo.refs().read_ref(&ref_path).is_ok() {
            return Err(DaftError::Config(format!(
                "A branch named '{}' already exists.",
                name
            )));
        }

        let target_oid = if let Some(sp) = start_point {
            crate::worktree::resolve_commit(self.repo, sp)?
        } else {
            crate::refs::peel_reference(self.repo.dft_dir(), "HEAD").map_err(DaftError::Ref)?
        };

        self.repo
            .refs()
            .write_ref(&ref_path, &ReferenceTarget::Direct(target_oid), None, None)?;

        Ok(target_oid)
    }

    /// Deletes a branch with safety checks unless force (`-D`) is requested.
    pub fn delete(&self, name: &str, force: bool) -> Result<ObjectId, DaftError> {
        let ref_path = format!("refs/heads/{}", name);
        let head_ref = self.repo.head().ok();

        if let Some(h) = head_ref {
            if let ReferenceTarget::Symbolic(sym) = &h.target {
                if sym == &ref_path || sym.strip_prefix("refs/heads/") == Some(name) {
                    return Err(DaftError::Config(format!(
                        "Cannot delete branch '{}' checked out",
                        name
                    )));
                }
            }
        }

        let branch_ref = self.repo.refs().read_ref(&ref_path)?;
        let branch_oid = match branch_ref.target {
            ReferenceTarget::Direct(oid) => oid,
            ReferenceTarget::Symbolic(sym) => self.repo.refs().resolve(&sym)?,
        };

        if !force {
            if let Ok(head_oid) = crate::refs::peel_reference(self.repo.dft_dir(), "HEAD") {
                let graph = crate::graph::StoreCommitGraph::new(self.repo.cas().as_ref());
                if !is_ancestor(&graph, &branch_oid, &head_oid)? {
                    return Err(DaftError::Config(format!(
                        "The branch '{}' is not fully merged. If you are sure you want to delete it, run 'dft branch -D {}'.",
                        name, name
                    )));
                }
            }
        }

        self.repo.refs().delete_ref(&ref_path, None)?;
        Ok(branch_oid)
    }

    /// Renames a branch, updating HEAD if the current branch is renamed.
    pub fn rename(&self, old_name: &str, new_name: &str, force: bool) -> Result<(), DaftError> {
        validate_ref_name(new_name)?;
        let old_ref_path = format!("refs/heads/{}", old_name);
        let new_ref_path = format!("refs/heads/{}", new_name);

        if !force && self.repo.refs().read_ref(&new_ref_path).is_ok() {
            return Err(DaftError::Config(format!(
                "A branch named '{}' already exists.",
                new_name
            )));
        }

        let branch_ref = self.repo.refs().read_ref(&old_ref_path)?;
        let target_oid = match branch_ref.target {
            ReferenceTarget::Direct(oid) => oid,
            ReferenceTarget::Symbolic(sym) => self.repo.refs().resolve(&sym)?,
        };

        // Write new ref
        self.repo.refs().write_ref(
            &new_ref_path,
            &ReferenceTarget::Direct(target_oid),
            None,
            None,
        )?;

        // Delete old ref
        self.repo.refs().delete_ref(&old_ref_path, None)?;

        // Update HEAD if old branch was active
        if let Ok(head_ref) = self.repo.head() {
            if let ReferenceTarget::Symbolic(sym) = &head_ref.target {
                if sym == &old_ref_path {
                    self.repo
                        .set_head(&ReferenceTarget::Symbolic(new_ref_path.clone()))?;
                }
            }
        }

        // Rename reflog if exists
        let old_log = self.repo.dft_dir().join("logs").join(&old_ref_path);
        let new_log = self.repo.dft_dir().join("logs").join(&new_ref_path);
        if old_log.exists() {
            if let Some(parent) = new_log.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let _ = std::fs::rename(old_log, new_log);
        }

        Ok(())
    }
}
