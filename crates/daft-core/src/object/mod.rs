pub mod blob;
pub mod commit;
pub mod error;
pub mod signature;
pub mod tag;
pub mod tree;

pub use blob::Blob;
pub use commit::Commit;
pub use error::ObjectError;
pub use signature::Signature;
pub use tag::Tag;
pub use tree::{cmp_tree_entries, entry_sort_key, FileMode, Tree, TreeEntry};

use crate::cas::{ObjectId, ObjectType, RawObject};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Object {
    Blob(Blob),
    Tree(Tree),
    Commit(Commit),
    Tag(Tag),
}

impl Object {
    pub fn obj_type(&self) -> ObjectType {
        match self {
            Object::Blob(_) => ObjectType::Blob,
            Object::Tree(_) => ObjectType::Tree,
            Object::Commit(_) => ObjectType::Commit,
            Object::Tag(_) => ObjectType::Tag,
        }
    }

    pub fn serialize_payload(&self) -> Vec<u8> {
        match self {
            Object::Blob(b) => b.serialize().to_vec(),
            Object::Tree(t) => t.serialize(),
            Object::Commit(c) => c.serialize(),
            Object::Tag(t) => t.serialize(),
        }
    }

    pub fn serialize_envelope(&self) -> Vec<u8> {
        let payload = self.serialize_payload();
        let header = format!("{} {}\0", self.obj_type().as_str(), payload.len());
        let mut envelope = Vec::with_capacity(header.len() + payload.len());
        envelope.extend_from_slice(header.as_bytes());
        envelope.extend_from_slice(&payload);
        envelope
    }

    pub fn compute_id(&self) -> ObjectId {
        let envelope = self.serialize_envelope();
        ObjectId::hash(&envelope)
    }

    pub fn to_raw(&self) -> RawObject {
        RawObject::new(self.obj_type(), self.serialize_payload())
    }

    pub fn deserialize(obj_type: ObjectType, payload: &[u8]) -> Result<Self, ObjectError> {
        match obj_type {
            ObjectType::Blob => Ok(Object::Blob(Blob::deserialize(payload)?)),
            ObjectType::Tree => Ok(Object::Tree(Tree::deserialize(payload)?)),
            ObjectType::Commit => Ok(Object::Commit(Commit::deserialize(payload)?)),
            ObjectType::Tag => Ok(Object::Tag(Tag::deserialize(payload)?)),
        }
    }

    pub fn parse_envelope(envelope: &[u8]) -> Result<(ObjectType, ObjectId, Self), ObjectError> {
        let null_pos = envelope.iter().position(|&b| b == 0).ok_or_else(|| {
            ObjectError::MalformedEnvelope("missing null byte in envelope".to_string())
        })?;
        let header_str = std::str::from_utf8(&envelope[..null_pos]).map_err(|e| {
            ObjectError::MalformedEnvelope(format!("invalid envelope header UTF-8: {}", e))
        })?;
        let (type_str, size_str) = header_str.split_once(' ').ok_or_else(|| {
            ObjectError::MalformedEnvelope("header missing space delimiter".to_string())
        })?;
        let obj_type = ObjectType::parse(type_str)
            .map_err(|e| ObjectError::UnknownType(format!("invalid object type: {}", e)))?;
        let declared_size: usize = size_str
            .parse()
            .map_err(|e| ObjectError::MalformedEnvelope(format!("invalid size integer: {}", e)))?;
        let payload = &envelope[null_pos + 1..];
        if payload.len() != declared_size {
            return Err(ObjectError::SizeMismatch {
                declared: declared_size,
                actual: payload.len(),
            });
        }
        let id = ObjectId::hash(envelope);
        let obj = Self::deserialize(obj_type, payload)?;
        Ok((obj_type, id, obj))
    }

    pub fn as_blob(&self) -> Option<&Blob> {
        if let Object::Blob(b) = self {
            Some(b)
        } else {
            None
        }
    }

    pub fn as_tree(&self) -> Option<&Tree> {
        if let Object::Tree(t) = self {
            Some(t)
        } else {
            None
        }
    }

    pub fn as_commit(&self) -> Option<&Commit> {
        if let Object::Commit(c) = self {
            Some(c)
        } else {
            None
        }
    }

    pub fn as_tag(&self) -> Option<&Tag> {
        if let Object::Tag(t) = self {
            Some(t)
        } else {
            None
        }
    }
}
