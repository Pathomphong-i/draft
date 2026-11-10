//! .daftignore / .dftignore / .gitignore rule processor for repository pruning,
//! automatic Python virtual environment & library exclusion, and high-performance scanning.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct IgnoreRule {
    pattern: String,
    negated: bool,
    dir_only: bool,
    anchored: bool,
    has_slash: bool,
}

impl IgnoreRule {
    fn parse(raw_line: &str) -> Option<Self> {
        let mut line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }

        let negated = if let Some(rest) = line.strip_prefix('!') {
            line = rest.trim();
            true
        } else {
            false
        };

        if line.is_empty() {
            return None;
        }

        let mut normalized = line.replace('\\', "/");
        if let Some(stripped) = normalized.strip_prefix("./") {
            normalized = stripped.to_string();
        }

        let dir_only = normalized.ends_with('/');
        let anchored = normalized.starts_with('/');
        let clean_pat = normalized.trim_matches('/').to_string();
        if clean_pat.is_empty() {
            return None;
        }

        let has_slash = clean_pat.contains('/');

        Some(Self {
            pattern: clean_pat,
            negated,
            dir_only,
            anchored,
            has_slash,
        })
    }

    fn matches(&self, rel_path: &str, is_dir: bool) -> bool {
        if self.dir_only && !is_dir {
            return false;
        }

        if self.anchored {
            return glob_match(&self.pattern, rel_path);
        }

        if let Some(suffix_pat) = self.pattern.strip_prefix("**/") {
            if glob_match(suffix_pat, rel_path) {
                return true;
            }
            for (idx, ch) in rel_path.char_indices() {
                if ch == '/' && glob_match(suffix_pat, &rel_path[idx + 1..]) {
                    return true;
                }
            }
            return false;
        }

        if self.has_slash {
            glob_match(&self.pattern, rel_path)
        } else {
            let basename = rel_path.rsplit('/').next().unwrap_or(rel_path);
            glob_match(&self.pattern, basename)
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DaftIgnore {
    rules: Vec<IgnoreRule>,
    workdir: Option<PathBuf>,
}

const DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    // VCS & Agent Metadata
    ".dft/",
    ".draft/",
    ".git/",
    ".hg/",
    ".svn/",
    ".agents/",
    // Python Virtual Environments, Bytecode & Package Caches
    "venv/",
    ".venv/",
    "env/",
    "ENV/",
    "virtualenv/",
    ".virtualenv/",
    "__pycache__/",
    "site-packages/",
    "dist-packages/",
    "pip-wheel-metadata/",
    ".pytest_cache/",
    ".mypy_cache/",
    ".ruff_cache/",
    ".pytype/",
    ".pyre/",
    ".hypothesis/",
    ".tox/",
    ".nox/",
    ".eggs/",
    "*.egg-info",
    "*.egg",
    "*.pyc",
    "*.pyo",
    "*.pyd",
    "*.py[cod]",
    "*$py.class",
    ".Python",
    "pyvenv.cfg",
    ".conda/",
    "conda-meta/",
    ".ipynb_checkpoints/",
    // Rust / Node / Web / Mobile / JVM / Build & Library Caches
    "target/",
    "node_modules/",
    "bower_components/",
    "jspm_packages/",
    ".pnpm-store/",
    ".yarn/",
    ".next/",
    ".nuxt/",
    ".svelte-kit/",
    ".output/",
    ".vercel/",
    ".netlify/",
    ".turbo/",
    ".parcel-cache/",
    ".gradle/",
    ".kotlin/",
    "DerivedData/",
    "Pods/",
    ".dart_tool/",
    ".pub-cache/",
    "cmake-build-debug/",
    "cmake-build-release/",
    ".terraform/",
    ".serverless/",
    ".DS_Store",
];

impl DaftIgnore {
    pub fn load_from_workdir(workdir: &Path) -> Self {
        let mut rules = Vec::with_capacity(DEFAULT_IGNORE_PATTERNS.len() + 32);

        for pat in DEFAULT_IGNORE_PATTERNS {
            if let Some(rule) = IgnoreRule::parse(pat) {
                rules.push(rule);
            }
        }

        // If workdir itself is an in-place Python virtual environment (contains pyvenv.cfg),
        // ignore the standard top-level virtualenv directories.
        if workdir.join("pyvenv.cfg").exists() {
            for venv_dir in &[
                "/lib/",
                "/lib64/",
                "/include/",
                "/bin/",
                "/Scripts/",
                "/Share/",
            ] {
                if let Some(rule) = IgnoreRule::parse(venv_dir) {
                    rules.push(rule);
                }
            }
        }

        // If workdir is inside `.dft/dimensions/<name>/workspace`, also load ignore rules from the main repo root.
        let mut search_roots = Vec::new();
        if let Some(repo_root) = Self::infer_repo_root_from_dimension_ws(workdir) {
            search_roots.push(repo_root);
        }
        search_roots.push(workdir.to_path_buf());

        for root in &search_roots {
            for filename in &[
                ".gitignore",
                ".daftignore",
                ".dftignore",
                ".dft/info/exclude",
                ".git/info/exclude",
            ] {
                let ignore_file = root.join(filename);
                if ignore_file.is_file() {
                    if let Ok(content) = fs::read_to_string(&ignore_file) {
                        for line in content.lines() {
                            if let Some(rule) = IgnoreRule::parse(line) {
                                rules.push(rule);
                            }
                        }
                    }
                }
            }
        }

        Self {
            rules,
            workdir: Some(workdir.to_path_buf()),
        }
    }

    fn infer_repo_root_from_dimension_ws(workdir: &Path) -> Option<PathBuf> {
        // Matches `<repo_root>/.dft/dimensions/<dim_name>/workspace`
        let ws_name = workdir.file_name()?.to_str()?;
        if ws_name != "workspace" {
            return None;
        }
        let dim_dir = workdir.parent()?;
        let dims_dir = dim_dir.parent()?;
        if dims_dir.file_name()?.to_str()? != "dimensions" {
            return None;
        }
        let dft_dir = dims_dir.parent()?;
        let dft_name = dft_dir.file_name()?.to_str()?;
        if dft_name != ".dft" && dft_name != ".draft" {
            return None;
        }
        dft_dir.parent().map(|p| p.to_path_buf())
    }

    /// Detects whether a directory on disk is a Python/Conda virtual environment,
    /// a Python `lib`/`lib64` site-packages tree, or contains a self-ignoring `.gitignore` (`*`).
    pub fn is_venv_or_self_ignored_dir(dir_path: &Path) -> bool {
        // 1. Standard Python venv or Conda environment markers
        if dir_path.join("pyvenv.cfg").is_file() || dir_path.join("conda-meta").is_dir() {
            return true;
        }

        // 2. Self-ignoring directory marker (e.g. `venv/.gitignore`, `uv` venvs, `cargo`, `pytest`, `ruff` caches containing `*`)
        for ignore_name in &[".gitignore", ".dftignore", ".daftignore"] {
            let ig = dir_path.join(ignore_name);
            if ig.is_file() {
                if let Ok(content) = fs::read_to_string(&ig) {
                    let mut ignores_all = false;
                    let mut has_negation = false;
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if trimmed.is_empty() || trimmed.starts_with('#') {
                            continue;
                        }
                        if trimmed == "*" || trimmed == "/*" || trimmed == "**" {
                            ignores_all = true;
                        } else if trimmed.starts_with('!') && trimmed != "!.gitignore" {
                            has_negation = true;
                        }
                    }
                    if ignores_all && !has_negation {
                        return true;
                    }
                }
            }
        }

        // 3. Python environment `lib` / `lib64` directory containing `site-packages` or `pythonX.Y/site-packages`
        if let Some(name) = dir_path.file_name().and_then(|n| n.to_str()) {
            if name == "lib" || name == "lib64" {
                if dir_path.join("site-packages").is_dir()
                    || dir_path.join("dist-packages").is_dir()
                {
                    return true;
                }
                if let Some(parent) = dir_path.parent() {
                    if parent.join("pyvenv.cfg").is_file() {
                        return true;
                    }
                }
                if let Ok(entries) = fs::read_dir(dir_path) {
                    for entry in entries.flatten() {
                        let sub_name = entry.file_name();
                        let sub_str = sub_name.to_string_lossy();
                        if (sub_str.starts_with("python2")
                            || sub_str.starts_with("python3")
                            || sub_str.starts_with("pypy"))
                            && entry.path().is_dir()
                            && (entry.path().join("site-packages").is_dir()
                                || entry.path().join("dist-packages").is_dir()
                                || entry.path().join("orig-prefix.txt").is_file())
                        {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Prunes a directory by its single component name.
    pub fn should_prune_dir(&self, dir_name: &str) -> bool {
        if dir_name == ".dft" || dir_name == ".draft" || dir_name == ".git" {
            return true;
        }
        let mut ignored = false;
        for rule in &self.rules {
            if !rule.anchored && !rule.has_slash && glob_match(&rule.pattern, dir_name) {
                ignored = !rule.negated;
            }
        }
        ignored
    }

    /// High-performance `WalkDir::filter_entry` predicate that prunes ignored directories
    /// (including Python virtualenvs, `lib/pythonX.Y/site-packages`, and `.gitignore` rules)
    /// before descending into them.
    pub fn should_prune_entry(&self, entry: &walkdir::DirEntry, workdir: &Path) -> bool {
        if entry.depth() == 0 {
            return false;
        }

        let name = entry.file_name().to_string_lossy();
        if name == ".dft" || name == ".draft" || name == ".git" {
            return true;
        }

        let is_dir = entry.file_type().is_dir();
        if is_dir && Self::is_venv_or_self_ignored_dir(entry.path()) {
            return true;
        }

        if let Ok(rel) = entry.path().strip_prefix(workdir) {
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if !rel_str.is_empty() {
                return self.is_ignored(&rel_str, is_dir);
            }
        }

        if is_dir {
            return self.should_prune_dir(&name);
        }

        false
    }

    /// Evaluates whether a repository-relative path is ignored.
    pub fn is_ignored(&self, rel_path: &str, is_dir: bool) -> bool {
        let clean = rel_path
            .trim_start_matches("./")
            .trim_matches('/')
            .replace('\\', "/");
        if clean.is_empty() {
            return false;
        }

        for part in clean.split('/') {
            if part == ".dft" || part == ".draft" || part == ".git" {
                return true;
            }
        }

        // Check each ancestor directory prefix first, then the full path
        let mut slash_indices: Vec<usize> = clean
            .char_indices()
            .filter_map(|(i, c)| if c == '/' { Some(i) } else { None })
            .collect();
        slash_indices.push(clean.len());

        for (idx, &end) in slash_indices.iter().enumerate() {
            let prefix = &clean[..end];
            let prefix_is_dir = if idx + 1 < slash_indices.len() {
                true
            } else {
                is_dir
            };

            if prefix_is_dir {
                if let Some(ref wd) = self.workdir {
                    let full_dir = wd.join(prefix);
                    if full_dir.is_dir() && Self::is_venv_or_self_ignored_dir(&full_dir) {
                        return true;
                    }
                }
            }

            let mut matched = false;
            for rule in &self.rules {
                if rule.matches(prefix, prefix_is_dir) {
                    matched = !rule.negated;
                }
            }
            if matched {
                return true;
            }
        }

        false
    }
}

/// Fast glob matcher supporting `*`, `**`, `?`, and `[...]` character classes.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p_chars: Vec<char> = pattern.chars().collect();
    let t_chars: Vec<char> = text.chars().collect();
    glob_match_slice(&p_chars, &t_chars)
}

fn glob_match_slice(pat: &[char], txt: &[char]) -> bool {
    let mut pi = 0;
    let mut ti = 0;
    let mut star_pi: Option<usize> = None;
    let mut star_ti: usize = 0;
    let mut star_allows_slash = false;

    while ti < txt.len() {
        if pi < pat.len() && pat[pi] == '*' {
            let mut is_double = false;
            while pi + 1 < pat.len() && pat[pi + 1] == '*' {
                is_double = true;
                pi += 1;
            }
            star_pi = Some(pi);
            star_ti = ti;
            star_allows_slash = is_double;
            pi += 1;
        } else if pi < pat.len() && pat[pi] == '?' && txt[ti] != '/' {
            pi += 1;
            ti += 1;
        } else if pi < pat.len() && pat[pi] == '[' {
            if let Some((matched, next_pi)) = match_char_class(&pat[pi..], txt[ti]) {
                if matched {
                    pi += next_pi;
                    ti += 1;
                    continue;
                }
            }
            if let Some(sp) = star_pi {
                if !star_allows_slash && txt[star_ti] == '/' {
                    return false;
                }
                star_ti += 1;
                ti = star_ti;
                pi = sp + 1;
            } else {
                return false;
            }
        } else if pi < pat.len() && pat[pi] == txt[ti] {
            pi += 1;
            ti += 1;
        } else if let Some(sp) = star_pi {
            if !star_allows_slash && txt[star_ti] == '/' {
                return false;
            }
            star_ti += 1;
            ti = star_ti;
            pi = sp + 1;
        } else {
            return false;
        }
    }

    while pi < pat.len() && pat[pi] == '*' {
        pi += 1;
    }

    pi == pat.len()
}

fn match_char_class(pat: &[char], ch: char) -> Option<(bool, usize)> {
    if pat.is_empty() || pat[0] != '[' || ch == '/' {
        return None;
    }
    let mut i = 1;
    let mut negated = false;
    if i < pat.len() && (pat[i] == '!' || pat[i] == '^') {
        negated = true;
        i += 1;
    }

    let mut matched = false;
    let mut found_close = false;
    while i < pat.len() {
        if pat[i] == ']' {
            found_close = true;
            i += 1;
            break;
        }
        if i + 2 < pat.len() && pat[i + 1] == '-' && pat[i + 2] != ']' {
            let start = pat[i];
            let end = pat[i + 2];
            if ch >= start && ch <= end {
                matched = true;
            }
            i += 3;
        } else {
            if pat[i] == ch {
                matched = true;
            }
            i += 1;
        }
    }

    if !found_close {
        return None;
    }

    Some((if negated { !matched } else { matched }, i))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_python_and_lib_ignores() {
        let tmp = TempDir::new().unwrap();
        let ignore = DaftIgnore::load_from_workdir(tmp.path());

        assert!(ignore.is_ignored("venv/lib/python3.11/site-packages/numpy/core.py", false));
        assert!(ignore.is_ignored(".venv/bin/python", false));
        assert!(ignore.is_ignored("infra/venv/pyvenv.cfg", false));
        assert!(ignore.is_ignored("infra/.venv/lib/site.py", false));
        assert!(ignore.is_ignored("backend/env/lib/python3.12/os.py", false));
        assert!(ignore.is_ignored("src/__pycache__/main.cpython-311.pyc", false));
        assert!(ignore.is_ignored("pkg/module.pyc", false));
        assert!(ignore.is_ignored("pkg/module.pyo", false));
        assert!(ignore.is_ignored("pkg/module.pyd", false));
        assert!(ignore.is_ignored("my_pkg.egg-info/PKG-INFO", false));
        assert!(ignore.is_ignored("lib/python3.11/site-packages/requests/__init__.py", false));

        // Legitimate source files in lib/ or src/ must NOT be ignored
        assert!(!ignore.is_ignored("lib/math.rs", false));
        assert!(!ignore.is_ignored("src/lib.rs", false));
        assert!(!ignore.is_ignored("src/main.py", false));
    }

    #[test]
    fn test_custom_venv_and_python_lib_detection() {
        let tmp = TempDir::new().unwrap();
        // Custom-named Python virtual environment with pyvenv.cfg
        let custom_venv = tmp.path().join("my_custom_python_env");
        fs::create_dir_all(custom_venv.join("lib/python3.11/site-packages")).unwrap();
        fs::write(custom_venv.join("pyvenv.cfg"), "home = /usr/bin\n").unwrap();
        fs::write(
            custom_venv.join("lib/python3.11/site-packages/pkg.py"),
            "x = 1\n",
        )
        .unwrap();

        // Standalone Python lib/python3.11/site-packages without pyvenv.cfg
        let standalone_lib = tmp.path().join("lib/python3.11/site-packages");
        fs::create_dir_all(&standalone_lib).unwrap();
        fs::write(standalone_lib.join("pip.py"), "y = 2\n").unwrap();

        let ignore = DaftIgnore::load_from_workdir(tmp.path());
        assert!(ignore.is_ignored(
            "my_custom_python_env/lib/python3.11/site-packages/pkg.py",
            false
        ));
        assert!(ignore.is_ignored("lib/python3.11/site-packages/pip.py", false));
    }

    #[test]
    fn test_gitignore_and_glob_rules() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join(".gitignore"),
            "android/CatoShop/app/build/\n*.log\n!important.log\n",
        )
        .unwrap();

        let ignore = DaftIgnore::load_from_workdir(tmp.path());
        assert!(ignore.is_ignored(
            "android/CatoShop/app/build/outputs/apk/debug/app.apk",
            false
        ));
        assert!(ignore.is_ignored("debug.log", false));
        assert!(!ignore.is_ignored("important.log", false));
    }
}
