//! Unified diff formatting and hunk generation.

use super::myers::{myers_diff, EditOp, EditOpKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub lines: Vec<HunkLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HunkLine {
    pub kind: EditOpKind,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct FilePatch {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub is_binary: bool,
    pub hunks: Vec<Hunk>,
}

impl FilePatch {
    pub fn is_empty(&self) -> bool {
        !self.is_binary && self.hunks.is_empty()
    }
}

/// Aggregates raw Myers edit operations into unified diff hunks with context.
pub fn create_hunks(ops: &[EditOp], context: usize) -> Vec<Hunk> {
    let mut edit_indices = Vec::new();
    for (i, op) in ops.iter().enumerate() {
        if op.kind != EditOpKind::Equal {
            edit_indices.push(i);
        }
    }

    if edit_indices.is_empty() {
        return Vec::new();
    }

    // Group edits separated by <= 2 * context
    let mut groups: Vec<(usize, usize)> = Vec::new();
    let mut current_start = edit_indices[0].saturating_sub(context);
    let mut current_end = (edit_indices[0] + context + 1).min(ops.len());

    for &idx in &edit_indices[1..] {
        let next_start = idx.saturating_sub(context);
        let next_end = (idx + context + 1).min(ops.len());

        if next_start <= current_end {
            current_end = next_end;
        } else {
            groups.push((current_start, current_end));
            current_start = next_start;
            current_end = next_end;
        }
    }
    groups.push((current_start, current_end));

    let mut hunks = Vec::new();
    for (start, end) in groups {
        let slice = &ops[start..end];
        let mut old_start = 0;
        let mut new_start = 0;
        let mut old_count = 0;
        let mut new_count = 0;
        let mut lines = Vec::new();

        for op in slice {
            if old_start == 0 {
                if let Some(line) = op.old_line {
                    old_start = line;
                }
            }
            if new_start == 0 {
                if let Some(line) = op.new_line {
                    new_start = line;
                }
            }

            match op.kind {
                EditOpKind::Equal => {
                    old_count += 1;
                    new_count += 1;
                }
                EditOpKind::Delete => {
                    old_count += 1;
                }
                EditOpKind::Insert => {
                    new_count += 1;
                }
            }

            let mut line_str = op.text.to_string();
            if !line_str.ends_with('\n') {
                line_str.push('\n');
            }
            lines.push(HunkLine {
                kind: op.kind,
                content: line_str,
            });
        }

        if old_start == 0 {
            old_start = 1;
        }
        if new_start == 0 {
            new_start = 1;
        }

        hunks.push(Hunk {
            old_start,
            old_count,
            new_start,
            new_count,
            lines,
        });
    }

    hunks
}

/// Generates a patch for a single text or binary file.
pub fn generate_file_patch(
    old_path: Option<&str>,
    new_path: Option<&str>,
    old_content: Option<&[u8]>,
    new_content: Option<&[u8]>,
    context: usize,
) -> FilePatch {
    use super::binary::is_binary;

    let old_bytes = old_content.unwrap_or(&[]);
    let new_bytes = new_content.unwrap_or(&[]);

    let binary = is_binary(old_bytes) || is_binary(new_bytes);

    if binary {
        let changed = old_bytes != new_bytes;
        return FilePatch {
            old_path: old_path.map(String::from),
            new_path: new_path.map(String::from),
            is_binary: changed,
            hunks: Vec::new(),
        };
    }

    let old_text = std::str::from_utf8(old_bytes).unwrap_or("");
    let new_text = std::str::from_utf8(new_bytes).unwrap_or("");

    if old_text == new_text {
        return FilePatch {
            old_path: old_path.map(String::from),
            new_path: new_path.map(String::from),
            is_binary: false,
            hunks: Vec::new(),
        };
    }

    let ops = myers_diff(old_text, new_text);
    let hunks = create_hunks(&ops, context);

    FilePatch {
        old_path: old_path.map(String::from),
        new_path: new_path.map(String::from),
        is_binary: false,
        hunks,
    }
}

fn format_range(start: usize, count: usize) -> String {
    if count == 1 {
        format!("{}", start)
    } else {
        format!("{},{}", start, count)
    }
}

/// Formats a collection of `FilePatch`es into standard unified diff string.
pub fn format_unified_diff(patches: &[FilePatch]) -> String {
    let mut out = String::new();

    for patch in patches {
        if patch.is_empty() {
            continue;
        }

        let p_old = patch.old_path.as_deref().unwrap_or("/dev/null");
        let p_new = patch.new_path.as_deref().unwrap_or("/dev/null");
        let display_path = if p_old != "/dev/null" { p_old } else { p_new };

        if patch.is_binary {
            out.push_str(&format!(
                "Binary files a/{} and b/{} differ\n",
                display_path, display_path
            ));
            continue;
        }

        out.push_str(&format!(
            "diff --dft a/{} b/{}\n",
            display_path, display_path
        ));
        if patch.old_path.is_none() {
            out.push_str("--- /dev/null\n");
        } else {
            out.push_str(&format!("--- a/{}\n", p_old));
        }

        if patch.new_path.is_none() {
            out.push_str("+++ /dev/null\n");
        } else {
            out.push_str(&format!("+++ b/{}\n", p_new));
        }

        for hunk in &patch.hunks {
            out.push_str(&format!(
                "@@ -{} +{} @@\n",
                format_range(hunk.old_start, hunk.old_count),
                format_range(hunk.new_start, hunk.new_count),
            ));
            for line in &hunk.lines {
                match line.kind {
                    EditOpKind::Equal => {
                        out.push(' ');
                        out.push_str(&line.content);
                    }
                    EditOpKind::Delete => {
                        out.push('-');
                        out.push_str(&line.content);
                    }
                    EditOpKind::Insert => {
                        out.push('+');
                        out.push_str(&line.content);
                    }
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_addition() {
        let patch = generate_file_patch(
            Some("test.txt"),
            Some("test.txt"),
            Some(b"line 1\n"),
            Some(b"line 1\nadded line\n"),
            3,
        );
        let diff = format_unified_diff(&[patch]);
        assert!(diff.contains("+added line"));
        assert!(diff.contains("@@ -1 +1,2 @@"));
    }

    #[test]
    fn test_format_binary() {
        let patch = generate_file_patch(
            Some("bin.dat"),
            Some("bin.dat"),
            Some(&[0x00, 0x01]),
            Some(&[0x00, 0x02]),
            3,
        );
        let diff = format_unified_diff(&[patch]);
        assert!(diff.contains("Binary files a/bin.dat and b/bin.dat differ"));
    }
}
