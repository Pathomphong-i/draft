use crate::cronos::config::SyncStrategy;
use crate::error::SyncError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum WalStep {
    TxBegin {
        source_dim: String,
        target_dim: String,
        strategy: SyncStrategy,
        source_head: String,
        target_head: String,
    },
    LocksAcquired {
        dimensions: Vec<String>,
    },
    TreePrepared {
        base_commit: Option<String>,
        result_tree_oid: String,
    },
    CommitCreated {
        commit_oid: String,
    },
    HeadUpdated {
        target_dim: String,
        old_oid: String,
        new_oid: String,
    },
    WorkspaceUpdated {
        files_updated: usize,
    },
    TxCommit {
        completion_time: DateTime<Utc>,
    },
    TxAbort {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalRecord {
    pub seq: u64,
    pub tx_id: String,
    pub timestamp: DateTime<Utc>,
    pub step: WalStep,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecoveryReport {
    pub transactions_analyzed: usize,
    pub recovered_transactions: usize,
    pub rolled_back: usize,
    pub errors: Vec<String>,
}

pub struct WalEngine {
    wal_path: PathBuf,
}

impl WalEngine {
    pub fn new(cronos_dir: &Path) -> Self {
        Self {
            wal_path: cronos_dir.join("wal.log"),
        }
    }

    pub fn append(&self, tx_id: &str, step: WalStep) -> Result<u64, SyncError> {
        if let Some(p) = self.wal_path.parent() {
            fs::create_dir_all(p)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.wal_path)?;

        let seq = Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
        let record = WalRecord {
            seq,
            tx_id: tx_id.to_string(),
            timestamp: Utc::now(),
            step,
        };

        let json = serde_json::to_string(&record)?;
        writeln!(file, "{}", json)?;
        file.sync_data()?;
        Ok(seq)
    }

    pub fn read_all(&self) -> Result<Vec<WalRecord>, SyncError> {
        if !self.wal_path.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(&self.wal_path)?;
        let reader = BufReader::new(file);
        let mut records = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(rec) = serde_json::from_str::<WalRecord>(&line) {
                records.push(rec);
            }
        }

        Ok(records)
    }

    pub fn verify(&self) -> Result<usize, SyncError> {
        let records = self.read_all()?;
        Ok(records.len())
    }

    pub fn recover(&self) -> Result<RecoveryReport, SyncError> {
        let records = self.read_all()?;
        let mut report = RecoveryReport::default();

        use std::collections::HashMap;
        let mut tx_steps: HashMap<String, Vec<WalStep>> = HashMap::new();

        for r in records {
            tx_steps.entry(r.tx_id).or_default().push(r.step);
        }

        report.transactions_analyzed = tx_steps.len();

        for (tx_id, steps) in tx_steps {
            let has_commit = steps.iter().any(|s| matches!(s, WalStep::TxCommit { .. }));
            let has_abort = steps.iter().any(|s| matches!(s, WalStep::TxAbort { .. }));

            if !has_commit && !has_abort {
                // Incomplete transaction found!
                let has_head_updated = steps
                    .iter()
                    .any(|s| matches!(s, WalStep::HeadUpdated { .. }));
                let has_ws_updated = steps
                    .iter()
                    .any(|s| matches!(s, WalStep::WorkspaceUpdated { .. }));

                if has_head_updated && has_ws_updated {
                    // Completed mutation but missed commit record
                    self.append(
                        &tx_id,
                        WalStep::TxCommit {
                            completion_time: Utc::now(),
                        },
                    )?;
                    report.recovered_transactions += 1;
                } else {
                    // Interrupted before complete state update -> Rollback
                    self.append(
                        &tx_id,
                        WalStep::TxAbort {
                            reason: "Interrupted transaction rolled back on startup".into(),
                        },
                    )?;
                    report.rolled_back += 1;
                }
            }
        }

        Ok(report)
    }
}
