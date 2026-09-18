//! Tag subsystem for lightweight and annotated tags.

use crate::cas::{ObjectId, ObjectType, RawObject};
use crate::error::DaftError;
use crate::merge::get_signature;
use crate::object::Tag;
use crate::refs::{validate_ref_name, ReferenceTarget};
use crate::repo::Repository;
use crate::worktree::reset::resolve_commit;

pub struct TagManager<'a> {
    repo: &'a Repository,
}

impl<'a> TagManager<'a> {
    pub fn new(repo: &'a Repository) -> Self {
        Self { repo }
    }

    /// Creates a lightweight tag pointing directly to a target commit.
    pub fn create_lightweight(
        &self,
        name: &str,
        target_rev: Option<&str>,
    ) -> Result<ObjectId, DaftError> {
        validate_ref_name(name)?;
        let target_oid = if let Some(rev) = target_rev {
            resolve_commit(self.repo, rev)?
        } else {
            crate::refs::peel_reference(self.repo.dft_dir(), "HEAD").map_err(DaftError::Ref)?
        };

        let tag_ref = format!("refs/tags/{}", name);
        self.repo
            .refs()
            .write_ref(&tag_ref, &ReferenceTarget::Direct(target_oid), None, None)?;

        Ok(target_oid)
    }

    /// Creates an annotated tag object in CAS and creates a tag ref pointing to it.
    pub fn create_annotated(
        &self,
        name: &str,
        message: &str,
        target_rev: Option<&str>,
    ) -> Result<ObjectId, DaftError> {
        validate_ref_name(name)?;
        let target_oid = if let Some(rev) = target_rev {
            resolve_commit(self.repo, rev)?
        } else {
            crate::refs::peel_reference(self.repo.dft_dir(), "HEAD").map_err(DaftError::Ref)?
        };

        let sig = get_signature();
        let tag_obj = Tag::new(target_oid, ObjectType::Commit, name, Some(sig), message);
        let raw = RawObject::new(ObjectType::Tag, tag_obj.serialize());
        let tag_oid = self.repo.cas().write_raw(&raw)?;

        let tag_ref = format!("refs/tags/{}", name);
        self.repo
            .refs()
            .write_ref(&tag_ref, &ReferenceTarget::Direct(tag_oid), None, None)?;

        Ok(tag_oid)
    }

    /// Lists all tags in `refs/tags/`.
    pub fn list(&self) -> Result<Vec<String>, DaftError> {
        let refs = self.repo.refs().list_refs("refs/tags")?;
        let mut names = Vec::new();
        for r in refs {
            let name = r
                .name
                .strip_prefix("refs/tags/")
                .unwrap_or(&r.name)
                .to_string();
            names.push(name);
        }
        names.sort();
        Ok(names)
    }

    /// Deletes a tag ref.
    pub fn delete(&self, name: &str) -> Result<(), DaftError> {
        let tag_ref = format!("refs/tags/{}", name);
        self.repo.refs().delete_ref(&tag_ref, None)?;
        Ok(())
    }

    /// Recursively peels a tag OID to its underlying commit OID.
    pub fn peel_to_commit(&self, mut oid: ObjectId) -> Result<ObjectId, DaftError> {
        use std::collections::HashSet;
        let mut visited = HashSet::new();

        loop {
            if !visited.insert(oid) {
                return Err(DaftError::Config("Tag cycle detected".into()));
            }
            let raw = self.repo.cas().read_raw(&oid)?;
            match raw.object_type {
                ObjectType::Commit => return Ok(oid),
                ObjectType::Tag => {
                    let tag = Tag::deserialize(&raw.data)?;
                    oid = tag.target;
                }
                other => {
                    return Err(DaftError::Config(format!(
                        "Cannot peel {} to commit",
                        other
                    )));
                }
            }
        }
    }
}
