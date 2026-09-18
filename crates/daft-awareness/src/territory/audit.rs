use crate::error::AwarenessError;
use crate::territory::claim::Claim;
use crate::territory::glob::{matches_glob, patterns_overlap};
use crate::territory::store::TerritoryStore;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimCollision {
    pub path_glob_a: String,
    pub claim_id_a: String,
    pub agent_a: Option<String>,
    pub dimension_a: String,
    pub path_glob_b: String,
    pub claim_id_b: String,
    pub agent_b: Option<String>,
    pub dimension_b: String,
    pub is_exclusive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FenceConflict {
    pub fence_path: String,
    pub fence_owner: Option<String>,
    pub claim_path: String,
    pub claim_owner_dim: String,
    pub claim_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViolationType {
    FenceBreach,
    ClaimBreach,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerritoryViolation {
    pub file_path: String,
    pub dimension: String,
    pub violation_type: ViolationType,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub total_claims: usize,
    pub active_claims: usize,
    pub expired_claims: Vec<Claim>,
    pub total_fences: usize,
    pub collisions: Vec<ClaimCollision>,
    pub fence_conflicts: Vec<FenceConflict>,
    pub violations: Vec<TerritoryViolation>,
    pub fixed_pruned_claims: Vec<String>,
}

impl AuditReport {
    pub fn is_clean(&self) -> bool {
        self.collisions.is_empty()
            && self.fence_conflicts.is_empty()
            && self.violations.is_empty()
            && self.expired_claims.is_empty()
    }
}

pub struct AuditEngine<'a> {
    store: &'a TerritoryStore,
    dft_dir: &'a Path,
}

impl<'a> AuditEngine<'a> {
    pub fn new(store: &'a TerritoryStore, dft_dir: &'a Path) -> Self {
        Self { store, dft_dir }
    }

    pub fn audit(&self, fix: bool) -> Result<AuditReport, AwarenessError> {
        let _guard = if fix {
            Some(self.store.lock(None, "audit:fix")?)
        } else {
            None
        };

        let now = Utc::now();
        let claims = self.store.load_all_claims()?;
        let fences = self.store.load_all_fences()?;

        let mut active_claims = Vec::new();
        let mut expired_claims = Vec::new();

        for c in claims {
            if c.is_active(now) {
                active_claims.push(c);
            } else {
                expired_claims.push(c);
            }
        }

        // Auto-fix pruning
        let mut fixed_pruned_claims = Vec::new();
        if fix {
            for exp in &expired_claims {
                if self.store.delete_claim(&exp.id)? {
                    fixed_pruned_claims.push(exp.id.clone());
                }
            }
        }

        // Detect claim collisions
        let mut collisions = Vec::new();
        for i in 0..active_claims.len() {
            for j in (i + 1)..active_claims.len() {
                let ca = &active_claims[i];
                let cb = &active_claims[j];

                if ca.dimension == cb.dimension && ca.agent_id == cb.agent_id {
                    continue;
                }

                if patterns_overlap(&ca.path_glob, &cb.path_glob) {
                    if ca.exclusive || cb.exclusive {
                        collisions.push(ClaimCollision {
                            path_glob_a: ca.path_glob.clone(),
                            claim_id_a: ca.id.clone(),
                            agent_a: ca.agent_id.clone(),
                            dimension_a: ca.dimension.clone(),
                            path_glob_b: cb.path_glob.clone(),
                            claim_id_b: cb.id.clone(),
                            agent_b: cb.agent_id.clone(),
                            dimension_b: cb.dimension.clone(),
                            is_exclusive: true,
                        });
                    }
                }
            }
        }

        // Detect fence vs claim conflicts
        let mut fence_conflicts = Vec::new();
        for fence in &fences {
            if !fence.is_active(now) {
                continue;
            }
            for claim in &active_claims {
                if patterns_overlap(&fence.path_glob, &claim.path_glob) {
                    if let Some(ref owner) = fence.owner_dimension {
                        if owner != &claim.dimension {
                            fence_conflicts.push(FenceConflict {
                                fence_path: fence.path_glob.clone(),
                                fence_owner: fence.owner_dimension.clone(),
                                claim_path: claim.path_glob.clone(),
                                claim_owner_dim: claim.dimension.clone(),
                                claim_agent: claim.agent_id.clone(),
                            });
                        }
                    }
                }
            }
        }

        // Detect cross-dimension workspace violations
        let mut violations = Vec::new();
        let dims_dir = self.dft_dir.join("dimensions");
        let mut checked_dimensions = vec![(
            "mainline".to_string(),
            self.dft_dir.parent().unwrap_or(self.dft_dir).to_path_buf(),
        )];

        if dims_dir.exists() {
            if let Ok(entries) = fs::read_dir(&dims_dir) {
                for e in entries.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    let ws = e.path().join("workspace");
                    if ws.exists() {
                        checked_dimensions.push((name, ws));
                    }
                }
            }
        }

        for (dim_name, ws_path) in checked_dimensions {
            for entry in WalkDir::new(&ws_path)
                .into_iter()
                .filter_entry(|e| {
                    let n = e.file_name().to_string_lossy();
                    n != ".dft" && n != ".git"
                })
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() {
                    if let Ok(rel) = entry.path().strip_prefix(&ws_path) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");

                        // Check fences
                        for fence in &fences {
                            if fence.hard
                                && fence.is_active(now)
                                && fence.is_violated_by(&rel_str, &dim_name)
                            {
                                violations.push(TerritoryViolation {
                                    file_path: rel_str.clone(),
                                    dimension: dim_name.clone(),
                                    violation_type: ViolationType::FenceBreach,
                                    message: format!(
                                        "File '{}' violates hard fence '{}'",
                                        rel_str, fence.path_glob
                                    ),
                                });
                            }
                        }

                        // Check claims
                        for claim in &active_claims {
                            if claim.exclusive
                                && claim.dimension != dim_name
                                && matches_glob(&claim.path_glob, &rel_str)
                            {
                                violations.push(TerritoryViolation {
                                    file_path: rel_str.clone(),
                                    dimension: dim_name.clone(),
                                    violation_type: ViolationType::ClaimBreach,
                                    message: format!(
                                        "File '{}' modified in dimension '{}' violates exclusive claim by '{}'",
                                        rel_str, dim_name, claim.dimension
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }

        let total_claims = active_claims.len() + expired_claims.len();
        let total_fences = fences.len();
        let active_count = active_claims.len();

        Ok(AuditReport {
            total_claims,
            active_claims: active_count,
            expired_claims,
            total_fences,
            collisions,
            fence_conflicts,
            violations,
            fixed_pruned_claims,
        })
    }
}
