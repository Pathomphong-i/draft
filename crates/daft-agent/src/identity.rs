use crate::error::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    Human,
    Ai,
}

impl std::str::FromStr for AgentType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "human" => Ok(AgentType::Human),
            "ai" => Ok(AgentType::Ai),
            other => Err(format!(
                "Unknown agent type: '{}'. Valid options: human, ai",
                other
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentIdentity {
    pub id: String,
    pub name: String,
    pub agent_type: AgentType,
    pub registered_at: u64,
    pub assigned_dimension: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

impl AgentIdentity {
    pub fn validate_name(name: &str) -> Result<(), AgentError> {
        if name.is_empty() {
            return Err(AgentError::InvalidAgentName(
                name.to_string(),
                "Name cannot be empty".to_string(),
            ));
        }

        if name.contains('/') || name.contains('\\') || name.contains('\0') {
            return Err(AgentError::InvalidAgentName(
                name.to_string(),
                "Name cannot contain path separators or null bytes".to_string(),
            ));
        }

        let is_valid = name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.');
        if !is_valid {
            return Err(AgentError::InvalidAgentName(
                name.to_string(),
                "Name must contain only alphanumeric, dash, underscore, or dot characters"
                    .to_string(),
            ));
        }

        Ok(())
    }
}
