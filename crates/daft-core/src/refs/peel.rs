use super::ReferenceTarget;
use crate::cas::ObjectId;
use crate::error::RefError;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

const MAX_PEEL_DEPTH: usize = 10;

/// Resolves a reference name (including HEAD) down to an ObjectId.
/// Handles symbolic references, loops, and unborn branches.
pub fn peel_reference(dft_dir: &Path, name: &str) -> Result<ObjectId, RefError> {
    let mut current_name = name.to_string();
    let mut visited = HashSet::new();
    let mut depth = 0;

    loop {
        if depth >= MAX_PEEL_DEPTH {
            return Err(RefError::SymbolicRefLoop(format!(
                "max peel depth exceeded for '{}'",
                name
            )));
        }

        if !visited.insert(current_name.clone()) {
            return Err(RefError::SymbolicRefLoop(format!(
                "reference cycle detected at '{}'",
                current_name
            )));
        }

        let ref_path = if current_name == "HEAD" {
            dft_dir.join("HEAD")
        } else {
            dft_dir.join(&current_name)
        };

        if !ref_path.exists() {
            if current_name.starts_with("refs/heads/") {
                let branch = current_name.trim_start_matches("refs/heads/");
                return Err(RefError::UnbornBranch(branch.to_string()));
            }
            return Err(RefError::RefNotFound(current_name));
        }

        let content = fs::read_to_string(&ref_path)?;
        let target = ReferenceTarget::parse(&content)?;

        match target {
            ReferenceTarget::Direct(oid) => return Ok(oid),
            ReferenceTarget::Symbolic(next_ref) => {
                current_name = next_ref;
                depth += 1;
            }
        }
    }
}
