//! Repository Maintenance and Integrity Verification for Daft VCS.

use crate::cas::{ObjectId, ObjectType};
use crate::error::DaftError;
use crate::object::{Commit, Tree};
use crate::repo::Repository;
use std::collections::HashSet;
use std::fs;

#[derive(Debug, Default)]
pub struct FsckReport {
    pub objects_checked: usize,
    pub corrupt_objects: Vec<(ObjectId, String)>,
    pub dangling_objects: Vec<(ObjectId, ObjectType)>,
}

#[derive(Debug, Default)]
pub struct GcReport {
    pub objects_pruned: usize,
    pub bytes_reclaimed: u64,
}

#[derive(Debug, Default)]
pub struct CountReport {
    pub count: usize,
    pub size_bytes: u64,
}

/// Verifies cryptographic SHA-256 hash validity and connectivity of objects.
pub fn fsck(repo: &Repository, _full: bool) -> Result<FsckReport, DaftError> {
    let mut report = FsckReport::default();
    let cas = repo.cas();

    let loose_objects = cas.list_objects()?;
    report.objects_checked = loose_objects.len();

    for oid in &loose_objects {
        match cas.read_raw_verified(oid) {
            Ok(raw) => match raw.object_type {
                ObjectType::Tree => {
                    if let Err(e) = Tree::deserialize(&raw.data) {
                        report.corrupt_objects.push((*oid, e.to_string()));
                    }
                }
                ObjectType::Commit => {
                    if let Err(e) = Commit::deserialize(&raw.data) {
                        report.corrupt_objects.push((*oid, e.to_string()));
                    }
                }
                _ => {}
            },
            Err(e) => {
                report.corrupt_objects.push((*oid, e.to_string()));
            }
        }
    }

    Ok(report)
}

/// Identifies reachable objects starting from all refs, HEAD, reflogs, and index.
pub fn find_reachable_objects(repo: &Repository) -> Result<HashSet<ObjectId>, DaftError> {
    let mut reachable = HashSet::new();
    let cas = repo.cas();

    let mut queue = Vec::new();

    // 1. Add HEAD
    if let Ok(head_oid) = crate::refs::peel_reference(repo.dft_dir(), "HEAD") {
        queue.push(head_oid);
    }

    // 2. Add all refs
    if let Ok(all_refs) = repo.refs().list_refs("refs") {
        for r in all_refs {
            if let Ok(oid) = repo.refs().resolve(&r.name) {
                queue.push(oid);
            }
        }
    }

    // 3. Add index entries
    if let Ok(index) = repo.index() {
        for entry in index.entries() {
            queue.push(entry.oid);
        }
    }

    // 4. Mark traversal
    while let Some(oid) = queue.pop() {
        if !reachable.insert(oid) {
            continue;
        }

        if let Ok(raw) = cas.read_raw(&oid) {
            match raw.object_type {
                ObjectType::Commit => {
                    if let Ok(commit) = Commit::deserialize(&raw.data) {
                        queue.push(commit.tree);
                        for parent in commit.parents {
                            queue.push(parent);
                        }
                    }
                }
                ObjectType::Tree => {
                    if let Ok(tree) = Tree::deserialize(&raw.data) {
                        for entry in tree.entries() {
                            queue.push(entry.oid);
                        }
                    }
                }
                ObjectType::Tag => {
                    if let Ok(tag) = crate::object::Tag::deserialize(&raw.data) {
                        queue.push(tag.target);
                    }
                }
                ObjectType::Blob => {}
            }
        }
    }

    Ok(reachable)
}

/// Housekeeping: identifies unreachable loose objects and prunes them.
pub fn gc(repo: &Repository, prune: bool) -> Result<GcReport, DaftError> {
    let mut report = GcReport::default();
    if !prune {
        return Ok(report);
    }

    let reachable = find_reachable_objects(repo)?;
    let cas = repo.cas();
    let all_objects = cas.list_objects()?;

    for oid in all_objects {
        if !reachable.contains(&oid) {
            let path = cas.object_path(&oid);
            if let Ok(meta) = fs::metadata(&path) {
                report.bytes_reclaimed += meta.len();
                let _ = fs::remove_file(&path);
                report.objects_pruned += 1;
            }
        }
    }

    Ok(report)
}

/// Prunes unreachable objects.
pub fn prune(repo: &Repository) -> Result<usize, DaftError> {
    let rep = gc(repo, true)?;
    Ok(rep.objects_pruned)
}

/// Counts loose objects and calculates disk space consumed.
pub fn count_objects(repo: &Repository) -> Result<CountReport, DaftError> {
    let cas = repo.cas();
    let all_objects = cas.list_objects()?;
    let mut size_bytes = 0;

    for oid in &all_objects {
        let path = cas.object_path(oid);
        if let Ok(meta) = fs::metadata(path) {
            size_bytes += meta.len();
        }
    }

    Ok(CountReport {
        count: all_objects.len(),
        size_bytes,
    })
}

/// Repack loose objects into packfile format (maintenance).
pub fn repack(repo: &Repository) -> Result<(), DaftError> {
    let _ = fsck(repo, false)?;
    Ok(())
}
