use crate::cas::ObjectStore;
use crate::error::{IndexError, RefError, RepoError};
use crate::index::{Index, IndexLock};
use crate::reflog::ReflogManager;
use crate::refs::{RefManager, Reference, ReferenceTarget};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Repository {
    workdir: Option<PathBuf>,
    dft_dir: PathBuf,
    cas: Arc<ObjectStore>,
}

impl Repository {
    pub fn open(dft_dir: &Path) -> Result<Self, RepoError> {
        let dft_dir_buf = dft_dir
            .canonicalize()
            .unwrap_or_else(|_| dft_dir.to_path_buf());
        if !dft_dir_buf.join("HEAD").exists() {
            return Err(RepoError::NotARepository(dft_dir_buf));
        }

        let is_dot_dft = dft_dir_buf
            .file_name()
            .map(|n| n == ".dft")
            .unwrap_or(false);
        let workdir = if is_dot_dft {
            dft_dir_buf.parent().map(|p| p.to_path_buf())
        } else {
            None
        };

        let objects_dir = dft_dir_buf.join("objects");
        let cas = Arc::new(ObjectStore::init(&objects_dir)?);

        Ok(Self {
            workdir,
            dft_dir: dft_dir_buf,
            cas,
        })
    }

    pub fn discover(start_path: &Path) -> Result<Self, RepoError> {
        let canonical = start_path
            .canonicalize()
            .map_err(|_| RepoError::NotARepository(start_path.to_path_buf()))?;
        let mut current = canonical.as_path();

        loop {
            // 1. Check for standard working tree (.dft directory)
            let candidate_dft = current.join(".dft");
            if candidate_dft.is_dir() && candidate_dft.join("HEAD").is_file() {
                return Self::open(&candidate_dft);
            }

            // 2. Check for bare repository (current directory contains objects/ and HEAD)
            if current.join("objects").is_dir() && current.join("HEAD").is_file() {
                return Self::open(current);
            }

            // 3. Ascend to parent directory
            match current.parent() {
                Some(parent) if parent != current => {
                    current = parent;
                }
                _ => return Err(RepoError::NotARepository(start_path.to_path_buf())),
            }
        }
    }

    pub fn workdir(&self) -> Option<&Path> {
        self.workdir.as_deref()
    }

    pub fn dft_dir(&self) -> &Path {
        &self.dft_dir
    }

    pub fn is_bare(&self) -> bool {
        self.workdir.is_none()
    }

    pub fn cas(&self) -> &Arc<ObjectStore> {
        &self.cas
    }

    pub fn index_path(&self) -> PathBuf {
        self.dft_dir.join("index")
    }

    pub fn index(&self) -> Result<Index, IndexError> {
        let path = self.index_path();
        if path.exists() {
            Index::read_from(&path)
        } else {
            Ok(Index::new())
        }
    }

    pub fn lock_index(&self) -> Result<IndexLock, IndexError> {
        IndexLock::acquire(&self.index_path())
    }

    pub fn refs(&self) -> RefManager {
        RefManager::new(&self.dft_dir)
    }

    pub fn head(&self) -> Result<Reference, RefError> {
        self.refs().read_ref("HEAD")
    }

    pub fn set_head(&self, target: &ReferenceTarget) -> Result<(), RefError> {
        self.refs().write_ref("HEAD", target, None, None)
    }

    pub fn reflog(&self) -> ReflogManager {
        ReflogManager::new(&self.dft_dir)
    }
}
