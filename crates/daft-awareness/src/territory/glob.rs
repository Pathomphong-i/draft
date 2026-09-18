//! Pure-Rust zero-dependency glob matching and pattern intersection engine.

/// Normalizes a path or glob pattern to forward slashes and strips leading `./`.
pub fn normalize_glob(pattern: &str) -> String {
    let mut s = pattern.replace('\\', "/");
    while s.starts_with("./") {
        s = s[2..].to_string();
    }
    s
}

/// Matches a normalized file path against a glob pattern.
/// Supports:
/// - `*` matches any characters within a single path segment (cannot cross `/`)
/// - `**` matches any characters across path segments (including `/`)
/// - `?` matches a single character (not `/`)
/// - Trailing `/` matches directory and all children under it
pub fn matches_glob(pattern: &str, path: &str) -> bool {
    let pat = normalize_glob(pattern);
    let target = normalize_glob(path);

    if pat == target || pat == "**" || pat == "." || pat == "*" && !target.contains('/') {
        return true;
    }

    // Trailing slash e.g. "src/auth/" matches "src/auth" or "src/auth/..."
    if let Some(prefix) = pat.strip_suffix('/') {
        if target == prefix || target.starts_with(&format!("{}/", prefix)) {
            return true;
        }
    }

    // Convert glob to segment-based matching
    match_segments(&pat, &target)
}

fn match_segments(pat: &str, target: &str) -> bool {
    let pat_parts: Vec<&str> = pat.split('/').collect();
    let tgt_parts: Vec<&str> = target.split('/').collect();

    match_parts(&pat_parts, &tgt_parts)
}

fn match_parts(pat: &[&str], tgt: &[&str]) -> bool {
    if pat.is_empty() {
        return tgt.is_empty();
    }

    if pat[0] == "**" {
        // Try matching 0 or more target parts
        for i in 0..=tgt.len() {
            if match_parts(&pat[1..], &tgt[i..]) {
                return true;
            }
        }
        return false;
    }

    if tgt.is_empty() {
        return false;
    }

    if match_single_pattern(pat[0], tgt[0]) {
        return match_parts(&pat[1..], &tgt[1..]);
    }

    false
}

/// Matches single filename segment (cannot cross `/`).
fn match_single_pattern(pat: &str, text: &str) -> bool {
    let pat_chars: Vec<char> = pat.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();

    fn helper(p: &[char], t: &[char]) -> bool {
        if p.is_empty() {
            return t.is_empty();
        }

        if p[0] == '*' {
            for i in 0..=t.len() {
                if helper(&p[1..], &t[i..]) {
                    return true;
                }
            }
            return false;
        }

        if t.is_empty() {
            return false;
        }

        if p[0] == '?' || p[0] == t[0] {
            return helper(&p[1..], &t[1..]);
        }

        false
    }

    helper(&pat_chars, &text_chars)
}

/// Checks whether two glob patterns can match overlapping file paths.
pub fn patterns_overlap(pat_a: &str, pat_b: &str) -> bool {
    let norm_a = normalize_glob(pat_a);
    let norm_b = normalize_glob(pat_b);

    if norm_a == norm_b || norm_a == "**" || norm_a == "." || norm_b == "**" || norm_b == "." {
        return true;
    }

    // Direct match check: does pat_a match pat_b or vice versa?
    if matches_glob(&norm_a, &norm_b) || matches_glob(&norm_b, &norm_a) {
        return true;
    }

    // Subtree prefixes
    let clean_a = norm_a.trim_end_matches('/').trim_end_matches("/**");
    let clean_b = norm_b.trim_end_matches('/').trim_end_matches("/**");

    if clean_a == clean_b
        || clean_a.starts_with(&format!("{}/", clean_b))
        || clean_b.starts_with(&format!("{}/", clean_a))
    {
        return true;
    }

    false
}
