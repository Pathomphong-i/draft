use crate::error::AgentError;
use crate::identity::{AgentIdentity, AgentType};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegistryStore {
    pub version: u32,
    pub agents: Vec<AgentIdentity>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct LegacyAgentRecord {
    pub name: String,
    pub agent_type: String,
    pub dimension: String,
    pub last_heartbeat: String,
}

pub struct AgentRegistry {
    agents_dir: PathBuf,
    dft_dir: PathBuf,
}

impl AgentRegistry {
    pub fn new(dft_dir: &Path) -> Self {
        Self {
            agents_dir: dft_dir.join("agents"),
            dft_dir: dft_dir.to_path_buf(),
        }
    }

    fn registry_path(&self) -> PathBuf {
        self.agents_dir.join("registry.json")
    }

    fn legacy_agent_path(&self, name: &str) -> PathBuf {
        self.agents_dir.join(format!("{}.json", name))
    }

    pub fn load_registry(&self) -> Result<RegistryStore, AgentError> {
        let mut store = if self.registry_path().exists() {
            let content = fs::read_to_string(self.registry_path())?;
            serde_json::from_str::<RegistryStore>(&content).unwrap_or_default()
        } else {
            RegistryStore {
                version: 1,
                agents: Vec::new(),
            }
        };

        // Scan for any legacy individual agent json files to ensure complete sync
        if let Ok(entries) = fs::read_dir(&self.agents_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    let file_name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    if file_name != "registry.json" && file_name != "global_inbox.json" {
                        let agent_name = file_name.trim_end_matches(".json");
                        if !store.agents.iter().any(|a| a.name == agent_name) {
                            if let Ok(content) = fs::read_to_string(&path) {
                                if let Ok(rec) = serde_json::from_str::<LegacyAgentRecord>(&content)
                                {
                                    let agent_type = rec
                                        .agent_type
                                        .parse::<AgentType>()
                                        .unwrap_or(AgentType::Ai);
                                    store.agents.push(AgentIdentity {
                                        id: format!("agent-{}", rec.name),
                                        name: rec.name,
                                        agent_type,
                                        registered_at: Utc::now().timestamp() as u64,
                                        assigned_dimension: Some(rec.dimension),
                                        capabilities: Vec::new(),
                                        metadata: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(store)
    }

    pub fn save_registry(&self, store: &RegistryStore) -> Result<(), AgentError> {
        fs::create_dir_all(&self.agents_dir)?;
        let temp_path = self
            .agents_dir
            .join(format!(".registry.{}.tmp", std::process::id()));
        let json = serde_json::to_string_pretty(store)?;
        fs::write(&temp_path, json)?;
        fs::rename(temp_path, self.registry_path())?;

        // Dual-write individual legacy records
        for a in &store.agents {
            let legacy_rec = LegacyAgentRecord {
                name: a.name.clone(),
                agent_type: match a.agent_type {
                    AgentType::Human => "human".to_string(),
                    AgentType::Ai => "ai".to_string(),
                },
                dimension: a
                    .assigned_dimension
                    .clone()
                    .unwrap_or_else(|| "mainline".to_string()),
                last_heartbeat: Utc::now().to_rfc3339(),
            };
            let leg_path = self.legacy_agent_path(&a.name);
            let _ = fs::write(
                leg_path,
                serde_json::to_string_pretty(&legacy_rec).unwrap_or_default(),
            );
        }

        Ok(())
    }

    fn validate_dimension_exists(&self, dimension: &str) -> Result<(), AgentError> {
        if dimension == "mainline" {
            return Ok(());
        }
        let dim_dir = self.dft_dir.join("dimensions").join(dimension);
        if !dim_dir.exists() {
            return Err(AgentError::DimensionNotFound(dimension.to_string()));
        }
        Ok(())
    }

    pub fn register(
        &self,
        name: &str,
        agent_type: AgentType,
        capabilities: Vec<String>,
    ) -> Result<AgentIdentity, AgentError> {
        AgentIdentity::validate_name(name)?;

        let mut store = self.load_registry()?;

        // Also check if legacy file exists
        if store.agents.iter().any(|a| a.name == name) || self.legacy_agent_path(name).exists() {
            return Err(AgentError::AgentAlreadyExists(name.to_string()));
        }

        let identity = AgentIdentity {
            id: format!("agent-{}", name),
            name: name.to_string(),
            agent_type,
            registered_at: Utc::now().timestamp() as u64,
            assigned_dimension: Some("mainline".to_string()),
            capabilities,
            metadata: None,
        };

        store.agents.push(identity.clone());
        self.save_registry(&store)?;

        Ok(identity)
    }

    pub fn list(&self) -> Result<Vec<AgentIdentity>, AgentError> {
        let store = self.load_registry()?;
        Ok(store.agents)
    }

    pub fn get(&self, name_or_id: &str) -> Result<AgentIdentity, AgentError> {
        let store = self.load_registry()?;
        store
            .agents
            .into_iter()
            .find(|a| a.name == name_or_id || a.id == name_or_id)
            .ok_or_else(|| AgentError::AgentNotFound(name_or_id.to_string()))
    }

    pub fn assign(&self, agent_name: &str, dimension: &str) -> Result<AgentIdentity, AgentError> {
        self.validate_dimension_exists(dimension)?;

        let mut store = self.load_registry()?;
        let agent = store
            .agents
            .iter_mut()
            .find(|a| a.name == agent_name || a.id == agent_name)
            .ok_or_else(|| AgentError::AgentNotFound(agent_name.to_string()))?;

        agent.assigned_dimension = Some(dimension.to_string());
        let updated = agent.clone();

        self.save_registry(&store)?;

        Ok(updated)
    }
}
