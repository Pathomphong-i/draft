use super::error::CasError;
use super::id::ObjectId;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectType {
    Blob,
    Tree,
    Commit,
    Tag,
}

impl ObjectType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Blob => "blob",
            Self::Tree => "tree",
            Self::Commit => "commit",
            Self::Tag => "tag",
        }
    }

    pub fn parse(s: &str) -> Result<Self, CasError> {
        match s {
            "blob" => Ok(Self::Blob),
            "tree" => Ok(Self::Tree),
            "commit" => Ok(Self::Commit),
            "tag" => Ok(Self::Tag),
            other => Err(CasError::UnknownObjectType(other.to_string())),
        }
    }
}

impl fmt::Display for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// In-memory representation of an uncompressed CAS object with envelope metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawObject {
    pub object_type: ObjectType,
    pub data: Vec<u8>,
}

impl RawObject {
    pub fn new(object_type: ObjectType, data: Vec<u8>) -> Self {
        Self { object_type, data }
    }

    pub fn blob(data: Vec<u8>) -> Self {
        Self::new(ObjectType::Blob, data)
    }

    pub fn tree(data: Vec<u8>) -> Self {
        Self::new(ObjectType::Tree, data)
    }

    pub fn commit(data: Vec<u8>) -> Self {
        Self::new(ObjectType::Commit, data)
    }

    pub fn tag(data: Vec<u8>) -> Self {
        Self::new(ObjectType::Tag, data)
    }

    /// Produce the uncompressed framed byte envelope: `<type> <size>\0<payload>`
    pub fn framed_bytes(&self) -> Vec<u8> {
        let header = format!("{} {}\0", self.object_type.as_str(), self.data.len());
        let mut framed = Vec::with_capacity(header.len() + self.data.len());
        framed.extend_from_slice(header.as_bytes());
        framed.extend_from_slice(&self.data);
        framed
    }

    /// Compute the cryptographic SHA-256 ObjectId across the entire uncompressed framed envelope.
    pub fn compute_id(&self) -> ObjectId {
        let framed = self.framed_bytes();
        ObjectId::hash(&framed)
    }

    /// Parse an uncompressed framed byte slice into a RawObject.
    pub fn parse_framed(bytes: &[u8]) -> Result<Self, CasError> {
        let space_pos = bytes.iter().position(|&b| b == b' ').ok_or_else(|| {
            CasError::InvalidHeader("missing space delimiter in envelope header".into())
        })?;

        let null_pos = bytes[space_pos + 1..]
            .iter()
            .position(|&b| b == 0)
            .map(|pos| space_pos + 1 + pos)
            .ok_or_else(|| {
                CasError::InvalidHeader("missing null byte terminator in envelope header".into())
            })?;

        let type_str = std::str::from_utf8(&bytes[..space_pos]).map_err(|e| {
            CasError::InvalidHeader(format!("header type contains invalid UTF-8: {e}"))
        })?;
        let object_type = ObjectType::parse(type_str)?;

        let size_str = std::str::from_utf8(&bytes[space_pos + 1..null_pos]).map_err(|e| {
            CasError::InvalidHeader(format!("header size contains invalid UTF-8: {e}"))
        })?;
        let declared_size: usize = size_str.parse().map_err(|e| {
            CasError::InvalidHeader(format!("header size is not a valid integer: {e}"))
        })?;

        let payload = &bytes[null_pos + 1..];
        if payload.len() != declared_size {
            return Err(CasError::SizeMismatch {
                declared: declared_size,
                actual: payload.len(),
            });
        }

        Ok(Self {
            object_type,
            data: payload.to_vec(),
        })
    }
}
