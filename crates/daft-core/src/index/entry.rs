use crate::cas::ObjectId;
use crate::error::IndexError;

pub const DIRC_MAGIC: &[u8; 4] = b"DIRC";
pub const DIRC_VERSION: u32 = 2;
pub const FIXED_ENTRY_SIZE: usize = 74;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Stage {
    Normal = 0,   // Stage 0: clean / normal / resolved
    Ancestor = 1, // Stage 1: merge base / common ancestor
    Ours = 2,     // Stage 2: target branch (HEAD)
    Theirs = 3,   // Stage 3: incoming branch (MERGE_HEAD)
}

impl Stage {
    pub fn from_u16(flags: u16) -> Self {
        match (flags & 0x3000) >> 12 {
            0 => Stage::Normal,
            1 => Stage::Ancestor,
            2 => Stage::Ours,
            3 => Stage::Theirs,
            _ => unreachable!(),
        }
    }

    pub fn to_bits(self) -> u16 {
        (self as u16) << 12
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IndexTime {
    pub sec: u32,
    pub nsec: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    pub ctime: IndexTime,
    pub mtime: IndexTime,
    pub dev: u32,
    pub ino: u32,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub file_size: u32,
    pub oid: ObjectId,
    pub flags: u16,
    pub path: String,
}

impl IndexEntry {
    pub fn validate_path(path: &str) -> Result<(), IndexError> {
        if path.is_empty() {
            return Err(IndexError::InvalidPath("empty path".to_string()));
        }
        if path.contains('\0') {
            return Err(IndexError::InvalidPath(
                "path contains null byte".to_string(),
            ));
        }
        Ok(())
    }

    pub fn new(
        path: impl Into<String>,
        oid: ObjectId,
        mode: u32,
        stage: Stage,
        file_size: u32,
    ) -> Result<Self, IndexError> {
        let path = path.into();
        Self::validate_path(&path)?;
        let path_len = (path.len().min(4095) as u16) & 0x0FFF;
        let flags = stage.to_bits() | path_len;
        Ok(Self {
            ctime: IndexTime::default(),
            mtime: IndexTime::default(),
            dev: 0,
            ino: 0,
            mode,
            uid: 0,
            gid: 0,
            file_size,
            oid,
            flags,
            path,
        })
    }

    pub fn stage(&self) -> Stage {
        Stage::from_u16(self.flags)
    }

    pub fn set_stage(&mut self, stage: Stage) {
        self.flags = (self.flags & !0x3000) | stage.to_bits();
    }

    pub fn is_assume_valid(&self) -> bool {
        (self.flags & 0x8000) != 0
    }

    pub fn set_assume_valid(&mut self, val: bool) {
        if val {
            self.flags |= 0x8000;
        } else {
            self.flags &= !0x8000;
        }
    }

    pub fn padding_len(&self) -> usize {
        let unpadded = FIXED_ENTRY_SIZE + self.path.len();
        8 - (unpadded % 8)
    }

    pub fn total_serialized_len(&self) -> usize {
        FIXED_ENTRY_SIZE + self.path.len() + self.padding_len()
    }
}
