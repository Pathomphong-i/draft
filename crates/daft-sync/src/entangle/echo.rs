use crate::error::SyncError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileProvenance {
    pub path: String,
    pub content_hash: String,
    pub origin_dim: String,
    pub session_id: String,
    pub propagated_at: DateTime<Utc>,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProvenanceStore {
    pub entries: HashMap<String, FileProvenance>,
}

pub fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

pub struct EchoCancellation {
    entangle_dir: std::path::PathBuf,
}

impl EchoCancellation {
    pub fn new(entangle_dir: &Path) -> Self {
        Self {
            entangle_dir: entangle_dir.to_path_buf(),
        }
    }

    fn provenance_file(&self) -> std::path::PathBuf {
        self.entangle_dir.join("provenance.json")
    }

    pub fn load_store(&self) -> ProvenanceStore {
        let path = self.provenance_file();
        if !path.exists() {
            return ProvenanceStore::default();
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_store(&self, store: &ProvenanceStore) -> Result<(), SyncError> {
        fs::create_dir_all(&self.entangle_dir)?;
        let path = self.provenance_file();
        let temp_path = self
            .entangle_dir
            .join(format!(".provenance.{}.tmp", std::process::id()));
        let json = serde_json::to_string_pretty(store)?;
        fs::write(&temp_path, json)?;
        fs::rename(temp_path, path)?;
        Ok(())
    }

    /// Retrieve the last known synchronized content hash for a dimension and path.
    pub fn get_last_synced_hash(&self, dim: &str, rel_path: &str) -> Option<String> {
        let store = self.load_store();
        let key = format!("{}:{}", dim, rel_path);
        store.entries.get(&key).and_then(|p| {
            if p.deleted {
                None
            } else {
                Some(p.content_hash.clone())
            }
        })
    }

    /// Check if a path was previously synchronized in this dimension.
    pub fn is_previously_synced(&self, dim: &str, rel_path: &str) -> bool {
        let store = self.load_store();
        let key = format!("{}:{}", dim, rel_path);
        store.entries.contains_key(&key)
    }

    /// Checks if propagating a file from source_dim to target_dim constitutes an echo loop.
    pub fn is_echo(
        &self,
        rel_path: &str,
        source_hash: &str,
        target_hash: Option<&str>,
        source_dim: &str,
        target_dim: &str,
    ) -> bool {
        // Tier 1: Identical content hash between source and target means no propagation needed
        if let Some(th) = target_hash {
            if th == source_hash {
                return true;
            }
        }

        // Tier 2: Check provenance registry
        let store = self.load_store();
        let key = format!("{}:{}", target_dim, rel_path);
        if let Some(prov) = store.entries.get(&key) {
            // If target already has the content originated from source_dim, don't ping-pong
            if !prov.deleted && prov.content_hash == source_hash && prov.origin_dim == source_dim {
                return true;
            }
        }

        // Check if source file is simply an echo of an earlier target_dim edit
        let source_key = format!("{}:{}", source_dim, rel_path);
        if let Some(source_prov) = store.entries.get(&source_key) {
            if !source_prov.deleted
                && source_prov.content_hash == source_hash
                && source_prov.origin_dim == target_dim
            {
                // The file in source_dim was propagated from target_dim;
                // if target has unchanged hash matching this provenance, do not bounce back!
                if let Some(th) = target_hash {
                    if th == source_hash {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Records synchronized state for BOTH source and target dimensions.
    pub fn record_sync(
        &self,
        rel_path: &str,
        content_hash: &str,
        origin_dim: &str,
        target_dim: &str,
        session_id: &str,
    ) -> Result<(), SyncError> {
        let mut store = self.load_store();
        let now = Utc::now();

        // Record for target
        store.entries.insert(
            format!("{}:{}", target_dim, rel_path),
            FileProvenance {
                path: rel_path.to_string(),
                content_hash: content_hash.to_string(),
                origin_dim: origin_dim.to_string(),
                session_id: session_id.to_string(),
                propagated_at: now,
                deleted: false,
            },
        );

        // Record for source
        store.entries.insert(
            format!("{}:{}", origin_dim, rel_path),
            FileProvenance {
                path: rel_path.to_string(),
                content_hash: content_hash.to_string(),
                origin_dim: origin_dim.to_string(),
                session_id: session_id.to_string(),
                propagated_at: now,
                deleted: false,
            },
        );

        self.save_store(&store)
    }

    /// Records tombstone deletion for both dimensions.
    pub fn record_deletion(
        &self,
        rel_path: &str,
        dim1: &str,
        dim2: &str,
        session_id: &str,
    ) -> Result<(), SyncError> {
        let mut store = self.load_store();
        let now = Utc::now();

        for dim in &[dim1, dim2] {
            let key = format!("{}:{}", dim, rel_path);
            store.entries.insert(
                key,
                FileProvenance {
                    path: rel_path.to_string(),
                    content_hash: String::new(),
                    origin_dim: dim.to_string(),
                    session_id: session_id.to_string(),
                    propagated_at: now,
                    deleted: true,
                },
            );
        }

        self.save_store(&store)
    }

    /// Records provenance for a successfully propagated file (backward compatibility).
    pub fn record(
        &self,
        rel_path: &str,
        content_hash: &str,
        origin_dim: &str,
        target_dim: &str,
        session_id: &str,
    ) -> Result<(), SyncError> {
        self.record_sync(rel_path, content_hash, origin_dim, target_dim, session_id)
    }
}
