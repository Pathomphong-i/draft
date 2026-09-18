use crate::error::AwarenessError;
use crate::observe::ObserveSubsystem;
use daft_core::cas::ObjectId;
use daft_core::diff::binary::is_binary;
use daft_core::diff::myers::{myers_diff, EditOpKind};
use daft_core::diff::tree::flatten_tree;
use daft_core::graph::{all_merge_bases, StoreCommitGraph};
use daft_core::object::Commit;
use daft_core::Repository;
use daft_dimension::DimensionRepository;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictType {
    TextOverlap,
    AddAddConflict,
    ModifyDeleteConflict,
    DeleteModifyConflict,
    BinaryCollision,
    ProximityWarning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictHunk {
    pub path: PathBuf,
    pub conflict_type: ConflictType,
    pub base_range: Range<usize>,
    pub ours_range: Range<usize>,
    pub theirs_range: Range<usize>,
    pub ours_lines: Vec<String>,
    pub theirs_lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeseeReport {
    pub dim1: String,
    pub dim2: String,
    pub lca_commit: Option<ObjectId>,
    pub has_conflicts: bool,
    pub conflict_count: usize,
    pub clean_file_count: usize,
    pub conflicting_files: Vec<PathBuf>,
    pub conflict_hunks: Vec<ConflictHunk>,
    pub severity_score: f64,
}

#[derive(Debug, Clone)]
struct DiffHunk {
    base_start: usize,
    base_end: usize,
    new_start: usize,
    new_end: usize,
    inserted_lines: Vec<String>,
}

#[derive(Debug, Default)]
struct ActiveHunk {
    anchor_base: usize,
    anchor_new: usize,
    deleted_base_start: Option<usize>,
    deleted_base_end: Option<usize>,
    inserted_new_start: Option<usize>,
    inserted_new_end: Option<usize>,
    inserted_lines: Vec<String>,
}

impl ActiveHunk {
    fn into_diff_hunk(self) -> DiffHunk {
        let (base_start, base_end) = match (self.deleted_base_start, self.deleted_base_end) {
            (Some(s), Some(e)) => (s, e),
            _ => (self.anchor_base, self.anchor_base),
        };
        let (new_start, new_end) = match (self.inserted_new_start, self.inserted_new_end) {
            (Some(s), Some(e)) => (s, e),
            _ => (self.anchor_new, self.anchor_new),
        };
        DiffHunk {
            base_start,
            base_end,
            new_start,
            new_end,
            inserted_lines: self.inserted_lines,
        }
    }
}

pub struct ForeseeSubsystem {
    repo: Arc<Repository>,
}

impl ForeseeSubsystem {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self { repo }
    }

    /// Predicts 3-way merge conflicts between two dimensions with in-flight dirty projection.
    pub fn predict(&self, dim1: &str, dim2: &str) -> Result<ForeseeReport, AwarenessError> {
        let observe = ObserveSubsystem::new(Arc::clone(&self.repo));

        let dim1_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), dim1)?;
        let dim2_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), dim2)?;

        let h1 = dim1_repo.head_commit();
        let h2 = dim2_repo.head_commit();

        let lca = match (h1, h2) {
            (Some(c1), Some(c2)) => {
                let graph = StoreCommitGraph::new(self.repo.cas().as_ref());
                all_merge_bases(&graph, &c1, &c2)?.into_iter().next()
            }
            _ => None,
        };

        // 1. Base files from LCA
        let mut base_files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        if let Some(base_oid) = lca {
            if let Ok(raw) = self.repo.cas().read_raw(&base_oid) {
                if let Ok(commit) = Commit::deserialize(&raw.data) {
                    let mut base_tree = BTreeMap::new();
                    let _ =
                        flatten_tree(self.repo.cas().as_ref(), &commit.tree, "", &mut base_tree);
                    for (path, (_mode, oid)) in base_tree {
                        if let Ok(blob) = self.repo.cas().read_raw(&oid) {
                            base_files.insert(path, blob.data);
                        }
                    }
                }
            }
        }

        // 2. Virtual projections
        let files1 = observe.get_dimension_virtual_files(dim1)?;
        let files2 = observe.get_dimension_virtual_files(dim2)?;

        let mut all_paths = BTreeSet::new();
        all_paths.extend(base_files.keys().cloned());
        all_paths.extend(files1.keys().cloned());
        all_paths.extend(files2.keys().cloned());

        let mut clean_file_count = 0;
        let mut conflicting_files = Vec::new();
        let mut conflict_hunks = Vec::new();

        for path in all_paths {
            let p_buf = PathBuf::from(&path);
            let base_bytes = base_files.get(&path).map(|v| v.as_slice());
            let d1_bytes = files1.get(&path).map(|v| v.as_slice());
            let d2_bytes = files2.get(&path).map(|v| v.as_slice());

            // Case 1: D1 and D2 have identical content
            if d1_bytes == d2_bytes {
                clean_file_count += 1;
                continue;
            }

            // Case 2: Only D1 changed relative to base
            if d2_bytes == base_bytes {
                clean_file_count += 1;
                continue;
            }

            // Case 3: Only D2 changed relative to base
            if d1_bytes == base_bytes {
                clean_file_count += 1;
                continue;
            }

            // Case 4: Both modified differently
            match (base_bytes, d1_bytes, d2_bytes) {
                (None, Some(b1), Some(b2)) => {
                    // Add/Add conflict
                    conflicting_files.push(p_buf.clone());
                    conflict_hunks.push(ConflictHunk {
                        path: p_buf,
                        conflict_type: ConflictType::AddAddConflict,
                        base_range: 0..0,
                        ours_range: 0..b1.len(),
                        theirs_range: 0..b2.len(),
                        ours_lines: String::from_utf8_lossy(b1)
                            .lines()
                            .map(String::from)
                            .collect(),
                        theirs_lines: String::from_utf8_lossy(b2)
                            .lines()
                            .map(String::from)
                            .collect(),
                    });
                }
                (Some(_), None, Some(b2)) => {
                    // D1 deleted, D2 modified
                    conflicting_files.push(p_buf.clone());
                    conflict_hunks.push(ConflictHunk {
                        path: p_buf,
                        conflict_type: ConflictType::DeleteModifyConflict,
                        base_range: 0..0,
                        ours_range: 0..0,
                        theirs_range: 0..b2.len(),
                        ours_lines: Vec::new(),
                        theirs_lines: String::from_utf8_lossy(b2)
                            .lines()
                            .map(String::from)
                            .collect(),
                    });
                }
                (Some(_), Some(b1), None) => {
                    // D1 modified, D2 deleted
                    conflicting_files.push(p_buf.clone());
                    conflict_hunks.push(ConflictHunk {
                        path: p_buf,
                        conflict_type: ConflictType::ModifyDeleteConflict,
                        base_range: 0..0,
                        ours_range: 0..b1.len(),
                        theirs_range: 0..0,
                        ours_lines: String::from_utf8_lossy(b1)
                            .lines()
                            .map(String::from)
                            .collect(),
                        theirs_lines: Vec::new(),
                    });
                }
                (Some(base), Some(b1), Some(b2)) => {
                    // Both modified
                    if is_binary(base) || is_binary(b1) || is_binary(b2) {
                        conflicting_files.push(p_buf.clone());
                        conflict_hunks.push(ConflictHunk {
                            path: p_buf,
                            conflict_type: ConflictType::BinaryCollision,
                            base_range: 0..0,
                            ours_range: 0..0,
                            theirs_range: 0..0,
                            ours_lines: vec!["<binary>".to_string()],
                            theirs_lines: vec!["<binary>".to_string()],
                        });
                    } else {
                        let text_base = String::from_utf8_lossy(base);
                        let text_1 = String::from_utf8_lossy(b1);
                        let text_2 = String::from_utf8_lossy(b2);

                        let hunks1 = Self::extract_diff_hunks(&text_base, &text_1);
                        let hunks2 = Self::extract_diff_hunks(&text_base, &text_2);

                        let file_conflicts =
                            Self::detect_overlapping_hunks(&p_buf, &hunks1, &hunks2);

                        if file_conflicts.is_empty() {
                            clean_file_count += 1;
                        } else {
                            conflicting_files.push(p_buf);
                            conflict_hunks.extend(file_conflicts);
                        }
                    }
                }
                (None, None, _) | (None, _, None) | (Some(_), None, None) => {
                    clean_file_count += 1;
                }
            }
        }

        conflicting_files.sort();
        conflicting_files.dedup();

        let conflict_count = conflict_hunks.len();
        let has_conflicts = conflict_count > 0;
        let severity_score = if has_conflicts {
            (0.60 + 0.10 * (conflict_count as f64)).min(1.0)
        } else {
            0.0
        };

        Ok(ForeseeReport {
            dim1: dim1.to_string(),
            dim2: dim2.to_string(),
            lca_commit: lca,
            has_conflicts,
            conflict_count,
            clean_file_count,
            conflicting_files,
            conflict_hunks,
            severity_score,
        })
    }

    /// Extracts modified hunks from Myers diff operations against base text.
    fn extract_diff_hunks(base_text: &str, new_text: &str) -> Vec<DiffHunk> {
        let ops = myers_diff(base_text, new_text);
        let mut hunks = Vec::new();

        let mut last_base_line: usize = 0;
        let mut last_new_line: usize = 0;
        let mut current_hunk: Option<ActiveHunk> = None;

        for op in ops {
            match op.kind {
                EditOpKind::Equal => {
                    if let Some(h) = current_hunk.take() {
                        hunks.push(h.into_diff_hunk());
                    }
                    if let Some(old_l) = op.old_line {
                        last_base_line = old_l;
                    }
                    if let Some(new_l) = op.new_line {
                        last_new_line = new_l;
                    }
                }
                EditOpKind::Delete => {
                    let hunk = current_hunk.get_or_insert_with(|| ActiveHunk {
                        anchor_base: last_base_line,
                        anchor_new: last_new_line,
                        ..Default::default()
                    });

                    let old_l = op.old_line.unwrap_or(last_base_line + 1);
                    last_base_line = old_l;

                    hunk.deleted_base_start =
                        Some(hunk.deleted_base_start.map_or(old_l, |s| s.min(old_l)));
                    hunk.deleted_base_end =
                        Some(hunk.deleted_base_end.map_or(old_l, |e| e.max(old_l)));
                }
                EditOpKind::Insert => {
                    let hunk = current_hunk.get_or_insert_with(|| ActiveHunk {
                        anchor_base: last_base_line,
                        anchor_new: last_new_line,
                        ..Default::default()
                    });

                    let new_l = op.new_line.unwrap_or(last_new_line + 1);
                    last_new_line = new_l;
                    let line_str = op.text.trim_end_matches('\n').to_string();

                    hunk.inserted_new_start =
                        Some(hunk.inserted_new_start.map_or(new_l, |s| s.min(new_l)));
                    hunk.inserted_new_end =
                        Some(hunk.inserted_new_end.map_or(new_l, |e| e.max(new_l)));
                    hunk.inserted_lines.push(line_str);
                }
            }
        }

        if let Some(h) = current_hunk {
            hunks.push(h.into_diff_hunk());
        }

        hunks
    }

    /// Checks if two sets of diff hunks intersect on base line ranges.
    fn detect_overlapping_hunks(
        path: &PathBuf,
        hunks1: &[DiffHunk],
        hunks2: &[DiffHunk],
    ) -> Vec<ConflictHunk> {
        let mut conflicts = Vec::new();

        for h1 in hunks1 {
            for h2 in hunks2 {
                // Interval overlap: max(start1, start2) <= min(end1, end2)
                let overlap_start = h1.base_start.max(h2.base_start);
                let overlap_end = h1.base_end.min(h2.base_end);

                let is_overlapping = if h1.base_start == h1.base_end && h2.base_start == h2.base_end
                {
                    h1.base_start == h2.base_start
                } else {
                    overlap_start <= overlap_end
                };

                if is_overlapping {
                    // Check if replacement lines are identical
                    if h1.inserted_lines != h2.inserted_lines {
                        conflicts.push(ConflictHunk {
                            path: path.clone(),
                            conflict_type: ConflictType::TextOverlap,
                            base_range: overlap_start..overlap_end.max(overlap_start) + 1,
                            ours_range: h1.new_start..h1.new_end + 1,
                            theirs_range: h2.new_start..h2.new_end + 1,
                            ours_lines: h1.inserted_lines.clone(),
                            theirs_lines: h2.inserted_lines.clone(),
                        });
                    }
                }
            }
        }

        conflicts
    }
}
