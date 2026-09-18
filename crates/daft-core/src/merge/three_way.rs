//! 3-way line-level text merge algorithm with conflict markers.

use crate::diff::myers::{myers_diff, EditOpKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextMergeResult {
    pub content: String,
    pub has_conflicts: bool,
}

#[derive(Debug, Clone)]
struct LineChange {
    /// 0-based index in base where change begins
    base_start: usize,
    /// Number of lines in base replaced (0 for pure insertion)
    base_count: usize,
    /// Replacement lines
    lines: Vec<String>,
}

fn extract_changes(base_text: &str, other_text: &str) -> Vec<LineChange> {
    let ops = myers_diff(base_text, other_text);
    let mut changes = Vec::new();
    let mut i = 0;
    let mut base_pos = 0;

    while i < ops.len() {
        if ops[i].kind == EditOpKind::Equal {
            base_pos += 1;
            i += 1;
            continue;
        }

        let change_start = base_pos;
        let mut base_deleted = 0;
        let mut replacement = Vec::new();

        while i < ops.len() && ops[i].kind != EditOpKind::Equal {
            match ops[i].kind {
                EditOpKind::Delete => {
                    base_deleted += 1;
                    base_pos += 1;
                }
                EditOpKind::Insert => {
                    let mut s = ops[i].text.to_string();
                    if !s.ends_with('\n') {
                        s.push('\n');
                    }
                    replacement.push(s);
                }
                EditOpKind::Equal => unreachable!(),
            }
            i += 1;
        }

        changes.push(LineChange {
            base_start: change_start,
            base_count: base_deleted,
            lines: replacement,
        });
    }

    changes
}

/// Merges `ours` and `theirs` against `base` line by line.
///
/// If conflicting changes are detected on the same base lines,
/// conflict markers (`<<<<<<<`, `=======`, `>>>>>>>`) are emitted.
pub fn merge_text_3way(
    base: &str,
    ours: &str,
    theirs: &str,
    ours_label: &str,
    theirs_label: &str,
) -> TextMergeResult {
    if ours == theirs {
        return TextMergeResult {
            content: ours.to_string(),
            has_conflicts: false,
        };
    }
    if base == ours {
        return TextMergeResult {
            content: theirs.to_string(),
            has_conflicts: false,
        };
    }
    if base == theirs {
        return TextMergeResult {
            content: ours.to_string(),
            has_conflicts: false,
        };
    }

    let base_lines: Vec<String> = base.lines().map(|l| format!("{}\n", l)).collect();

    let changes_a = extract_changes(base, ours);
    let changes_b = extract_changes(base, theirs);

    let mut out = String::new();
    let mut has_conflicts = false;

    let mut a_idx = 0;
    let mut b_idx = 0;
    let mut base_cursor = 0;
    let n_base = base_lines.len();

    while base_cursor < n_base || a_idx < changes_a.len() || b_idx < changes_b.len() {
        let next_a = changes_a.get(a_idx);
        let next_b = changes_b.get(b_idx);

        let next_start = match (next_a, next_b) {
            (Some(a), Some(b)) => a.base_start.min(b.base_start),
            (Some(a), None) => a.base_start,
            (None, Some(b)) => b.base_start,
            (None, None) => n_base,
        };

        // Emit clean island lines between base_cursor and the next change
        if base_cursor < next_start && base_cursor < n_base {
            let end = next_start.min(n_base);
            for line in &base_lines[base_cursor..end] {
                out.push_str(line);
            }
            base_cursor = end;
        }

        if a_idx >= changes_a.len() && b_idx >= changes_b.len() && base_cursor >= n_base {
            break;
        }

        if a_idx >= changes_a.len() && b_idx >= changes_b.len() {
            continue;
        }

        // Determine extent of modification window with iterative expansion
        let window_start = base_cursor;
        let mut window_end = window_start;
        let mut cur_a_changes = Vec::new();
        let mut cur_b_changes = Vec::new();

        loop {
            let mut expanded = false;
            if let Some(ca) = changes_a.get(a_idx) {
                if ca.base_start < window_end
                    || (window_start == window_end && ca.base_start == window_start)
                {
                    window_end = window_end.max(ca.base_start + ca.base_count);
                    cur_a_changes.push(ca);
                    a_idx += 1;
                    expanded = true;
                }
            }
            if let Some(cb) = changes_b.get(b_idx) {
                if cb.base_start < window_end
                    || (window_start == window_end && cb.base_start == window_start)
                {
                    window_end = window_end.max(cb.base_start + cb.base_count);
                    cur_b_changes.push(cb);
                    b_idx += 1;
                    expanded = true;
                }
            }
            if !expanded {
                break;
            }
        }

        let a_active = !cur_a_changes.is_empty();
        let b_active = !cur_b_changes.is_empty();

        // Reconstruct ours version in window
        let mut a_lines = Vec::new();
        let mut pos_a = window_start;
        for ca in cur_a_changes {
            while pos_a < ca.base_start && pos_a < n_base {
                a_lines.push(base_lines[pos_a].clone());
                pos_a += 1;
            }
            a_lines.extend(ca.lines.clone());
            pos_a = ca.base_start + ca.base_count;
        }
        while pos_a < window_end && pos_a < n_base {
            a_lines.push(base_lines[pos_a].clone());
            pos_a += 1;
        }

        // Reconstruct theirs version in window
        let mut b_lines = Vec::new();
        let mut pos_b = window_start;
        for cb in cur_b_changes {
            while pos_b < cb.base_start && pos_b < n_base {
                b_lines.push(base_lines[pos_b].clone());
                pos_b += 1;
            }
            b_lines.extend(cb.lines.clone());
            pos_b = cb.base_start + cb.base_count;
        }
        while pos_b < window_end && pos_b < n_base {
            b_lines.push(base_lines[pos_b].clone());
            pos_b += 1;
        }

        if a_active && !b_active {
            // Clean edit in ours
            for l in &a_lines {
                out.push_str(l);
            }
        } else if !a_active && b_active {
            // Clean edit in theirs
            for l in &b_lines {
                out.push_str(l);
            }
        } else {
            // Both active in this window
            if a_lines == b_lines {
                // Identical modification: clean!
                for l in &a_lines {
                    out.push_str(l);
                }
            } else {
                // Conflict!
                has_conflicts = true;
                out.push_str(&format!("<<<<<<< {}\n", ours_label));
                for l in &a_lines {
                    out.push_str(l);
                }
                out.push_str("=======\n");
                for l in &b_lines {
                    out.push_str(l);
                }
                out.push_str(&format!(">>>>>>> {}\n", theirs_label));
            }
        }
        base_cursor = window_end.max(base_cursor);
    }

    TextMergeResult {
        content: out,
        has_conflicts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_clean_disjoint() {
        let base = "line 1\nline 2\nline 3\n";
        let ours = "line 1 modified\nline 2\nline 3\n";
        let theirs = "line 1\nline 2\nline 3 modified\n";

        let res = merge_text_3way(base, ours, theirs, "HEAD", "theirs");
        assert!(!res.has_conflicts);
        assert!(res.content.contains("line 1 modified"));
        assert!(res.content.contains("line 3 modified"));
    }

    #[test]
    fn test_merge_conflict() {
        let base = "original line\n";
        let ours = "left modification\n";
        let theirs = "right modification\n";

        let res = merge_text_3way(base, ours, theirs, "HEAD", "branch-left");
        assert!(res.has_conflicts);
        assert!(res.content.contains("<<<<<<< HEAD"));
        assert!(res.content.contains("left modification"));
        assert!(res.content.contains("======="));
        assert!(res.content.contains("right modification"));
        assert!(res.content.contains(">>>>>>> branch-left"));
    }
}
