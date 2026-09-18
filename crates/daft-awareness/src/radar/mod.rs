use crate::error::AwarenessError;
use chrono::{DateTime, Utc};
use daft_core::diff::tree::flatten_tree;
use daft_core::object::Commit;
use daft_core::Repository;
use daft_dimension::DimensionRepository;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathActivity {
    pub path: PathBuf,
    pub dimensions: Vec<String>,
    pub touch_count: usize,
    pub is_hot: bool,
    pub last_modified: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarReport {
    pub scanned_dimensions: Vec<String>,
    pub total_active_files: usize,
    pub hot_zones: Vec<PathActivity>,
    pub activities: Vec<PathActivity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RadarEvent {
    HotZoneEntered {
        path: PathBuf,
        dimensions: Vec<String>,
        touch_count: usize,
    },
    HotZoneCleared {
        path: PathBuf,
        remaining_dimensions: Vec<String>,
    },
    FileModified {
        dimension: String,
        path: PathBuf,
    },
    FileReverted {
        dimension: String,
        path: PathBuf,
    },
}

pub struct WatchHandle {
    stop_signal: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

impl WatchHandle {
    pub fn stop(mut self) {
        self.stop_signal.store(true, Ordering::SeqCst);
        if let Some(h) = self.thread_handle.take() {
            let _ = h.join();
        }
    }
}

pub struct RadarSubsystem {
    repo: Arc<Repository>,
}

impl RadarSubsystem {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self { repo }
    }

    /// Discovers all available dimensions including mainline.
    pub fn discover_dimensions(&self) -> Vec<String> {
        let mut dims = vec!["mainline".to_string()];
        let dims_dir = self.repo.dft_dir().join("dimensions");
        if let Ok(entries) = fs::read_dir(dims_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name != "mainline" && !dims.contains(&name) {
                        dims.push(name);
                    }
                }
            }
        }
        dims.sort();
        dims
    }

    /// Scans a single dimension for dirty or active files.
    fn scan_dimension(
        repo: &Arc<Repository>,
        dimension: &str,
    ) -> Result<(String, HashMap<String, DateTime<Utc>>), AwarenessError> {
        let dim_repo = DimensionRepository::for_dimension(Arc::clone(repo), dimension)?;
        let mut active_files: HashMap<String, DateTime<Utc>> = HashMap::new();

        // 1. Get HEAD tree files
        let mut head_files = BTreeMap::new();
        if let Some(head_oid) = dim_repo.head_commit() {
            if let Ok(raw) = repo.cas().read_raw(&head_oid) {
                if let Ok(commit) = Commit::deserialize(&raw.data) {
                    let _ = flatten_tree(repo.cas().as_ref(), &commit.tree, "", &mut head_files);
                }
            }
        }

        // 2. Check staged index
        let index = dim_repo.index().unwrap_or_default();
        for entry in index.entries() {
            let is_dirty = match head_files.get(&entry.path) {
                Some((_mode, head_oid)) => entry.oid != *head_oid,
                None => true,
            };
            if is_dirty {
                let mtime =
                    DateTime::<Utc>::from_timestamp(entry.mtime.sec as i64, entry.mtime.nsec)
                        .unwrap_or_else(Utc::now);
                active_files.insert(entry.path.clone(), mtime);
            }
        }

        // 3. Check workspace directory
        let workdir = dim_repo.workdir();
        if workdir.exists() {
            for entry in WalkDir::new(workdir)
                .into_iter()
                .filter_entry(|e| {
                    let name = e.file_name().to_string_lossy();
                    name != ".dft"
                        && name != ".git"
                        && name != "target"
                        && name != ".agents"
                        && name != "node_modules"
                })
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() {
                    if let Ok(rel) = entry.path().strip_prefix(workdir) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        let mtime = entry
                            .metadata()
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .map(DateTime::<Utc>::from)
                            .unwrap_or_else(Utc::now);

                        if let Some(idx_entry) = index.entries().iter().find(|e| e.path == rel_str)
                        {
                            if let Ok(meta) = entry.metadata() {
                                if meta.len() != idx_entry.file_size as u64 {
                                    active_files.insert(rel_str, mtime);
                                } else if let Ok(data) = fs::read(entry.path()) {
                                    let current_oid =
                                        daft_core::cas::RawObject::blob(data).compute_id();
                                    if current_oid != idx_entry.oid {
                                        active_files.insert(rel_str, mtime);
                                    }
                                }
                            }
                        } else {
                            // Untracked
                            active_files.insert(rel_str, mtime);
                        }
                    }
                }
            }
        }

        Ok((dimension.to_string(), active_files))
    }

    /// Executes a parallel concurrency scan across all dimensions.
    pub fn scan(&self, threshold: usize) -> Result<RadarReport, AwarenessError> {
        let dimensions = self.discover_dimensions();

        let results: Vec<(String, HashMap<String, DateTime<Utc>>)> = dimensions
            .par_iter()
            .filter_map(|dim| Self::scan_dimension(&self.repo, dim).ok())
            .collect();

        let mut path_map: BTreeMap<String, (Vec<String>, DateTime<Utc>)> = BTreeMap::new();

        for (dim, active_files) in results {
            for (path, mtime) in active_files {
                let entry = path_map.entry(path).or_insert_with(|| (Vec::new(), mtime));
                entry.0.push(dim.clone());
                if mtime > entry.1 {
                    entry.1 = mtime;
                }
            }
        }

        let mut activities = Vec::new();
        let mut hot_zones = Vec::new();

        for (path_str, (mut dims, last_mod)) in path_map {
            dims.sort();
            dims.dedup();
            let touch_count = dims.len();
            let is_hot = touch_count >= threshold;

            let act = PathActivity {
                path: PathBuf::from(path_str),
                dimensions: dims,
                touch_count,
                is_hot,
                last_modified: last_mod,
            };

            if is_hot {
                hot_zones.push(act.clone());
            }
            activities.push(act);
        }

        hot_zones.sort_by(|a, b| {
            b.touch_count
                .cmp(&a.touch_count)
                .then_with(|| a.path.cmp(&b.path))
        });
        activities.sort_by(|a, b| {
            b.touch_count
                .cmp(&a.touch_count)
                .then_with(|| a.path.cmp(&b.path))
        });

        let total_active_files = activities.len();

        Ok(RadarReport {
            scanned_dimensions: dimensions,
            total_active_files,
            hot_zones,
            activities,
        })
    }

    /// Checks if a specific path is being touched in any dimension.
    pub fn inspect_path(&self, target_path: &Path) -> Result<Option<PathActivity>, AwarenessError> {
        let report = self.scan(1)?;
        let target_str = target_path
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches("./")
            .to_string();

        for act in report.activities {
            let act_str = act.path.to_string_lossy().replace('\\', "/");
            if act_str == target_str || act_str.starts_with(&format!("{}/", target_str)) {
                return Ok(Some(act));
            }
        }

        Ok(None)
    }

    /// Starts a live watch polling loop streaming events via channel.
    pub fn watch(
        &self,
        poll_interval: Duration,
        threshold: usize,
    ) -> Result<(Receiver<RadarEvent>, WatchHandle), AwarenessError> {
        let (tx, rx): (Sender<RadarEvent>, Receiver<RadarEvent>) = channel();
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop_signal);

        let repo_clone = Arc::clone(&self.repo);

        let thread_handle = thread::spawn(move || {
            let radar = RadarSubsystem::new(repo_clone);
            let mut prev_state: HashMap<PathBuf, Vec<String>> = HashMap::new();

            while !stop_clone.load(Ordering::SeqCst) {
                if let Ok(report) = radar.scan(threshold) {
                    let mut current_state: HashMap<PathBuf, Vec<String>> = HashMap::new();

                    for act in &report.activities {
                        current_state.insert(act.path.clone(), act.dimensions.clone());
                    }

                    // Check newly entered hot zones
                    for act in &report.hot_zones {
                        let prev_dims = prev_state.get(&act.path);
                        let was_hot = prev_dims.map(|d| d.len() >= threshold).unwrap_or(false);
                        if !was_hot {
                            let _ = tx.send(RadarEvent::HotZoneEntered {
                                path: act.path.clone(),
                                dimensions: act.dimensions.clone(),
                                touch_count: act.touch_count,
                            });
                        }
                    }

                    // Check cleared hot zones
                    for (prev_path, prev_dims) in &prev_state {
                        if prev_dims.len() >= threshold {
                            let curr_dims = current_state.get(prev_path);
                            let is_still_hot =
                                curr_dims.map(|d| d.len() >= threshold).unwrap_or(false);
                            if !is_still_hot {
                                let _ = tx.send(RadarEvent::HotZoneCleared {
                                    path: prev_path.clone(),
                                    remaining_dimensions: curr_dims.cloned().unwrap_or_default(),
                                });
                            }
                        }
                    }

                    prev_state = current_state;
                }

                thread::sleep(poll_interval);
            }
        });

        Ok((
            rx,
            WatchHandle {
                stop_signal,
                thread_handle: Some(thread_handle),
            },
        ))
    }
}
