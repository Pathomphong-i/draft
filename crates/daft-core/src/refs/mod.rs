pub mod lock;
pub mod manager;
pub mod peel;
pub mod validate;

pub use lock::RefLock;
pub use manager::RefManager;
pub use peel::peel_reference;
pub use validate::validate_ref_name;

use crate::cas::ObjectId;
use crate::error::RefError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceTarget {
    /// Symbolic reference pointing to another ref name (e.g. "refs/heads/main")
    Symbolic(String),
    /// Direct reference containing raw SHA-256 ObjectId
    Direct(ObjectId),
}

impl ReferenceTarget {
    pub fn parse(content: &str) -> Result<Self, RefError> {
        let trimmed = content.trim();
        if let Some(rest) = trimmed.strip_prefix("ref:") {
            let target = rest.trim();
            if target.is_empty() {
                return Err(RefError::InvalidRefName(
                    "empty symbolic target".to_string(),
                ));
            }
            Ok(ReferenceTarget::Symbolic(target.to_string()))
        } else {
            let oid = ObjectId::from_hex(trimmed).map_err(|_| {
                RefError::InvalidRefName(format!("invalid hex object id '{}'", trimmed))
            })?;
            Ok(ReferenceTarget::Direct(oid))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub name: String,
    pub target: ReferenceTarget,
}

impl Reference {
    pub fn is_symbolic(&self) -> bool {
        matches!(self.target, ReferenceTarget::Symbolic(_))
    }

    pub fn is_direct(&self) -> bool {
        matches!(self.target, ReferenceTarget::Direct(_))
    }

    pub fn ref_type(&self) -> ReferenceType {
        ReferenceType::classify(&self.name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceType {
    Branch,
    Tag,
    Dimension,
    Remote,
    Head,
    Other,
}

impl ReferenceType {
    pub fn classify(name: &str) -> Self {
        if name == "HEAD" {
            Self::Head
        } else if name.starts_with("refs/heads/") {
            Self::Branch
        } else if name.starts_with("refs/tags/") {
            Self::Tag
        } else if name.starts_with("refs/dimensions/") {
            Self::Dimension
        } else if name.starts_with("refs/remotes/") {
            Self::Remote
        } else {
            Self::Other
        }
    }
}
