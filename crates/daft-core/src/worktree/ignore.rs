//! .daftignore rule processor for repository pruning and high performance scanning.

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct DaftIgnore {
    patterns: Vec<String>,
}

impl DaftIgnore {
    pub fn load_from_workdir(workdir: &Path) -> Self {
        let mut patterns = vec![
            ".dft".to_string(),
            ".git".to_string(),
            "target".to_string(),
            ".agents".to_string(),
            "node_modules".to_string(),
        ];

        for filename in &[".daftignore", ".dftignore"] {
            let ignore_file = workdir.join(filename);
            if ignore_file.exists() {
                if let Ok(content) = fs::read_to_string(&ignore_file) {
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() && !trimmed.starts_with('#') {
                            patterns.push(trimmed.trim_end_matches('/').to_string());
                        }
                    }
                }
            }
        }

        Self { patterns }
    }

    pub fn should_prune_dir(&self, dir_name: &str) -> bool {
        self.patterns.iter().any(|p| p == dir_name)
    }

    pub fn is_ignored(&self, rel_path: &str, _is_dir: bool) -> bool {
        let clean = rel_path.trim_start_matches("./").replace("\\", "/");
        let parts: Vec<&str> = clean.split('/').collect();

        for pattern in &self.patterns {
            let pat = pattern.trim_end_matches('/');
            if parts.iter().any(|&part| part == pat) {
                return true;
            }
            if clean == pat || clean.starts_with(&format!("{}/", pat)) {
                return true;
            }
        }
        false
    }
}
