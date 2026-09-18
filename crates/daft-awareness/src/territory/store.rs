use crate::error::AwarenessError;
use crate::territory::claim::Claim;
use crate::territory::fence::Fence;
use crate::territory::lock::TerritoryLockGuard;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::NamedTempFile;

pub struct TerritoryStore {
    territory_dir: PathBuf,
    claims_dir: PathBuf,
    fences_dir: PathBuf,
}

impl TerritoryStore {
    pub fn new(territory_dir: &Path) -> Self {
        let claims_dir = territory_dir.join("claims");
        let fences_dir = territory_dir.join("fences");
        Self {
            territory_dir: territory_dir.to_path_buf(),
            claims_dir,
            fences_dir,
        }
    }

    pub fn territory_dir(&self) -> &Path {
        &self.territory_dir
    }

    pub fn lock(
        &self,
        agent_id: Option<&str>,
        op: &str,
    ) -> Result<TerritoryLockGuard, AwarenessError> {
        TerritoryLockGuard::acquire(&self.territory_dir, agent_id, op, Duration::from_secs(5))
    }

    fn init_dirs(&self) -> Result<(), AwarenessError> {
        fs::create_dir_all(&self.claims_dir)?;
        fs::create_dir_all(&self.fences_dir)?;
        Ok(())
    }

    pub fn save_claim(&self, claim: &Claim) -> Result<(), AwarenessError> {
        self.init_dirs()?;
        let target_path = self.claims_dir.join(format!("{}.json", claim.id));
        let mut tmp = NamedTempFile::new_in(&self.claims_dir)?;
        let json = serde_json::to_string_pretty(claim)?;
        tmp.write_all(json.as_bytes())?;
        tmp.as_file().sync_all()?;
        tmp.persist(&target_path)
            .map_err(|e| AwarenessError::Io(e.error))?;

        self.refresh_claims_cache()?;
        Ok(())
    }

    pub fn delete_claim(&self, claim_id: &str) -> Result<bool, AwarenessError> {
        let path = self.claims_dir.join(format!("{}.json", claim_id));
        if path.exists() {
            fs::remove_file(path)?;
            self.refresh_claims_cache()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn load_all_claims(&self) -> Result<Vec<Claim>, AwarenessError> {
        let mut claims = Vec::new();
        if !self.claims_dir.exists() {
            return Ok(claims);
        }
        for entry in fs::read_dir(&self.claims_dir)?.flatten() {
            let p = entry.path();
            if p.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&p) {
                    if let Ok(c) = serde_json::from_str::<Claim>(&content) {
                        claims.push(c);
                    }
                }
            }
        }
        claims.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(claims)
    }

    fn refresh_claims_cache(&self) -> Result<(), AwarenessError> {
        let claims = self.load_all_claims()?;
        let cache_path = self.territory_dir.join("claims.json");
        let mut tmp = NamedTempFile::new_in(&self.territory_dir)?;
        let json = serde_json::to_string_pretty(&claims)?;
        tmp.write_all(json.as_bytes())?;
        tmp.as_file().sync_all()?;
        tmp.persist(&cache_path)
            .map_err(|e| AwarenessError::Io(e.error))?;
        Ok(())
    }

    pub fn save_fence(&self, fence: &Fence) -> Result<(), AwarenessError> {
        self.init_dirs()?;
        let target_path = self.fences_dir.join(format!("{}.json", fence.id));
        let mut tmp = NamedTempFile::new_in(&self.fences_dir)?;
        let json = serde_json::to_string_pretty(fence)?;
        tmp.write_all(json.as_bytes())?;
        tmp.as_file().sync_all()?;
        tmp.persist(&target_path)
            .map_err(|e| AwarenessError::Io(e.error))?;

        self.refresh_fences_cache()?;
        Ok(())
    }

    pub fn delete_fence(&self, fence_id: &str) -> Result<bool, AwarenessError> {
        let path = self.fences_dir.join(format!("{}.json", fence_id));
        if path.exists() {
            fs::remove_file(path)?;
            self.refresh_fences_cache()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn load_all_fences(&self) -> Result<Vec<Fence>, AwarenessError> {
        let mut fences = Vec::new();
        if !self.fences_dir.exists() {
            return Ok(fences);
        }
        for entry in fs::read_dir(&self.fences_dir)?.flatten() {
            let p = entry.path();
            if p.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&p) {
                    if let Ok(f) = serde_json::from_str::<Fence>(&content) {
                        fences.push(f);
                    }
                }
            }
        }
        fences.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(fences)
    }

    fn refresh_fences_cache(&self) -> Result<(), AwarenessError> {
        let fences = self.load_all_fences()?;
        let cache_path = self.territory_dir.join("fences.json");
        let mut tmp = NamedTempFile::new_in(&self.territory_dir)?;
        let json = serde_json::to_string_pretty(&fences)?;
        tmp.write_all(json.as_bytes())?;
        tmp.as_file().sync_all()?;
        tmp.persist(&cache_path)
            .map_err(|e| AwarenessError::Io(e.error))?;
        Ok(())
    }
}
