//! Binary search regression engine (`dft bisect`).

use crate::cas::ObjectId;
use crate::error::DaftError;
use crate::graph::{walk_commits, StoreCommitGraph};
use crate::repo::Repository;
use crate::worktree::reset::resolve_commit;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

fn bisect_start_path(repo: &Repository) -> PathBuf {
    repo.dft_dir().join("BISECT_START")
}

fn bisect_bad_path(repo: &Repository) -> PathBuf {
    repo.dft_dir().join("BISECT_BAD")
}

fn bisect_good_path(repo: &Repository) -> PathBuf {
    repo.dft_dir().join("BISECT_GOOD")
}

#[derive(Debug, Clone)]
pub enum BisectStep {
    Step {
        midpoint: ObjectId,
        revisions_left: usize,
        estimated_steps: usize,
    },
    FoundFirstBad(ObjectId),
    WaitingForInput(String),
}

pub fn bisect_start(repo: &Repository) -> Result<(), DaftError> {
    let head_ref = repo.head()?;
    let ref_str = match &head_ref.target {
        crate::refs::ReferenceTarget::Symbolic(s) => s.clone(),
        crate::refs::ReferenceTarget::Direct(oid) => oid.to_hex(),
    };
    fs::write(bisect_start_path(repo), ref_str)?;
    Ok(())
}

pub fn bisect_bad(repo: &Repository, rev: Option<&str>) -> Result<BisectStep, DaftError> {
    let oid = if let Some(r) = rev {
        resolve_commit(repo, r)?
    } else {
        crate::refs::peel_reference(repo.dft_dir(), "HEAD").map_err(DaftError::Ref)?
    };
    fs::write(bisect_bad_path(repo), format!("{}\n", oid))?;
    compute_next_step(repo)
}

pub fn bisect_good(repo: &Repository, rev: Option<&str>) -> Result<BisectStep, DaftError> {
    let oid = if let Some(r) = rev {
        resolve_commit(repo, r)?
    } else {
        crate::refs::peel_reference(repo.dft_dir(), "HEAD").map_err(DaftError::Ref)?
    };
    let good_path = bisect_good_path(repo);
    let mut current = if good_path.exists() {
        fs::read_to_string(&good_path)?
    } else {
        String::new()
    };
    current.push_str(&format!("{}\n", oid));
    fs::write(&good_path, current)?;
    compute_next_step(repo)
}

fn compute_next_step(repo: &Repository) -> Result<BisectStep, DaftError> {
    let bad_path = bisect_bad_path(repo);
    let good_path = bisect_good_path(repo);

    if !bad_path.exists() {
        return Ok(BisectStep::WaitingForInput("Waiting for bad commit".into()));
    }
    let bad_str = fs::read_to_string(&bad_path)?;
    let bad_oid = ObjectId::from_hex(bad_str.trim())
        .map_err(|_| DaftError::Config("Corrupt BISECT_BAD".into()))?;

    if !good_path.exists() {
        return Ok(BisectStep::WaitingForInput(
            "Waiting for good commit(s)".into(),
        ));
    }
    let good_str = fs::read_to_string(&good_path)?;
    let mut good_oids = Vec::new();
    for line in good_str.lines() {
        if let Ok(oid) = ObjectId::from_hex(line.trim()) {
            good_oids.push(oid);
        }
    }
    if good_oids.is_empty() {
        return Ok(BisectStep::WaitingForInput(
            "Waiting for good commit(s)".into(),
        ));
    }

    let cas = repo.cas();
    let graph = StoreCommitGraph::new(cas.as_ref());

    // Reachable from bad
    let reachable_from_bad = walk_commits(&graph, &[bad_oid])?;
    let mut reachable_from_good = HashSet::new();
    for g in &good_oids {
        let walk = walk_commits(&graph, &[*g])?;
        for c in walk {
            reachable_from_good.insert(c);
        }
    }

    // Candidates: Reachable(bad) \ Reachable(good)
    let candidates: Vec<ObjectId> = reachable_from_bad
        .into_iter()
        .filter(|c| !reachable_from_good.contains(c))
        .collect();

    if candidates.is_empty() {
        return Ok(BisectStep::FoundFirstBad(bad_oid));
    }

    if candidates.len() == 1 {
        return Ok(BisectStep::FoundFirstBad(candidates[0]));
    }

    // Midpoint: select candidate closest to half
    let mid_idx = candidates.len() / 2;
    let midpoint = candidates[mid_idx];

    // Checkout midpoint in detached HEAD
    let engine = crate::checkout::CheckoutEngine::new(repo);
    engine.checkout_commit(&midpoint.to_hex())?;

    let revisions_left = candidates.len();
    let estimated_steps = (revisions_left as f64).log2().ceil() as usize;

    Ok(BisectStep::Step {
        midpoint,
        revisions_left,
        estimated_steps,
    })
}

pub fn bisect_reset(repo: &Repository) -> Result<(), DaftError> {
    let start_path = bisect_start_path(repo);
    if start_path.exists() {
        let start_ref = fs::read_to_string(&start_path)?;
        let trimmed = start_ref.trim();
        if trimmed.starts_with("refs/heads/") {
            let branch = trimmed.strip_prefix("refs/heads/").unwrap();
            let engine = crate::checkout::CheckoutEngine::new(repo);
            engine.switch_branch(branch, false)?;
        } else {
            let engine = crate::checkout::CheckoutEngine::new(repo);
            engine.checkout_commit(trimmed)?;
        }
        let _ = fs::remove_file(start_path);
    }

    let bad_path = bisect_bad_path(repo);
    if bad_path.exists() {
        let _ = fs::remove_file(bad_path);
    }
    let good_path = bisect_good_path(repo);
    if good_path.exists() {
        let _ = fs::remove_file(good_path);
    }

    Ok(())
}
