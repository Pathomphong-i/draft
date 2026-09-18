//! Myers O(ND) difference algorithm implementation.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditOpKind {
    Equal,
    Insert,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditOp<'a> {
    pub kind: EditOpKind,
    pub text: &'a str,
    pub old_line: Option<usize>, // 1-based index in old file
    pub new_line: Option<usize>, // 1-based index in new file
}

/// Splits text into lines while preserving trailing newline information if desired,
/// or splits into lines. Standard unified diff operates on lines.
fn split_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut start = 0;
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            lines.push(&text[start..=i]);
            start = i + 1;
        }
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

/// Computes the Myers diff between two text strings line by line.
pub fn myers_diff<'a>(old_text: &'a str, new_text: &'a str) -> Vec<EditOp<'a>> {
    let a = split_lines(old_text);
    let b = split_lines(new_text);
    let n = a.len();
    let m = b.len();

    if n == 0 && m == 0 {
        return Vec::new();
    }

    if n == 0 {
        return b
            .into_iter()
            .enumerate()
            .map(|(idx, line)| EditOp {
                kind: EditOpKind::Insert,
                text: line,
                old_line: None,
                new_line: Some(idx + 1),
            })
            .collect();
    }

    if m == 0 {
        return a
            .into_iter()
            .enumerate()
            .map(|(idx, line)| EditOp {
                kind: EditOpKind::Delete,
                text: line,
                old_line: Some(idx + 1),
                new_line: None,
            })
            .collect();
    }

    // Common prefix trimming
    let mut prefix_len = 0;
    while prefix_len < n && prefix_len < m && a[prefix_len] == b[prefix_len] {
        prefix_len += 1;
    }

    // Common suffix trimming
    let mut suffix_len = 0;
    while suffix_len < (n - prefix_len)
        && suffix_len < (m - prefix_len)
        && a[n - 1 - suffix_len] == b[m - 1 - suffix_len]
    {
        suffix_len += 1;
    }

    let a_mid = &a[prefix_len..n - suffix_len];
    let b_mid = &b[prefix_len..m - suffix_len];
    let mid_n = a_mid.len();
    let mid_m = b_mid.len();

    let mut mid_ops = if mid_n == 0 && mid_m == 0 {
        Vec::new()
    } else if mid_n == 0 {
        b_mid
            .iter()
            .map(|&line| RawOp {
                kind: EditOpKind::Insert,
                text: line,
            })
            .collect()
    } else if mid_m == 0 {
        a_mid
            .iter()
            .map(|&line| RawOp {
                kind: EditOpKind::Delete,
                text: line,
            })
            .collect()
    } else {
        compute_myers_ses(a_mid, b_mid)
    };

    // Construct full EditOp sequence
    let mut result = Vec::with_capacity(n + m);
    let mut old_idx = 1;
    let mut new_idx = 1;

    for &line in &a[..prefix_len] {
        result.push(EditOp {
            kind: EditOpKind::Equal,
            text: line,
            old_line: Some(old_idx),
            new_line: Some(new_idx),
        });
        old_idx += 1;
        new_idx += 1;
    }

    for op in mid_ops.drain(..) {
        match op.kind {
            EditOpKind::Equal => {
                result.push(EditOp {
                    kind: EditOpKind::Equal,
                    text: op.text,
                    old_line: Some(old_idx),
                    new_line: Some(new_idx),
                });
                old_idx += 1;
                new_idx += 1;
            }
            EditOpKind::Delete => {
                result.push(EditOp {
                    kind: EditOpKind::Delete,
                    text: op.text,
                    old_line: Some(old_idx),
                    new_line: None,
                });
                old_idx += 1;
            }
            EditOpKind::Insert => {
                result.push(EditOp {
                    kind: EditOpKind::Insert,
                    text: op.text,
                    old_line: None,
                    new_line: Some(new_idx),
                });
                new_idx += 1;
            }
        }
    }

    for &line in &a[n - suffix_len..] {
        result.push(EditOp {
            kind: EditOpKind::Equal,
            text: line,
            old_line: Some(old_idx),
            new_line: Some(new_idx),
        });
        old_idx += 1;
        new_idx += 1;
    }

    result
}

struct RawOp<'a> {
    kind: EditOpKind,
    text: &'a str,
}

