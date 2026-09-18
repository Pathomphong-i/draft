use crate::error::AwarenessError;
use crate::foresee::ForeseeSubsystem;
use crate::observe::ObserveSubsystem;
use daft_core::diff::myers::{myers_diff, EditOpKind};
use daft_core::diff::tree::flatten_tree;
use daft_core::graph::{walk_commits, StoreCommitGraph};
use daft_core::object::Commit;
use daft_core::Repository;
use daft_dimension::DimensionRepository;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntropyMetrics {
    pub hunk_collision_score: f64,
    pub file_overlap_score: f64,
    pub topological_drift_score: f64,
    pub line_churn_score: f64,
    pub total_entropy: f64,
    pub topological_commit_distance: usize,
    pub shared_modified_files: Vec<PathBuf>,
    pub disjoint_modified_files: Vec<PathBuf>,
    pub total_lines_churned: usize,
    pub risk_tier: String,
    pub recommended_action: String,
}

pub struct EntropySubsystem {
    repo: Arc<Repository>,
}

impl EntropySubsystem {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self { repo }
    }

    /// Computes quantitative mathematical divergence entropy H(D1, D2).
    pub fn calculate(&self, dim1: &str, dim2: &str) -> Result<EntropyMetrics, AwarenessError> {
        let observe = ObserveSubsystem::new(Arc::clone(&self.repo));
        let foresee = ForeseeSubsystem::new(Arc::clone(&self.repo));

        let report = foresee.predict(dim1, dim2)?;

        let dim1_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), dim1)?;
        let dim2_repo = DimensionRepository::for_dimension(Arc::clone(&self.repo), dim2)?;

        let h1 = dim1_repo.head_commit();
        let h2 = dim2_repo.head_commit();

        // 1. Files in base
        let mut base_files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        if let Some(base_oid) = report.lca_commit {
            if let Ok(raw) = self.repo.cas().read_raw(&base_oid) {
                if let Ok(commit) = Commit::deserialize(&raw.data) {
                    let mut tree_entries = BTreeMap::new();
                    let _ = flatten_tree(
                        self.repo.cas().as_ref(),
                        &commit.tree,
                        "",
                        &mut tree_entries,
                    );
                    for (p, (_m, oid)) in tree_entries {
                        if let Ok(blob) = self.repo.cas().read_raw(&oid) {
                            base_files.insert(p, blob.data);
                        }
                    }
                }
            }
        }

        // 2. Virtual files
        let files1 = observe.get_dimension_virtual_files(dim1)?;
        let files2 = observe.get_dimension_virtual_files(dim2)?;

        // Find modified paths in D1 relative to base
        let mut m1: HashSet<String> = HashSet::new();
        for (p, data1) in &files1 {
            if base_files.get(p).map(|v| v.as_slice()) != Some(data1.as_slice()) {
                m1.insert(p.clone());
            }
        }
        for p in base_files.keys() {
            if !files1.contains_key(p) {
                m1.insert(p.clone());
            }
        }

        // Find modified paths in D2 relative to base
        let mut m2: HashSet<String> = HashSet::new();
        for (p, data2) in &files2 {
            if base_files.get(p).map(|v| v.as_slice()) != Some(data2.as_slice()) {
                m2.insert(p.clone());
            }
        }
        for p in base_files.keys() {
            if !files2.contains_key(p) {
                m2.insert(p.clone());
            }
        }

        // Sets: intersection and union
        let m_intersect: HashSet<String> = m1.intersection(&m2).cloned().collect();
        let m_union: HashSet<String> = m1.union(&m2).cloned().collect();

        let mut shared_modified_files: Vec<PathBuf> =
            m_intersect.iter().map(PathBuf::from).collect();
        shared_modified_files.sort();

        let mut disjoint_modified_files: Vec<PathBuf> =
            m1.symmetric_difference(&m2).map(PathBuf::from).collect();
        disjoint_modified_files.sort();

        // Sub-metric 1: C (Hunk collision score)
        let mut total_conflict_lines = 0;
        for hunk in &report.conflict_hunks {
            total_conflict_lines += hunk.ours_lines.len() + hunk.theirs_lines.len();
        }

        // Compute churn across modified files
        let mut total_lines_churned = 0;
        for p in &m_union {
            let base_text = base_files
                .get(p)
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_default();
            if let Some(b1) = files1.get(p) {
                let t1 = String::from_utf8_lossy(b1);
                for op in myers_diff(&base_text, &t1) {
                    if op.kind != EditOpKind::Equal {
                        total_lines_churned += 1;
                    }
                }
            }
            if let Some(b2) = files2.get(p) {
                let t2 = String::from_utf8_lossy(b2);
                for op in myers_diff(&base_text, &t2) {
                    if op.kind != EditOpKind::Equal {
                        total_lines_churned += 1;
                    }
                }
            }
        }

        let c_score = if report.has_conflicts {
            let gamma = 0.60;
            let ratio = (total_conflict_lines as f64) / (total_lines_churned.max(1) as f64);
            (gamma + (1.0 - gamma) * ratio).min(1.0)
        } else {
            0.0
        };

        // Sub-metric 2: F (File overlap Jaccard score)
        let f_score = if m_union.is_empty() {
            0.0
        } else {
            (m_intersect.len() as f64) / (m_union.len() as f64)
        };

        // Sub-metric 3: T (Topological commit distance)
        let graph = StoreCommitGraph::new(self.repo.cas().as_ref());
        let d1 = if let (Some(head), Some(base)) = (h1, report.lca_commit) {
            Self::commit_distance(&graph, &base, &head)
        } else {
            0
        };
        let d2 = if let (Some(head), Some(base)) = (h2, report.lca_commit) {
            Self::commit_distance(&graph, &base, &head)
        } else {
            0
        };
        let d_topo = d1 + d2;
        let t_score = 1.0 - (-0.08 * (d_topo as f64)).exp();

        // Sub-metric 4: L (Line churn)
        let l_score = ((total_lines_churned as f64) / 500.0).tanh();

        // Weighted sum: 0.50 C + 0.25 F + 0.15 T + 0.10 L
        let mut total_entropy = 0.50 * c_score + 0.25 * f_score + 0.15 * t_score + 0.10 * l_score;

        // Identity check: identical dimensions
        let is_identical = h1 == h2 && files1 == files2;
        if is_identical {
            total_entropy = 0.0;
        } else if report.has_conflicts {
            // Collision monotonicity
            total_entropy = total_entropy.max(0.65);
        }

        // Clamp to [0.0, 1.0]
        total_entropy = total_entropy.clamp(0.0, 1.0);

        let (risk_tier, recommended_action) = if total_entropy == 0.0 {
            (
                "Clean".to_string(),
                "Fast-forward or no-op merge".to_string(),
            )
        } else if total_entropy <= 0.15 {
            ("Negligible".to_string(), "Automatic fast-merge".to_string())
        } else if total_entropy <= 0.35 {
            (
                "Low".to_string(),
                "Automated merge with standard tests".to_string(),
            )
        } else if total_entropy <= 0.64 {
            (
                "Moderate".to_string(),
                "Automated merge with review recommended".to_string(),
            )
        } else if total_entropy <= 0.85 {
            (
                "High".to_string(),
                "Interactive conflict resolution required".to_string(),
            )
        } else {
            (
                "Critical".to_string(),
                "Hard block; structural reconciliation required".to_string(),
            )
        };

        Ok(EntropyMetrics {
            hunk_collision_score: c_score,
            file_overlap_score: f_score,
            topological_drift_score: t_score,
            line_churn_score: l_score,
            total_entropy,
            topological_commit_distance: d_topo,
            shared_modified_files,
            disjoint_modified_files,
            total_lines_churned,
            risk_tier,
            recommended_action,
        })
    }

    /// Computes shortest topological commit distance from base to head.
    fn commit_distance(
        graph: &StoreCommitGraph,
        base: &daft_core::cas::ObjectId,
        head: &daft_core::cas::ObjectId,
    ) -> usize {
        if base == head {
            return 0;
        }
        if let Ok(commits) = walk_commits(graph, &[*head]) {
            if let Some(pos) = commits.iter().position(|oid| oid == base) {
                return pos;
            }
            return commits.len();
        }
        0
    }
}
