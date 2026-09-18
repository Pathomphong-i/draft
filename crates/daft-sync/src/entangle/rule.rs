use crate::error::SyncError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EntangleDirection {
    #[default]
    Bidirectional,
    Unidirectional,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntangleRule {
    pub id: String,
    pub dim1: String,
    pub dim2: String,
    #[serde(default)]
    pub direction: EntangleDirection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paths: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(default = "default_active")]
    pub active: bool,
}

fn default_active() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntangleRuleStore {
    pub version: u32,
    pub rules: Vec<EntangleRule>,
}

pub struct RuleStorage;

impl RuleStorage {
    pub fn load_rules(entangle_dir: &Path) -> Result<Vec<EntangleRule>, SyncError> {
        let rules_path = entangle_dir.join("rules.json");
        if !rules_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&rules_path)?;
        // Handle both raw Vec<EntangleRule> and EntangleRuleStore
        if let Ok(store) = serde_json::from_str::<EntangleRuleStore>(&content) {
            return Ok(store.rules);
        }
        if let Ok(rules) = serde_json::from_str::<Vec<EntangleRule>>(&content) {
            return Ok(rules);
        }

        // Try backward compatibility with minimal json schema
        #[derive(Deserialize)]
        struct LegacyRule {
            dim1: String,
            dim2: String,
            paths: Option<String>,
        }
        if let Ok(legacy) = serde_json::from_str::<Vec<LegacyRule>>(&content) {
            let converted = legacy
                .into_iter()
                .enumerate()
                .map(|(idx, r)| EntangleRule {
                    id: format!("rule-{:08x}", idx + 1),
                    dim1: r.dim1,
                    dim2: r.dim2,
                    direction: EntangleDirection::Bidirectional,
                    paths: r.paths,
                    created_at: Utc::now(),
                    active: true,
                })
                .collect();
            return Ok(converted);
        }

        Ok(Vec::new())
    }

    pub fn save_rules(entangle_dir: &Path, rules: &[EntangleRule]) -> Result<(), SyncError> {
        fs::create_dir_all(entangle_dir)?;
        let rules_path = entangle_dir.join("rules.json");

        let store = EntangleRuleStore {
            version: 1,
            rules: rules.to_vec(),
        };

        let temp_path = entangle_dir.join(format!(".rules.{}.tmp", std::process::id()));
        let json = serde_json::to_string_pretty(&store)?;
        fs::write(&temp_path, json)?;
        fs::rename(temp_path, rules_path)?;

        Ok(())
    }
}
