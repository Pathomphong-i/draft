use crate::error::AgentError;
use crate::heartbeat::{AgentLiveness, HeartbeatSubsystem};
use crate::identity::AgentType;
use crate::registry::AgentRegistry;
use chrono::Utc;
use daft_core::index::Stage;
use daft_core::object::Commit;
use daft_core::Repository;
use daft_dimension::DimensionRepository;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentActivity {
    pub agent_id: String,
    pub name: String,
    pub agent_type: AgentType,
    pub assigned_dimension: String,
    pub liveness: AgentLiveness,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds_since_heartbeat: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_heartbeat_timestamp: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_commit_oid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_commit_message: Option<String>,
    pub staged_files: Vec<String>,
    pub modified_files: Vec<String>,
    pub untracked_files: Vec<String>,
    pub total_dirty_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmStatusReport {
    pub timestamp: u64,
    pub total_agents: usize,
    pub active_agents: usize,
    pub idle_agents: usize,
    pub offline_agents: usize,
    pub agents: Vec<AgentActivity>,
}

pub struct StatusAggregator {
    repo: Arc<Repository>,
    dft_dir: std::path::PathBuf,
}

impl StatusAggregator {
    pub fn new(repo: Arc<Repository>) -> Self {
        let dft_dir = repo.dft_dir().to_path_buf();
        Self { repo, dft_dir }
    }

    pub fn aggregate(&self, target_agent: Option<&str>) -> Result<SwarmStatusReport, AgentError> {
        let registry = AgentRegistry::new(&self.dft_dir);
        let heartbeats = HeartbeatSubsystem::new(&self.dft_dir);
        let now = Utc::now().timestamp() as u64;

        let mut agents = registry.list()?;
        if let Some(name) = target_agent {
            agents.retain(|a| a.name == name || a.id == name);
        }

        let mut activities = Vec::new();
        let mut active_count = 0;
        let mut idle_count = 0;
        let mut offline_count = 0;

        for agent in agents {
            let dim_name = agent.assigned_dimension.as_deref().unwrap_or("mainline");

            let hb = heartbeats.get(&agent.name);
            let liveness = AgentLiveness::evaluate(hb.as_ref().map(|h| h.timestamp), now);
            let seconds_since = hb.as_ref().map(|h| {
                if now >= h.timestamp {
                    now - h.timestamp
                } else {
                    0
                }
            });

            match liveness {
                AgentLiveness::Active => active_count += 1,
                AgentLiveness::Idle => idle_count += 1,
                AgentLiveness::Offline => offline_count += 1,
            }

            // Inspect dimension workspace
            let mut staged_files = Vec::new();
            let mut modified_files = Vec::new();
            let mut untracked_files = Vec::new();
            let mut last_commit_oid = None;
            let mut last_commit_msg = None;

            if let Ok(dim_repo) =
                DimensionRepository::for_dimension(Arc::clone(&self.repo), dim_name)
            {
                let mut head_tree_files = std::collections::BTreeMap::new();
                if let Some(oid) = dim_repo.head_commit() {
                    last_commit_oid = Some(oid.to_hex());
                    if let Ok(raw) = self.repo.cas().read_raw(&oid) {
                        if let Ok(commit) = Commit::deserialize(&raw.data) {
                            let first_line =
                                commit.message.lines().next().unwrap_or("").to_string();
                            last_commit_msg = Some(first_line);
                            let _ = daft_core::diff::tree::flatten_tree(
                                self.repo.cas().as_ref(),
                                &commit.tree,
                                "",
                                &mut head_tree_files,
                            );
                        }
                    }
                }

                // Check index entries
                if let Ok(index) = dim_repo.index() {
                    let mut index_files = std::collections::BTreeMap::new();
                    for entry in index.entries() {
                        if entry.stage() == Stage::Normal {
                            index_files.insert(entry.path.clone(), entry);
                        }
                    }

                    // 1. Staged Changes: Compare HEAD Tree vs Staging Index
                    let mut all_staged_paths = std::collections::BTreeSet::new();
                    for p in head_tree_files.keys() {
                        all_staged_paths.insert(p.clone());
                    }
                    for p in index_files.keys() {
                        all_staged_paths.insert(p.clone());
                    }

                    for p in all_staged_paths {
                        let in_head = head_tree_files.get(&p);
                        let in_index = index_files.get(&p);

                        match (in_head, in_index) {
                            (Some((_, head_oid)), Some(entry)) => {
                                if head_oid != &entry.oid {
                                    staged_files.push(p);
                                }
                            }
                            (None, Some(_)) => {
                                staged_files.push(p);
                            }
                            (Some(_), None) => {
                                staged_files.push(format!("deleted: {}", p));
                            }
                            (None, None) => {}
                        }
                    }

                    // 2. Unstaged Working Tree Changes: Compare Index vs Disk
                    let workdir = dim_repo.workdir();
                    for entry in index.entries() {
                        let fpath = workdir.join(&entry.path);
                        if !fpath.exists() {
                            modified_files.push(format!("deleted: {}", entry.path));
                        } else if let Ok(meta) = fpath.metadata() {
                            if meta.len() != entry.file_size as u64 {
                                modified_files.push(entry.path.clone());
                            }
                        }
                    }

                    // 3. Untracked files
                    if workdir.exists() {
                        for entry in WalkDir::new(workdir)
                            .into_iter()
                            .filter_entry(|e| {
                                let name = e.file_name().to_string_lossy();
                                !name.starts_with(".dft") && !name.starts_with(".git")
                            })
                            .flatten()
                        {
                            if entry.file_type().is_file() {
                                if let Ok(rel) = entry.path().strip_prefix(workdir) {
                                    let rel_str = rel.to_string_lossy().to_string();
                                    if index.find_entry(&rel_str, Stage::Normal).is_none() {
                                        untracked_files.push(rel_str);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let total_dirty = staged_files.len() + modified_files.len() + untracked_files.len();

            activities.push(AgentActivity {
                agent_id: agent.id,
                name: agent.name,
                agent_type: agent.agent_type,
                assigned_dimension: dim_name.to_string(),
                liveness,
                seconds_since_heartbeat: seconds_since,
                last_heartbeat_timestamp: hb.map(|h| h.timestamp),
                last_commit_oid,
                last_commit_message: last_commit_msg,
                staged_files,
                modified_files,
                untracked_files,
                total_dirty_files: total_dirty,
            });
        }

        Ok(SwarmStatusReport {
            timestamp: now,
            total_agents: activities.len(),
            active_agents: active_count,
            idle_agents: idle_count,
            offline_agents: offline_count,
            agents: activities,
        })
    }
}
