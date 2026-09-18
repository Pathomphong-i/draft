use super::error::CasError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::fmt;
use std::str::FromStr;

/// A 32-byte SHA-256 Content Addressable Identifier.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ObjectId([u8; 32]);

impl ObjectId {
    /// The all-zero null object ID.
    pub const ZERO: Self = Self([0u8; 32]);

    /// Construct an ObjectId from a 32-byte raw array.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Access the underlying 32 raw bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Convert into raw 32-byte array.
    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }

    /// Check if this is the null/zero object ID.
    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 32]
    }

    /// Compute SHA-256 digest of data as an ObjectId.
    pub fn hash(data: &[u8]) -> Self {
        let digest = Sha256::digest(data);
        Self(digest.into())
    }

    /// Format as 64-character lowercase hex string.
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Parse from a 64-character hex string.
    pub fn from_hex(s: &str) -> Result<Self, CasError> {
        let trimmed = s.trim();
        if trimmed.len() != 64 {
            return Err(CasError::InvalidObjectId(
                s.to_string(),
                format!("expected 64 hex characters, got {}", trimmed.len()),
            ));
        }
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(trimmed, &mut bytes)
            .map_err(|e| CasError::InvalidObjectId(s.to_string(), e.to_string()))?;
        Ok(Self(bytes))
    }

    /// Split the hex digest into a 2-character directory fanout prefix and a 62-character filename remainder.
    pub fn fanout_parts(&self) -> (String, String) {
        let hex = self.to_hex();
        (hex[..2].to_string(), hex[2..].to_string())
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl fmt::Debug for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ObjectId(\"{}\")", self.to_hex())
    }
}

impl FromStr for ObjectId {
    type Err = CasError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

impl AsRef<[u8]> for ObjectId {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<[u8; 32]> for ObjectId {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl Serialize for ObjectId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for ObjectId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ObjectId::from_hex(&s).map_err(serde::de::Error::custom)
    }
}
