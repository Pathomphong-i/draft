//! Polymorphic object inspection (`dft show`).

use crate::cas::{ObjectId, ObjectType};
use crate::diff::tree::diff_trees;
use crate::diff::unified::format_unified_diff;
use crate::error::DaftError;
use crate::object::{Commit, Tag, Tree};
use crate::repo::Repository;
use crate::worktree::reset::resolve_commit;

#[derive(Debug, Clone)]
pub enum ShowResult {
    Commit {
        commit: Commit,
        oid: ObjectId,
        diff: String,
    },
    Tree {
        oid: ObjectId,
        tree: Tree,
    },
    Blob {
        oid: ObjectId,
        content: Vec<u8>,
    },
    Tag {
        oid: ObjectId,
        tag: Tag,
    },
}

pub fn show_object(repo: &Repository, rev: &str) -> Result<ShowResult, DaftError> {
    let oid = match ObjectId::from_hex(rev) {
        Ok(id) if repo.cas().has_object(&id) => id,
        _ => resolve_commit(repo, rev)?,
    };

    let raw = repo.cas().read_raw(&oid)?;

    match raw.object_type {
        ObjectType::Commit => {
            let commit = Commit::deserialize(&raw.data)?;
            let parent_tree = if let Some(parent_oid) = commit.parents.first() {
                let parent_raw = repo.cas().read_raw(parent_oid)?;
                let parent_commit = Commit::deserialize(&parent_raw.data)?;
                Some(parent_commit.tree)
            } else {
                None
            };

            let patches = diff_trees(
                repo.cas().as_ref(),
                parent_tree.as_ref(),
                Some(&commit.tree),
            )?;
            let diff_str = format_unified_diff(&patches);

            Ok(ShowResult::Commit {
                commit,
                oid,
                diff: diff_str,
            })
        }
        ObjectType::Tree => {
            let tree = Tree::deserialize(&raw.data)?;
            Ok(ShowResult::Tree { oid, tree })
        }
        ObjectType::Blob => Ok(ShowResult::Blob {
            oid,
            content: raw.data,
        }),
        ObjectType::Tag => {
            let tag = Tag::deserialize(&raw.data)?;
            Ok(ShowResult::Tag { oid, tag })
        }
    }
}
