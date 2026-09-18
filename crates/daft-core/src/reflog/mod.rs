pub mod entry;

pub use entry::ReflogEntry;

use crate::error::ReflogError;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ReflogManager {
    dft_dir: PathBuf,
}

impl ReflogManager {
    pub fn new(dft_dir: impl Into<PathBuf>) -> Self {
        Self {
            dft_dir: dft_dir.into(),
        }
    }

    pub fn log_path(&self, ref_name: &str) -> PathBuf {
        self.dft_dir.join("logs").join(ref_name)
    }

    pub fn append(&self, ref_name: &str, entry: &ReflogEntry) -> Result<(), ReflogError> {
        let path = self.log_path(ref_name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;

        file.write_all(entry.format_line().as_bytes())?;
        file.flush()?;
        file.sync_all()?;
        Ok(())
    }

    pub fn read_all(&self, ref_name: &str) -> Result<Vec<ReflogEntry>, ReflogError> {
        let path = self.log_path(ref_name);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(&path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if !line.trim().is_empty() {
                entries.push(ReflogEntry::parse_line(&line)?);
            }
        }

        Ok(entries)
    }

    pub fn read_reverse(&self, ref_name: &str) -> Result<Vec<ReflogEntry>, ReflogError> {
        let mut entries = self.read_all(ref_name)?;
        entries.reverse();
        Ok(entries)
    }
}
