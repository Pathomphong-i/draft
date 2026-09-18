//! In-Memory Key-Value Store with Write-Ahead Log (WAL).

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

pub struct AetherStorage {
    map: BTreeMap<String, String>,
    wal_path: PathBuf,
    wal_file: File,
}

impl AetherStorage {
    pub fn open<P: AsRef<Path>>(wal_path: P) -> io::Result<Self> {
        let path = wal_path.as_ref().to_path_buf();
        let mut map = BTreeMap::new();

        if path.exists() {
            let file = File::open(&path)?;
            let reader = BufReader::new(file);
            for line in reader.lines() {
                let line = line?;
                let mut parts = line.splitn(3, ' ');
                match (parts.next(), parts.next(), parts.next()) {
                    (Some("SET"), Some(k), Some(v)) => {
                        map.insert(k.to_string(), v.to_string());
                    }
                    (Some("DEL"), Some(k), _) => {
                        map.remove(k);
                    }
                    _ => {}
                }
            }
        }

        let wal_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;

        Ok(Self {
            map,
            wal_path: path,
            wal_file,
        })
    }

    pub fn set(&mut self, key: &str, value: &str) -> io::Result<()> {
        writeln!(self.wal_file, "SET {} {}", key, value)?;
        self.wal_file.flush()?;
        self.map.insert(key.to_string(), value.to_string());
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.map.get(key)
    }

    pub fn del(&mut self, key: &str) -> io::Result<bool> {
        if self.map.contains_key(key) {
            writeln!(self.wal_file, "DEL {}", key)?;
            self.wal_file.flush()?;
            self.map.remove(key);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn scan(&self, prefix: &str) -> Vec<(&String, &String)> {
        self.map
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .collect()
    }
}
