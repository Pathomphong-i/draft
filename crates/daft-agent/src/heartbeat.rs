use crate::error::AgentError;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentLiveness {
    Active,
    Idle,
    Offline,
}

impl AgentLiveness {
    pub const ACTIVE_THRESHOLD_SECS: u64 = 60;
    pub const IDLE_THRESHOLD_SECS: u64 = 300;

    pub fn evaluate(last_heartbeat: Option<u64>, now: u64) -> Self {
        match last_heartbeat {
            Some(ts) if now >= ts => {
                let delta = now - ts;
                if delta < Self::ACTIVE_THRESHOLD_SECS {
                    AgentLiveness::Active
                } else if delta <= Self::IDLE_THRESHOLD_SECS {
                    AgentLiveness::Idle
                } else {
                    AgentLiveness::Offline
                }
            }
            _ => AgentLiveness::Offline,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatRecord {
    pub agent_name: String,
    pub timestamp: u64,
    pub timestamp_iso: String,
    pub pid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimension: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

pub struct HeartbeatSubsystem {
    agents_dir: PathBuf,
}

impl HeartbeatSubsystem {
    pub fn new(dft_dir: &Path) -> Self {
        Self {
            agents_dir: dft_dir.join("agents"),
        }
    }

    fn agent_dir(&self, agent_name: &str) -> PathBuf {
        self.agents_dir.join(agent_name)
    }

    fn heartbeat_file(&self, agent_name: &str) -> PathBuf {
        self.agent_dir(agent_name).join("heartbeat.json")
    }

    pub fn record(
        &self,
        agent_name: &str,
        dimension: Option<&str>,
        message: Option<&str>,
    ) -> Result<HeartbeatRecord, AgentError> {
        let dir = self.agent_dir(agent_name);
        fs::create_dir_all(&dir)?;

        let now = Utc::now();
        let record = HeartbeatRecord {
            agent_name: agent_name.to_string(),
            timestamp: now.timestamp() as u64,
            timestamp_iso: now.to_rfc3339(),
            pid: std::process::id(),
            dimension: dimension.map(|s| s.to_string()),
            message: message.map(|s| s.to_string()),
        };

        let temp_path = dir.join(format!(".heartbeat.{}.tmp", std::process::id()));
        let json = serde_json::to_string_pretty(&record)?;
        fs::write(&temp_path, json)?;
        fs::rename(temp_path, self.heartbeat_file(agent_name))?;

        // Dual-update legacy agent record if present
        let legacy_file = self.agents_dir.join(format!("{}.json", agent_name));
        if legacy_file.exists() {
            if let Ok(content) = fs::read_to_string(&legacy_file) {
                if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&content) {
                    val["last_heartbeat"] = serde_json::json!(now.to_rfc3339());
                    if let Some(dim) = dimension {
                        val["dimension"] = serde_json::json!(dim);
                    }
                    let _ = fs::write(
                        &legacy_file,
                        serde_json::to_string_pretty(&val).unwrap_or_default(),
                    );
                }
            }
        }

        Ok(record)
    }

    pub fn get(&self, agent_name: &str) -> Option<HeartbeatRecord> {
        let path = self.heartbeat_file(agent_name);
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(record) = serde_json::from_str::<HeartbeatRecord>(&content) {
                    return Some(record);
                }
            }
        }

        // Fallback: check legacy agent record
        let legacy_file = self.agents_dir.join(format!("{}.json", agent_name));
        if legacy_file.exists() {
            if let Ok(content) = fs::read_to_string(&legacy_file) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(ts_str) = val.get("last_heartbeat").and_then(|v| v.as_str()) {
                        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts_str) {
                            return Some(HeartbeatRecord {
                                agent_name: agent_name.to_string(),
                                timestamp: dt.timestamp() as u64,
                                timestamp_iso: ts_str.to_string(),
                                pid: 0,
                                dimension: val
                                    .get("dimension")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string()),
                                message: None,
                            });
                        }
                    }
                }
            }
        }

        None
    }

    pub fn liveness(&self, agent_name: &str) -> AgentLiveness {
        let record = self.get(agent_name);
        let now = Utc::now().timestamp() as u64;
        AgentLiveness::evaluate(record.map(|r| r.timestamp), now)
    }
}
