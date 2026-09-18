pub mod audit;
pub mod claim;
pub mod fence;
pub mod glob;
pub mod lock;
pub mod store;

pub use audit::{AuditReport, ClaimCollision, FenceConflict, TerritoryViolation, ViolationType};
pub use claim::{parse_ttl_string, Claim};
pub use fence::Fence;
pub use glob::{matches_glob, normalize_glob, patterns_overlap};
pub use lock::{TerritoryLockGuard, TerritoryLockInfo};
pub use store::TerritoryStore;

use crate::error::AwarenessError;
use chrono::Utc;
use std::path::{Path, PathBuf};

pub struct TerritoryManager {
    dft_dir: PathBuf,
    store: TerritoryStore,
}

impl TerritoryManager {
    pub fn new(dft_dir: &Path) -> Self {
        let territory_dir = dft_dir.join("territory");
        Self {
            dft_dir: dft_dir.to_path_buf(),
            store: TerritoryStore::new(&territory_dir),
        }
    }

    pub fn store(&self) -> &TerritoryStore {
        &self.store
    }

    /// Primary entry point for registering a territory claim.
    pub fn claim(
        &self,
        path_glob: &str,
        dimension: &str,
        agent_id: Option<&str>,
        ttl_seconds: Option<u64>,
        exclusive: bool,
    ) -> Result<Claim, AwarenessError> {
        let _guard = self.store.lock(agent_id, &format!("claim:{}", path_glob))?;

        let now = Utc::now();
        let claims = self.store.load_all_claims()?;

        for existing in &claims {
            if existing.is_active(now) {
                // Renewal if same owner
                if existing.dimension == dimension
                    && existing.agent_id.as_deref() == agent_id
                    && existing.path_glob == path_glob
                {
                    continue;
                }

                if patterns_overlap(&existing.path_glob, path_glob) {
                    if exclusive || existing.exclusive {
                        return Err(AwarenessError::TerritoryConflict(format!(
                            "Path '{}' conflicts with active claim '{}' held by dimension '{}' (agent: {:?})",
                            path_glob,
                            existing.id,
                            existing.dimension,
                            existing.agent_id
                        )));
                    }
                }
            }
        }

        let new_claim = Claim::new(
            path_glob.to_string(),
            dimension.to_string(),
            agent_id.map(|s| s.to_string()),
            ttl_seconds,
            exclusive,
        );

        self.store.save_claim(&new_claim)?;
        Ok(new_claim)
    }

    /// Releases claims matching a path or glob.
    pub fn yield_path(
        &self,
        path_glob: &str,
        agent_id: Option<&str>,
        dimension: Option<&str>,
    ) -> Result<Vec<Claim>, AwarenessError> {
        let _guard = self.store.lock(agent_id, &format!("yield:{}", path_glob))?;
        let claims = self.store.load_all_claims()?;
        let mut released = Vec::new();

        for claim in claims {
            let matches_path = claim.path_glob == path_glob
                || matches_glob(&claim.path_glob, path_glob)
                || matches_glob(path_glob, &claim.path_glob);

            if matches_path {
                if let Some(ag) = agent_id {
                    if claim.agent_id.as_deref() != Some(ag) {
                        continue;
                    }
                }
                if let Some(dim) = dimension {
                    if claim.dimension != dim {
                        continue;
                    }
                }
                self.store.delete_claim(&claim.id)?;
                released.push(claim);
            }
        }

        Ok(released)
    }

    /// Releases all claims for an agent.
    pub fn yield_agent(
        &self,
        agent_id: &str,
        dimension: Option<&str>,
    ) -> Result<Vec<Claim>, AwarenessError> {
        let _guard = self.store.lock(Some(agent_id), "yield_agent")?;
        let claims = self.store.load_all_claims()?;
        let mut released = Vec::new();

        for claim in claims {
            if claim.agent_id.as_deref() == Some(agent_id) {
                if let Some(dim) = dimension {
                    if claim.dimension != dim {
                        continue;
                    }
                }
                self.store.delete_claim(&claim.id)?;
                released.push(claim);
            }
        }

        Ok(released)
    }

    /// Releases all claims.
    pub fn yield_all(&self) -> Result<Vec<Claim>, AwarenessError> {
        let _guard = self.store.lock(None, "yield_all")?;
        let claims = self.store.load_all_claims()?;
        for claim in &claims {
            self.store.delete_claim(&claim.id)?;
        }
        Ok(claims)
    }

    /// Creates an exclusionary write barrier (fence).
    pub fn fence(
        &self,
        path_glob: &str,
        owner_dimension: Option<&str>,
        agent_id: Option<&str>,
        hard: bool,
        ttl_seconds: Option<u64>,
        reason: Option<&str>,
    ) -> Result<Fence, AwarenessError> {
        let _guard = self.store.lock(agent_id, &format!("fence:{}", path_glob))?;
        let new_fence = Fence::new(
            path_glob.to_string(),
            owner_dimension.map(|s| s.to_string()),
            agent_id.map(|s| s.to_string()),
            hard,
            ttl_seconds,
            reason.map(|s| s.to_string()),
        );
        self.store.save_fence(&new_fence)?;
        Ok(new_fence)
    }

    /// Removes a fence by path glob.
    pub fn unfence(&self, path_glob: &str) -> Result<Option<Fence>, AwarenessError> {
        let _guard = self.store.lock(None, &format!("unfence:{}", path_glob))?;
        let fences = self.store.load_all_fences()?;
        for fence in fences {
            if fence.path_glob == path_glob {
                self.store.delete_fence(&fence.id)?;
                return Ok(Some(fence));
            }
        }
        Ok(None)
    }

    /// Lists active claims.
    pub fn list_claims(&self) -> Result<Vec<Claim>, AwarenessError> {
        let now = Utc::now();
        let claims = self.store.load_all_claims()?;
        Ok(claims.into_iter().filter(|c| c.is_active(now)).collect())
    }

    /// Lists active fences.
    pub fn list_fences(&self) -> Result<Vec<Fence>, AwarenessError> {
        let now = Utc::now();
        let fences = self.store.load_all_fences()?;
        Ok(fences.into_iter().filter(|f| f.is_active(now)).collect())
    }

    /// Runs forensic audit over claims, fences, and workspaces.
    pub fn audit(&self, fix: bool) -> Result<AuditReport, AwarenessError> {
        let engine = audit::AuditEngine::new(&self.store, &self.dft_dir);
        engine.audit(fix)
    }

    /// Checks if staging the given paths is allowed.
    pub fn check_stage_allowed(
        &self,
        dimension: &str,
        paths: &[&Path],
    ) -> Result<(), AwarenessError> {
        let fences = self.list_fences()?;
        for path in paths {
            let path_str = path
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches("./")
                .to_string();
            for fence in &fences {
                if fence.hard && fence.is_violated_by(&path_str, dimension) {
                    return Err(AwarenessError::General(format!(
                        "Operation blocked by territory fence: FENCE on '{}'",
                        path_str
                    )));
                }
            }
        }
        Ok(())
    }

    /// Checks if committing the given staged paths is allowed.
    pub fn check_commit_allowed(
        &self,
        dimension: &str,
        paths: &[&Path],
    ) -> Result<(), AwarenessError> {
        self.check_stage_allowed(dimension, paths)
    }
}
