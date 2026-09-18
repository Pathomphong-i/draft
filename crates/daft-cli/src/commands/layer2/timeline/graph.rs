use chrono::{DateTime, Utc};
use daft_core::cas::ObjectId;
use daft_core::graph::{merge_base, walk_commits, StoreCommitGraph};
use daft_core::Repository;
use daft_dimension::{DimensionManager, DimensionRepository};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineNodeType {
    Commit,
    Root,
    ForkPoint,
    Merge,
    Convergence,
    DirtyWorkspace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineNode {
    pub id: String,
    pub short_id: String,
    pub node_type: TimelineNodeType,
    pub author: String,
    pub timestamp: i64,
    pub datetime: String,
    pub summary: String,
    pub parent_ids: Vec<String>,
    pub child_ids: Vec<String>,
    pub reachable_dimensions: Vec<String>,
    pub head_of_dimensions: Vec<String>,
    pub forked_dimensions: Vec<String>,
    pub lane: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineDimension {
    pub name: String,
    pub parent: Option<String>,
    pub creator: String,
    pub created_at: String,
    pub fork_base_commit: Option<String>,
    pub head_commit: Option<String>,
    pub status: String,
    pub is_active: bool,
    pub lane: usize,
    pub dirty_files_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineEdgeType {
    CommitParent,
    DimensionFork,
    MergeInput,
    LiveWorkspace,
    EntangleLink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEdge {
    pub source: String,
    pub target: String,
    pub edge_type: TimelineEdgeType,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiverseTimelineGraph {
    pub version: String,
    pub repository: String,
    pub current_dimension: String,
    pub dimensions: Vec<TimelineDimension>,
    pub nodes: Vec<TimelineNode>,
    pub edges: Vec<TimelineEdge>,
}

pub struct TimelineGraphBuilder;

impl TimelineGraphBuilder {
    pub fn build(
        repo: &Arc<Repository>,
        filter_dim: Option<&str>,
    ) -> Result<MultiverseTimelineGraph, crate::error::CliError> {
        let dim_mgr = DimensionManager::new(Arc::clone(repo));
        let active_dim = dim_mgr.current_dimension_name();

        // 1. Enumerate dimensions
        let raw_dims = dim_mgr.list_dimensions()?;
        let mut dimensions: Vec<TimelineDimension> = Vec::new();
        let mut lane_counter = 0;

        // Ensure mainline is always lane 0
        let mainline_meta = dim_mgr.read_dimension_metadata("mainline").ok();
        let mainline_head = DimensionRepository::for_dimension(Arc::clone(repo), "mainline")
            .ok()
            .and_then(|r| r.head_commit())
            .or_else(|| mainline_meta.as_ref().and_then(|m| m.head_commit));

        dimensions.push(TimelineDimension {
            name: "mainline".to_string(),
            parent: None,
            creator: "system".to_string(),
            created_at: mainline_meta
                .as_ref()
                .map(|m| m.created_at.clone())
                .unwrap_or_default(),
            fork_base_commit: None,
            head_commit: mainline_head.map(|o| o.to_hex()),
            status: "clean".to_string(),
            is_active: active_dim == "mainline",
            lane: lane_counter,
            dirty_files_count: 0,
        });
        lane_counter += 1;

        for d in raw_dims {
            if d.name == "mainline" {
                continue;
            }
            if let Some(fd) = filter_dim {
                if d.name != fd {
                    continue;
                }
            }

            let meta = dim_mgr.read_dimension_metadata(&d.name).ok();
            let dim_repo = DimensionRepository::for_dimension(Arc::clone(repo), &d.name).ok();
            let head = dim_repo
                .as_ref()
                .and_then(|r| r.head_commit())
                .or_else(|| meta.as_ref().and_then(|m| m.head_commit));

            let parent = meta.as_ref().and_then(|m| m.parent.clone()).or(d.parent);
            let is_active = active_dim == d.name;

            dimensions.push(TimelineDimension {
                name: d.name,
                parent,
                creator: meta
                    .as_ref()
                    .map(|m| m.creator.clone())
                    .unwrap_or_else(|| "user".to_string()),
                created_at: meta
                    .as_ref()
                    .map(|m| m.created_at.clone())
                    .unwrap_or_default(),
                fork_base_commit: None,
                head_commit: head.map(|o| o.to_hex()),
                status: d.status,
                is_active,
                lane: lane_counter,
                dirty_files_count: 0,
            });
            lane_counter += 1;
        }

        // 2. Compute fork base commits via LCA (merge_base)
        let store_graph = StoreCommitGraph::new(repo.cas());

        for dim in &mut dimensions {
            if let Some(ref parent_name) = dim.parent {
                let parent_head = dim_mgr
                    .read_dimension_metadata(parent_name)
                    .ok()
                    .and_then(|m| m.head_commit);

                let this_head = dim
                    .head_commit
                    .as_ref()
                    .and_then(|h| ObjectId::from_hex(h).ok());

                if let (Some(ph), Some(th)) = (parent_head, this_head) {
                    if let Ok(Some(lca)) = merge_base(&store_graph, &ph, &th) {
                        dim.fork_base_commit = Some(lca.to_hex());
                    }
                }
            }
        }

        // 3. Walk DAG commits
        let mut head_oids = Vec::new();
        let mut head_map: HashMap<ObjectId, Vec<String>> = HashMap::new();

        for d in &dimensions {
            if let Some(ref h_hex) = d.head_commit {
                if let Ok(oid) = ObjectId::from_hex(h_hex) {
                    if !head_oids.contains(&oid) {
                        head_oids.push(oid);
                    }
                    head_map.entry(oid).or_default().push(d.name.clone());
                }
            }
        }

        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        if !head_oids.is_empty() {
            if let Ok(ordered_commits) = walk_commits(&store_graph, &head_oids) {
                let mut child_map: HashMap<ObjectId, Vec<ObjectId>> = HashMap::new();
                let mut commit_objects: HashMap<ObjectId, Arc<daft_core::object::Commit>> =
                    HashMap::new();

                for oid in &ordered_commits {
                    if let Ok(raw) = repo.cas().read_raw(oid) {
                        if let Ok(c) = daft_core::object::Commit::deserialize(&raw.data) {
                            let arc_c = Arc::new(c);
                            for p in &arc_c.parents {
                                child_map.entry(*p).or_default().push(*oid);
                            }
                            commit_objects.insert(*oid, arc_c);
                        }
                    }
                }

                for oid in &ordered_commits {
                    if let Some(commit) = commit_objects.get(oid) {
                        let id_hex = oid.to_hex();
                        let short_id = if id_hex.len() >= 7 {
                            id_hex[..7].to_string()
                        } else {
                            id_hex.clone()
                        };

                        let heads_here = head_map.get(oid).cloned().unwrap_or_default();
                        let forked_here: Vec<String> = dimensions
                            .iter()
                            .filter(|d| d.fork_base_commit.as_deref() == Some(&id_hex))
                            .map(|d| d.name.clone())
                            .collect();

                        let node_type = if commit.parents.is_empty() {
                            TimelineNodeType::Root
                        } else if !forked_here.is_empty() {
                            TimelineNodeType::ForkPoint
                        } else if commit.parents.len() > 1 {
                            TimelineNodeType::Merge
                        } else if commit.message.contains("[converge]")
                            || commit.message.contains("[collapse]")
                            || commit.message.contains("[weave]")
                            || commit.message.contains("[splice]")
                            || commit.message.contains("[cascade]")
                        {
                            TimelineNodeType::Convergence
                        } else {
                            TimelineNodeType::Commit
                        };

                        // Determine lane
                        let lane = if let Some(first_dim) = heads_here.first() {
                            dimensions
                                .iter()
                                .find(|d| &d.name == first_dim)
                                .map(|d| d.lane)
                                .unwrap_or(0)
                        } else {
                            0
                        };

                        let parent_hexes: Vec<String> =
                            commit.parents.iter().map(|p| p.to_hex()).collect();
                        let child_hexes: Vec<String> = child_map
                            .get(oid)
                            .map(|children| children.iter().map(|c| c.to_hex()).collect())
                            .unwrap_or_default();

                        // Add edges
                        for p in &parent_hexes {
                            edges.push(TimelineEdge {
                                source: id_hex.clone(),
                                target: p.clone(),
                                edge_type: TimelineEdgeType::CommitParent,
                                label: None,
                            });
                        }

                        let dt = DateTime::<Utc>::from_timestamp(commit.author.time, 0)
                            .unwrap_or_else(Utc::now)
                            .to_rfc3339();

                        let summary = commit
                            .message
                            .lines()
                            .next()
                            .unwrap_or("")
                            .trim()
                            .to_string();

                        nodes.push(TimelineNode {
                            id: id_hex,
                            short_id,
                            node_type,
                            author: commit.author.name.clone(),
                            timestamp: commit.author.time,
                            datetime: dt,
                            summary,
                            parent_ids: parent_hexes,
                            child_ids: child_hexes,
                            reachable_dimensions: heads_here.clone(),
                            head_of_dimensions: heads_here,
                            forked_dimensions: forked_here,
                            lane,
                        });
                    }
                }
            }
        }

        // 4. Connect entanglements if present
        let entangle_file = repo.dft_dir().join("entangle/rules.json");
        if entangle_file.exists() {
            if let Ok(content) = std::fs::read_to_string(entangle_file) {
                #[derive(Deserialize)]
                struct RuleStub {
                    dim1: String,
                    dim2: String,
                }
                if let Ok(rules) = serde_json::from_str::<Vec<RuleStub>>(&content) {
                    for r in rules {
                        edges.push(TimelineEdge {
                            source: r.dim1.clone(),
                            target: r.dim2.clone(),
                            edge_type: TimelineEdgeType::EntangleLink,
                            label: Some("entangled".to_string()),
                        });
                    }
                }
            }
        }

        Ok(MultiverseTimelineGraph {
            version: "1.0".to_string(),
            repository: repo
                .workdir()
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "daft-repo".to_string()),
            current_dimension: active_dim,
            dimensions,
            nodes,
            edges,
        })
    }
}
