use crate::error::SyncError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SyncStrategy {
    #[default]
    Merge,
    Rebase,
    Theirs,
    Ours,
}

impl std::str::FromStr for SyncStrategy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "merge" => Ok(SyncStrategy::Merge),
            "rebase" => Ok(SyncStrategy::Rebase),
            "theirs" => Ok(SyncStrategy::Theirs),
            "ours" => Ok(SyncStrategy::Ours),
            other => Err(format!(
                "Unknown sync strategy: '{}'. Valid options: merge, rebase, theirs, ours",
                other
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WatchedDimension {
    pub dimension: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<SyncStrategy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval_secs: Option<u64>,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub paused: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_synced_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_synced_commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronosConfig {
    pub interval_secs: u64,
    pub default_strategy: SyncStrategy,
    #[serde(default)]
    pub watched_dimensions: Vec<WatchedDimension>,
    #[serde(default = "default_auto_entangle")]
    pub auto_entangle: bool,
}

fn default_auto_entangle() -> bool {
    true
}

impl Default for CronosConfig {
    fn default() -> Self {
        Self {
            interval_secs: 30,
            default_strategy: SyncStrategy::Merge,
            watched_dimensions: Vec::new(),
            auto_entangle: true,
        }
    }
}

pub struct ConfigStorage;

impl ConfigStorage {
    pub fn load_config(cronos_dir: &Path) -> Result<CronosConfig, SyncError> {
        let path = cronos_dir.join("config.json");
        if !path.exists() {
            return Ok(CronosConfig::default());
        }
        let content = fs::read_to_string(&path)?;
        let cfg = serde_json::from_str(&content).unwrap_or_default();
        Ok(cfg)
    }

    pub fn save_config(cronos_dir: &Path, config: &CronosConfig) -> Result<(), SyncError> {
        fs::create_dir_all(cronos_dir)?;
        let path = cronos_dir.join("config.json");
        let temp_path = cronos_dir.join(format!(".config.{}.tmp", std::process::id()));
        let json = serde_json::to_string_pretty(config)?;
        fs::write(&temp_path, json)?;
        fs::rename(temp_path, path)?;
        Ok(())
    }

    pub fn set_paused(cronos_dir: &Path, dimension: &str, paused: bool) -> Result<(), SyncError> {
        let mut cfg = Self::load_config(cronos_dir)?;
        let mut found = false;
        for wd in &mut cfg.watched_dimensions {
            if wd.dimension == dimension {
                wd.paused = paused;
                found = true;
            }
        }
        if !found {
            // Add as watched dimension with paused state
            cfg.watched_dimensions.push(WatchedDimension {
                dimension: dimension.to_string(),
                target: "mainline".to_string(),
                strategy: None,
                interval_secs: None,
                paths: Vec::new(),
                paused,
                last_synced_at: None,
                last_synced_commit: None,
            });
        }
        Self::save_config(cronos_dir, &cfg)?;
        Ok(())
    }
}