fn compute_myers_ses<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<RawOp<'a>> {
    let n = a.len();
    let m = b.len();
    let max = n + m;
    let offset = max as isize;
    let mut v = vec![0isize; 2 * max + 1];
    let mut trace: Vec<Vec<isize>> = Vec::new();

    for d in 0..=max {
        trace.push(v.clone());
        let mut k = -(d as isize);
        while k <= d as isize {
            let k_idx = (k + offset) as usize;
            let mut x = if k == -(d as isize)
                || (k != d as isize && v[(k - 1 + offset) as usize] < v[(k + 1 + offset) as usize])
            {
                v[(k + 1 + offset) as usize]
            } else {
                v[(k - 1 + offset) as usize] + 1
            };
            let mut y = x - k;

            while x < n as isize && y < m as isize && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }

            v[k_idx] = x;

            if x >= n as isize && y >= m as isize {
                // Backtrack to find edits
                return backtrack(a, b, &trace, d, x, y, offset);
            }

            k += 2;
        }
    }

    Vec::new()
}

fn backtrack<'a>(
    a: &[&'a str],
    b: &[&'a str],
    trace: &[Vec<isize>],
    d: usize,
    mut x: isize,
    mut y: isize,
    offset: isize,
) -> Vec<RawOp<'a>> {
    let mut ops = Vec::new();

    for step in (1..=d).rev() {
        let v = &trace[step];
        let k = x - y;
        let prev_k = if k == -(step as isize)
            || (k != step as isize && v[(k - 1 + offset) as usize] < v[(k + 1 + offset) as usize])
        {
            k + 1
        } else {
            k - 1
        };

        let prev_x = v[(prev_k + offset) as usize];
        let prev_y = prev_x - prev_k;

        let (snake_start_x, snake_start_y) = if prev_k == k + 1 {
            (prev_x, prev_y + 1)
        } else {
            (prev_x + 1, prev_y)
        };

        // Equal snake backward along diagonal k
        while x > snake_start_x && y > snake_start_y {
            x -= 1;
            y -= 1;
            ops.push(RawOp {
                kind: EditOpKind::Equal,
                text: a[x as usize],
            });
        }

        if prev_k == k + 1 {
            // Insert from b
            y -= 1;
            ops.push(RawOp {
                kind: EditOpKind::Insert,
                text: b[y as usize],
            });
        } else {
            // Delete from a
            x -= 1;
            ops.push(RawOp {
                kind: EditOpKind::Delete,
                text: a[x as usize],
            });
        }
    }

    // Remaining snake from (0, 0)
    while x > 0 && y > 0 {
        x -= 1;
        y -= 1;
        ops.push(RawOp {
            kind: EditOpKind::Equal,
            text: a[x as usize],
        });
    }

    ops.reverse();
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_myers_identical() {
        let text = "alpha\nbeta\ngamma\n";
        let ops = myers_diff(text, text);
        assert_eq!(ops.len(), 3);
        assert!(ops.iter().all(|op| op.kind == EditOpKind::Equal));
    }

    #[test]
    fn test_myers_insert_delete() {
        let old = "line 1\nline 2\n";
        let new = "line 1\nline 2\nline 3\n";
        let ops = myers_diff(old, new);
        assert_eq!(ops.len(), 3);
        assert_eq!(ops[2].kind, EditOpKind::Insert);
        assert_eq!(ops[2].text, "line 3\n");
    }

    #[test]
    fn test_myers_replacement() {
        let old = "a\nb\nc\n";
        let new = "a\nx\nc\n";
        let ops = myers_diff(old, new);
        let kinds: Vec<_> = ops.iter().map(|o| o.kind).collect();
        assert_eq!(
            kinds,
            vec![
                EditOpKind::Equal,
                EditOpKind::Delete,
                EditOpKind::Insert,
                EditOpKind::Equal
            ]
        );
    }

    #[test]
    fn test_myers_inner_snake_preserved() {
        let old = "header\nA\ncommon\nB\nfooter\n";
        let new = "header\nA_mod\ncommon\nB_mod\nfooter\n";
        let ops = myers_diff(old, new);
        let common = ops.iter().find(|op| op.text == "common\n");
        assert!(common.is_some(), "Inner snake must be preserved");
        assert_eq!(common.unwrap().kind, EditOpKind::Equal);
    }

    #[test]
    fn test_myers_multiple_clean_islands() {
        let old = "island 1\nconf 1 old\nisland 2\nconf 2 old\nisland 3\n";
        let new = "island 1\nconf 1 new\nisland 2\nconf 2 new\nisland 3\n";
        let ops = myers_diff(old, new);
        let equal_texts: Vec<&str> = ops
            .iter()
            .filter(|o| o.kind == EditOpKind::Equal)
            .map(|o| o.text)
            .collect();
        assert_eq!(equal_texts, vec!["island 1\n", "island 2\n", "island 3\n"]);
    }
}
