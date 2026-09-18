use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Claim {
    pub id: String,
    pub path_glob: String,
    pub agent_id: Option<String>,
    pub dimension: String,
    pub exclusive: bool,
    pub created_at: DateTime<Utc>,
    pub ttl_seconds: Option<u64>,
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Claim {
    pub fn new(
        path_glob: String,
        dimension: String,
        agent_id: Option<String>,
        ttl_seconds: Option<u64>,
        exclusive: bool,
    ) -> Self {
        let created_at = Utc::now();
        let expires_at = ttl_seconds.map(|secs| created_at + Duration::seconds(secs as i64));
        let id_source = format!("{}:{}:{:?}", path_glob, dimension, agent_id);
        let oid = daft_core::cas::ObjectId::hash(id_source.as_bytes());
        let id = format!("claim_{}", &oid.to_hex()[..16]);

        Self {
            id,
            path_glob,
            agent_id,
            dimension,
            exclusive,
            created_at,
            ttl_seconds,
            expires_at,
            description: None,
        }
    }

    pub fn is_active(&self, now: DateTime<Utc>) -> bool {
        match self.expires_at {
            Some(exp) => now < exp,
            None => true,
        }
    }

    pub fn remaining_ttl(&self, now: DateTime<Utc>) -> Option<Duration> {
        self.expires_at.map(|exp| exp - now)
    }
}

/// Parses TTL string such as "30s", "15m", "2h", "1d", or numeric seconds.
pub fn parse_ttl_string(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Empty TTL string".to_string());
    }

    if let Some(num) = s.strip_suffix('s').or_else(|| s.strip_suffix('S')) {
        return num.parse::<u64>().map_err(|e| e.to_string());
    }
    if let Some(num) = s.strip_suffix('m').or_else(|| s.strip_suffix('M')) {
        return num
            .parse::<u64>()
            .map(|n| n * 60)
            .map_err(|e| e.to_string());
    }
    if let Some(num) = s.strip_suffix('h').or_else(|| s.strip_suffix('H')) {
        return num
            .parse::<u64>()
            .map(|n| n * 3600)
            .map_err(|e| e.to_string());
    }
    if let Some(num) = s.strip_suffix('d').or_else(|| s.strip_suffix('D')) {
        return num
            .parse::<u64>()
            .map(|n| n * 86400)
            .map_err(|e| e.to_string());
    }

    s.parse::<u64>().map_err(|e| e.to_string())
}
