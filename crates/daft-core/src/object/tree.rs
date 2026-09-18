use super::error::ObjectError;
use crate::cas::ObjectId;
use std::cmp::Ordering;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileMode(pub u32);

impl FileMode {
    pub const REGULAR: FileMode = FileMode(0o100644); // Normal non-executable file
    pub const EXECUTABLE: FileMode = FileMode(0o100755); // Executable file
    pub const SYMLINK: FileMode = FileMode(0o120000); // Symbolic link
    pub const TREE: FileMode = FileMode(0o040000); // Subdirectory (subtree)
    pub const GITLINK: FileMode = FileMode(0o160000); // Submodule / Dimension mount pointer

    pub fn is_tree(&self) -> bool {
        self.0 == 0o040000
    }

    pub fn is_blob(&self) -> bool {
        self.0 == 0o100644 || self.0 == 0o100755 || self.0 == 0o120000
    }

    pub fn is_executable(&self) -> bool {
        self.0 == 0o100755
    }

    pub fn is_symlink(&self) -> bool {
        self.0 == 0o120000
    }

    pub fn is_gitlink(&self) -> bool {
        self.0 == 0o160000
    }

    /// Mode format for tree payload binary: octal string WITHOUT leading zero (Git canonical)
    /// e.g. 0o040000 -> "40000", 0o100644 -> "100644"
    pub fn to_tree_octal(&self) -> String {
        format!("{:o}", self.0)
    }

    /// Mode format for display: 6 octal digits with leading zeros
    /// e.g. 0o040000 -> "040000", 0o100644 -> "100644"
    pub fn to_display_octal(&self) -> String {
        format!("{:06o}", self.0)
    }

    /// Parse octal string, tolerating both "40000" and "040000", "100644", etc.
    pub fn from_octal_str(s: &str) -> Result<Self, ObjectError> {
        let val = u32::from_str_radix(s, 8)
            .map_err(|e| ObjectError::InvalidMode(s.to_string(), e.to_string()))?;
        match val {
            0o100644 | 0o100755 | 0o120000 | 0o040000 | 0o160000 => Ok(FileMode(val)),
            _ => Err(ObjectError::InvalidMode(
                s.to_string(),
                format!("unrecognized file mode 0o{:o}", val),
            )),
        }
    }
}

#[inline]
pub fn entry_sort_key<'a>(name: &'a str, mode: FileMode) -> impl Iterator<Item = u8> + 'a {
    name.as_bytes()
        .iter()
        .copied()
        .chain(if mode.is_tree() { Some(b'/') } else { None })
}

#[inline]
pub fn cmp_tree_entries(
    name_a: &str,
    mode_a: FileMode,
    name_b: &str,
    mode_b: FileMode,
) -> Ordering {
    entry_sort_key(name_a, mode_a).cmp(entry_sort_key(name_b, mode_b))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub mode: FileMode,
    pub name: String,
    pub oid: ObjectId,
}

impl TreeEntry {
    pub fn new(
        mode: FileMode,
        name: impl Into<String>,
        oid: ObjectId,
    ) -> Result<Self, ObjectError> {
        let name = name.into();
        Self::validate_name(&name)?;
        Ok(Self { mode, name, oid })
    }

    pub fn validate_name(name: &str) -> Result<(), ObjectError> {
        if name.is_empty() {
            return Err(ObjectError::InvalidEntryName("empty filename".to_string()));
        }
        if name == "." || name == ".." {
            return Err(ObjectError::InvalidEntryName(format!(
                "reserved name '{}'",
                name
            )));
        }
        if name.contains('/') {
            return Err(ObjectError::InvalidEntryName(format!(
                "filename contains slash: '{}'",
                name
            )));
        }
        if name.contains('\0') {
            return Err(ObjectError::InvalidEntryName(
                "filename contains null byte".to_string(),
            ));
        }
        Ok(())
    }

