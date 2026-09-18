pub mod echo;
pub mod log;
pub mod propagate;
pub mod rule;

pub use echo::{compute_hash, EchoCancellation, FileProvenance};
pub use log::{AuditLogger, EntangleEvent};
pub use propagate::{PropagationResult, Propagator};
pub use rule::{EntangleDirection, EntangleRule, RuleStorage};

use crate::error::SyncError;
use chrono::Utc;
use daft_core::Repository;
use std::path::PathBuf;
use std::sync::Arc;

pub struct EntangleEngine {
    repo: Arc<Repository>,
    entangle_dir: PathBuf,
}

impl EntangleEngine {
    pub fn new(repo: Arc<Repository>) -> Self {
        let entangle_dir = repo.dft_dir().join("entangle");
        Self { repo, entangle_dir }
    }

    fn validate_dimension(&self, dim: &str) -> Result<(), SyncError> {
        if dim == "mainline" {
            return Ok(());
        }
        let dim_dir = self.repo.dft_dir().join("dimensions").join(dim);
        if !dim_dir.exists() {
            return Err(SyncError::DimensionNotFound(dim.to_string()));
        }
        Ok(())
    }

    pub fn link(
        &self,
        dim1: &str,
        dim2: &str,
        paths: Option<&str>,
        direction: EntangleDirection,
    ) -> Result<EntangleRule, SyncError> {
        if dim1 == dim2 {
            return Err(SyncError::InvalidRule(
                "Cannot entangle a dimension with itself".into(),
            ));
        }

        self.validate_dimension(dim1)?;
        self.validate_dimension(dim2)?;

        let mut rules = RuleStorage::load_rules(&self.entangle_dir)?;

        // If rule exists connecting these dimensions, update it
        let rule_id = format!("rule-{:08x}", Utc::now().timestamp_millis() as u32);
        let new_rule = EntangleRule {
            id: rule_id,
            dim1: dim1.to_string(),
            dim2: dim2.to_string(),
            direction,
            paths: paths.map(|s| s.to_string()),
            created_at: Utc::now(),
            active: true,
        };

        // Remove previous links between these two dimensions
        rules.retain(|r| {
            !((&r.dim1 == dim1 && &r.dim2 == dim2) || (&r.dim1 == dim2 && &r.dim2 == dim1))
        });

        rules.push(new_rule.clone());
        RuleStorage::save_rules(&self.entangle_dir, &rules)?;

        Ok(new_rule)
    }

    pub fn list(&self) -> Result<Vec<EntangleRule>, SyncError> {
        RuleStorage::load_rules(&self.entangle_dir)
    }

    pub fn sever(&self, dim1: &str, dim2: &str) -> Result<bool, SyncError> {
        let mut rules = RuleStorage::load_rules(&self.entangle_dir)?;
        let initial_len = rules.len();

        rules.retain(|r| {
            !((&r.dim1 == dim1 && &r.dim2 == dim2) || (&r.dim1 == dim2 && &r.dim2 == dim1))
        });

        if rules.len() != initial_len {
            RuleStorage::save_rules(&self.entangle_dir, &rules)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn propagate_rule(&self, rule_id: &str) -> Result<Vec<PropagationResult>, SyncError> {
        let rules = self.list()?;
        let rule = rules
            .iter()
            .find(|r| r.id == rule_id)
            .ok_or_else(|| SyncError::RuleNotFound(rule_id.to_string()))?;

        let session_id = format!(
            "sess-{}-{}",
            Utc::now().timestamp_millis(),
            std::process::id()
        );
        let propagator = Propagator::new(Arc::clone(&self.repo));
        propagator.propagate_rule(rule, &session_id)
    }

    pub fn propagate_all(&self) -> Result<Vec<PropagationResult>, SyncError> {
        let rules = self.list()?;
        let session_id = format!(
            "sess-{}-{}",
            Utc::now().timestamp_millis(),
            std::process::id()
        );
        let propagator = Propagator::new(Arc::clone(&self.repo));
        let mut all_results = Vec::new();

        for rule in &rules {
            if rule.active {
                if let Ok(results) = propagator.propagate_rule(rule, &session_id) {
                    all_results.extend(results);
                }
            }
        }

        Ok(all_results)
    }

    pub fn read_log(&self, limit: Option<usize>) -> Result<Vec<EntangleEvent>, SyncError> {
        AuditLogger::read_events(&self.entangle_dir, limit)
    }
}
