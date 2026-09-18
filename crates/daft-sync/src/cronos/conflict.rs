use crate::error::SyncError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictRecord {
    pub path: String,
    pub reason: String,
    pub base_hash: Option<String>,
    pub source_hash: String,
    pub target_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncLogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub tx_id: String,
    pub source_dim: String,
    pub target_dim: String,
    pub strategy: String,
    pub action: String,
    #[serde(default)]
    pub conflicts: Vec<ConflictRecord>,
    pub message: String,
}

pub struct ConflictLogger;

impl ConflictLogger {
    pub fn append(cronos_dir: &Path, entry: &SyncLogEntry) -> Result<(), SyncError> {
        fs::create_dir_all(cronos_dir)?;
        let log_file = cronos_dir.join("sync.log");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)?;

        let line = serde_json::to_string(entry)?;
        writeln!(file, "{}", line)?;
        file.sync_data()?;
        Ok(())
    }

    pub fn read_entries(
        cronos_dir: &Path,
        limit: Option<usize>,
    ) -> Result<Vec<SyncLogEntry>, SyncError> {
        let log_file = cronos_dir.join("sync.log");
        if !log_file.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(&log_file)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(entry) = serde_json::from_str::<SyncLogEntry>(&line) {
                entries.push(entry);
            }
        }

        if let Some(n) = limit {
            if entries.len() > n {
                entries = entries.into_iter().rev().take(n).rev().collect();
            }
        }

        Ok(entries)
    }
}