    pub fn cmp_canonical(&self, other: &Self) -> Ordering {
        cmp_tree_entries(&self.name, self.mode, &other.name, other.mode)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tree {
    entries: Vec<TreeEntry>,
}

impl Tree {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn from_entries(mut entries: Vec<TreeEntry>) -> Result<Self, ObjectError> {
        let mut seen = HashSet::with_capacity(entries.len());
        for entry in &entries {
            TreeEntry::validate_name(&entry.name)?;
            if !seen.insert(entry.name.as_str()) {
                return Err(ObjectError::DuplicateTreeEntry(entry.name.clone()));
            }
        }
        drop(seen);

        entries.sort_by(|a, b| a.cmp_canonical(b));
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[TreeEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, name: &str) -> Option<&TreeEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    pub fn insert(&mut self, entry: TreeEntry) -> Result<(), ObjectError> {
        TreeEntry::validate_name(&entry.name)?;
        self.entries.retain(|e| e.name != entry.name);
        self.entries.push(entry);
        self.entries.sort_by(|a, b| a.cmp_canonical(b));
        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> Option<TreeEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.name == name) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.entries.len() * 45);
        for entry in &self.entries {
            buf.extend_from_slice(entry.mode.to_tree_octal().as_bytes());
            buf.push(b' ');
            buf.extend_from_slice(entry.name.as_bytes());
            buf.push(0);
            buf.extend_from_slice(entry.oid.as_bytes()); // 32 raw bytes
        }
        buf
    }

    pub fn deserialize(data: &[u8]) -> Result<Self, ObjectError> {
        let est_entries = data.len() / 40;
        let mut entries: Vec<TreeEntry> = Vec::with_capacity(est_entries);
        let mut seen: HashSet<&str> = HashSet::with_capacity(est_entries);
        let mut offset = 0;
        let len = data.len();

        while offset < len {
            let space_pos = data[offset..]
                .iter()
                .position(|&b| b == b' ')
                .ok_or(ObjectError::TruncatedTree(offset))?
                + offset;
            let mode_str = std::str::from_utf8(&data[offset..space_pos])
                .map_err(|e| ObjectError::MalformedHeader(format!("mode utf8 error: {}", e)))?;
            let mode = FileMode::from_octal_str(mode_str)?;

            let name_start = space_pos + 1;
            let null_pos = data[name_start..]
                .iter()
                .position(|&b| b == 0)
                .ok_or(ObjectError::TruncatedTree(name_start))?
                + name_start;
            let name_str = std::str::from_utf8(&data[name_start..null_pos]).map_err(|e| {
                ObjectError::MalformedHeader(format!("entry name utf8 error: {}", e))
            })?;
            TreeEntry::validate_name(name_str)?;

            if !seen.insert(name_str) {
                return Err(ObjectError::DuplicateTreeEntry(name_str.to_string()));
            }

            let hash_start = null_pos + 1;
            let hash_end = hash_start + 32;
            if hash_end > len {
                return Err(ObjectError::TruncatedTree(hash_start));
            }
            let mut oid_bytes = [0u8; 32];
            oid_bytes.copy_from_slice(&data[hash_start..hash_end]);
            let oid = ObjectId::from_bytes(oid_bytes);

            let entry = TreeEntry {
                mode,
                name: name_str.to_string(),
                oid,
            };

            // Invariant verification: entries must be strictly canonically sorted
            if let Some(prev) = entries.last() {
                match prev.cmp_canonical(&entry) {
                    Ordering::Less => {}
                    Ordering::Equal => {
                        return Err(ObjectError::DuplicateTreeEntry(entry.name));
                    }
                    Ordering::Greater => {
                        return Err(ObjectError::TreeOutOfOrder(prev.name.clone(), entry.name));
                    }
                }
            }

            entries.push(entry);
            offset = hash_end;
        }

        Ok(Self { entries })
    }
}
