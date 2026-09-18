use chrono::Utc;
use serde::{Deserialize, Serialize};

use std::sync::atomic::{AtomicU64, Ordering};

static MSG_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMessage {
    pub id: String,
    pub sender: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
    pub timestamp: u64,
    pub timestamp_iso: String,
    pub subject: String,
    pub body: String,
    pub read: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimension: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<String>,
}

impl AgentMessage {
    pub fn new_broadcast(sender: &str, subject: &str, body: &str) -> Self {
        let now = Utc::now();
        let counter = MSG_COUNTER.fetch_add(1, Ordering::Relaxed);
        Self {
            id: format!(
                "msg-{}-{}-{}",
                now.timestamp_nanos_opt().unwrap_or(0),
                std::process::id(),
                counter
            ),
            sender: sender.to_string(),
            recipient: None,
            timestamp: now.timestamp() as u64,
            timestamp_iso: now.to_rfc3339(),
            subject: subject.to_string(),
            body: body.to_string(),
            read: false,
            dimension: None,
            in_reply_to: None,
        }
    }

    pub fn new_direct(sender: &str, recipient: &str, subject: &str, body: &str) -> Self {
        let now = Utc::now();
        let counter = MSG_COUNTER.fetch_add(1, Ordering::Relaxed);
        Self {
            id: format!(
                "msg-{}-{}-{}",
                now.timestamp_nanos_opt().unwrap_or(0),
                std::process::id(),
                counter
            ),
            sender: sender.to_string(),
            recipient: Some(recipient.to_string()),
            timestamp: now.timestamp() as u64,
            timestamp_iso: now.to_rfc3339(),
            subject: subject.to_string(),
            body: body.to_string(),
            read: false,
            dimension: None,
            in_reply_to: None,
        }
    }
}
