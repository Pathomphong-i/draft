//! Cherry-pick commit patch onto HEAD (`dft cherry-pick`).

use crate::cas::{ObjectId, ObjectType, RawObject};
use crate::error::DaftError;
use crate::merge::tree_merge::merge_trees_3way;
use crate::merge::{checkout_tree, get_signature};
use crate::object::Commit;
use crate::refs::ReferenceTarget;
use crate::repo::Repository;
use crate::worktree::reset::resolve_commit;

pub fn cherry_pick(repo: &Repository, rev: &str) -> Result<ObjectId, DaftError> {
    let commit_oid = resolve_commit(repo, rev)?;
    let raw = repo.cas().read_raw(&commit_oid)?;
    let commit = Commit::deserialize(&raw.data)?;

    let head_oid = crate::refs::peel_reference(repo.dft_dir(), "HEAD").map_err(DaftError::Ref)?;
    let head_raw = repo.cas().read_raw(&head_oid)?;
    let head_commit = Commit::deserialize(&head_raw.data)?;

    let parent_tree_oid = if let Some(p) = commit.parents.first() {
        let p_raw = repo.cas().read_raw(p)?;
        let p_commit = Commit::deserialize(&p_raw.data)?;
        Some(p_commit.tree)
    } else {
        None
    };

    let merge_result = merge_trees_3way(
        repo.cas().as_ref(),
        parent_tree_oid.as_ref(),
        &head_commit.tree,
        &commit.tree,
        "HEAD",
        &commit_oid.to_hex()[..8],
    )?;

    if merge_result.has_conflicts() {
        return Err(DaftError::Config(format!(
            "Could not apply {}: merge conflict detected",
            &commit_oid.to_hex()[..8]
        )));
    }

    let merged_tree = merge_result.write_clean_tree(repo.cas().as_ref())?;
    let committer_sig = get_signature();

    let new_commit = Commit::new(
        merged_tree,
        vec![head_oid],
        commit.author,
        committer_sig,
        &commit.message,
    );
    let serialized = new_commit.serialize();
    let raw_obj = RawObject::new(ObjectType::Commit, serialized);
    let new_oid = repo.cas().write_raw(&raw_obj)?;

    // Update branch ref or HEAD
    let head_ref = repo.head()?;
    if let ReferenceTarget::Symbolic(sym) = &head_ref.target {
        repo.refs()
            .write_ref(sym, &ReferenceTarget::Direct(new_oid), None, None)?;
    } else {
        repo.set_head(&ReferenceTarget::Direct(new_oid))?;
    }

    if let Some(workdir) = repo.workdir() {
        let mut index = repo.index()?;
        checkout_tree(repo.cas().as_ref(), &merged_tree, workdir, &mut index)?;
        index.write_to(&repo.index_path())?;
    }

    Ok(new_oid)
}
