use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fence {
    pub id: String,
    pub path_glob: String,
    pub owner_dimension: Option<String>,
    pub agent_id: Option<String>,
    pub hard: bool,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl Fence {
    pub fn new(
        path_glob: String,
        owner_dimension: Option<String>,
        agent_id: Option<String>,
        hard: bool,
        ttl_seconds: Option<u64>,
        reason: Option<String>,
    ) -> Self {
        let created_at = Utc::now();
        let expires_at = ttl_seconds.map(|secs| created_at + Duration::seconds(secs as i64));
        let id_source = format!("{}:{:?}:{:?}", path_glob, owner_dimension, agent_id);
        let oid = daft_core::cas::ObjectId::hash(id_source.as_bytes());
        let id = format!("fence_{}", &oid.to_hex()[..16]);

        Self {
            id,
            path_glob,
            owner_dimension,
            agent_id,
            hard,
            created_at,
            expires_at,
            reason,
        }
    }

    pub fn is_active(&self, now: DateTime<Utc>) -> bool {
        match self.expires_at {
            Some(exp) => now < exp,
            None => true,
        }
    }

    pub fn is_violated_by(&self, path: &str, modifying_dimension: &str) -> bool {
        if let Some(ref owner) = self.owner_dimension {
            if owner == modifying_dimension {
                return false;
            }
        }
        crate::territory::glob::matches_glob(&self.path_glob, path)
    }
}
