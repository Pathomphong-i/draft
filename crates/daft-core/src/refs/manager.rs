use super::lock::RefLock;
use super::peel::peel_reference;
use super::validate::validate_ref_name;
use super::{Reference, ReferenceTarget};
use crate::cas::ObjectId;
use crate::error::RefError;
use crate::reflog::{ReflogEntry, ReflogManager};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct RefManager {
    dft_dir: PathBuf,
}

impl RefManager {
    pub fn new(dft_dir: impl Into<PathBuf>) -> Self {
        Self {
            dft_dir: dft_dir.into(),
        }
    }

    pub fn dft_dir(&self) -> &Path {
        &self.dft_dir
    }

    pub fn ref_path(&self, name: &str) -> PathBuf {
        if name == "HEAD" {
            self.dft_dir.join("HEAD")
        } else {
            self.dft_dir.join(name)
        }
    }

    pub fn read_ref(&self, name: &str) -> Result<Reference, RefError> {
        let path = self.ref_path(name);
        if !path.exists() {
            return Err(RefError::RefNotFound(name.to_string()));
        }
        let content = fs::read_to_string(&path)?;
        let target = ReferenceTarget::parse(&content)?;
        Ok(Reference {
            name: name.to_string(),
            target,
        })
    }

    pub fn write_ref(
        &self,
        name: &str,
        target: &ReferenceTarget,
        expected_old: Option<&ReferenceTarget>,
        log_entry: Option<&ReflogEntry>,
    ) -> Result<(), RefError> {
        if name != "HEAD" {
            validate_ref_name(name)?;
        }

        let path = self.ref_path(name);
        let mut lock = RefLock::acquire(&path, expected_old)?;
        lock.write_target(target)?;
        lock.commit()?;

        if let Some(entry) = log_entry {
            let reflog = ReflogManager::new(&self.dft_dir);
            let _ = reflog.append(name, entry);
        }

        Ok(())
    }

    pub fn delete_ref(
        &self,
        name: &str,
        expected_old: Option<&ReferenceTarget>,
    ) -> Result<(), RefError> {
        if name == "HEAD" {
            return Err(RefError::InvalidRefName("cannot delete HEAD".to_string()));
        }
        validate_ref_name(name)?;
        let path = self.ref_path(name);
        if !path.exists() {
            return Err(RefError::RefNotFound(name.to_string()));
        }

        let _lock = RefLock::acquire(&path, expected_old)?;
        if !path.exists() {
            return Err(RefError::RefNotFound(name.to_string()));
        }

        fs::remove_file(&path)?;
        Ok(())
    }

    pub fn list_refs(&self, prefix: &str) -> Result<Vec<Reference>, RefError> {
        let mut results = Vec::new();
        let search_dir = self.dft_dir.join(prefix);
        if !search_dir.exists() {
            return Ok(results);
        }

        for entry in WalkDir::new(&search_dir).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let path = entry.path();
                let file_name = entry.file_name().to_string_lossy();
                if file_name.ends_with(".lock") {
                    continue;
                }
                if let Ok(rel) = path.strip_prefix(&self.dft_dir) {
                    let ref_name = rel.to_string_lossy().replace('\\', "/");
                    if let Ok(r) = self.read_ref(&ref_name) {
                        results.push(r);
                    }
                }
            }
        }

        results.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(results)
    }

    pub fn resolve(&self, name: &str) -> Result<ObjectId, RefError> {
        peel_reference(&self.dft_dir, name)
    }
}
