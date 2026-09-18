use crate::error::SyncError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntangleEvent {
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub rule_id: String,
    pub source_dim: String,
    pub target_dim: String,
    pub path: String,
    pub source_hash: String,
    pub target_previous_hash: String,
    pub action: String,
    pub session_id: String,
    pub status: String,
}

pub struct AuditLogger;

impl AuditLogger {
    pub fn append(entangle_dir: &Path, event: &EntangleEvent) -> Result<(), SyncError> {
        fs::create_dir_all(entangle_dir)?;
        let log_file = entangle_dir.join("log.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)?;

        let line = serde_json::to_string(event)?;
        writeln!(file, "{}", line)?;
        file.sync_data()?;
        Ok(())
    }

    pub fn read_events(
        entangle_dir: &Path,
        limit: Option<usize>,
    ) -> Result<Vec<EntangleEvent>, SyncError> {
        let log_file = entangle_dir.join("log.jsonl");
        if !log_file.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(&log_file)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(ev) = serde_json::from_str::<EntangleEvent>(&line) {
                events.push(ev);
            }
        }

        if let Some(n) = limit {
            if events.len() > n {
                events = events.into_iter().rev().take(n).rev().collect();
            }
        }

        Ok(events)
    }
}
