use super::error::ObjectError;
use super::signature::Signature;
use crate::cas::{ObjectId, ObjectType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub target: ObjectId,
    pub target_type: ObjectType,
    pub name: String,
    pub tagger: Option<Signature>,
    pub message: String,
}

impl Tag {
    pub fn new(
        target: ObjectId,
        target_type: ObjectType,
        name: impl Into<String>,
        tagger: Option<Signature>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            target,
            target_type,
            name: name.into(),
            tagger,
            message: message.into(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = String::new();
        out.push_str(&format!("object {}\n", self.target.to_hex()));
        out.push_str(&format!("type {}\n", self.target_type.as_str()));
        out.push_str(&format!("tag {}\n", self.name));
        if let Some(ref tagger) = self.tagger {
            out.push_str(&format!("tagger {}\n", tagger));
        }
        out.push('\n');
        out.push_str(&self.message);
        out.into_bytes()
    }

    pub fn deserialize(data: &[u8]) -> Result<Self, ObjectError> {
        let text = std::str::from_utf8(data)
            .map_err(|e| ObjectError::MalformedEnvelope(format!("tag UTF-8 error: {}", e)))?;

        let mut target: Option<ObjectId> = None;
        let mut target_type: Option<ObjectType> = None;
        let mut name: Option<String> = None;
        let mut tagger: Option<Signature> = None;
        let mut header_end_offset = 0;
        let mut char_idx = 0;

        for line in text.split_inclusive('\n') {
            if line == "\n" || line == "\r\n" {
                char_idx += line.len();
                header_end_offset = char_idx;
                break;
            }
            let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
            if let Some((key, val)) = trimmed.split_once(' ') {
                match key {
                    "object" => {
                        target = Some(ObjectId::from_hex(val).map_err(|e| {
                            ObjectError::InvalidHash(format!("tag target hash invalid: {}", e))
                        })?);
                    }
                    "type" => {
                        target_type = Some(ObjectType::parse(val).map_err(|e| {
                            ObjectError::UnknownType(format!("invalid tag target type: {}", e))
                        })?);
                    }
                    "tag" => {
                        name = Some(val.to_string());
                    }
                    "tagger" => {
                        tagger = Some(Signature::parse(val)?);
                    }
                    _ => {}
                }
            }
            char_idx += line.len();
        }

        let target = target
            .ok_or_else(|| ObjectError::MalformedHeader("missing 'object' in tag".to_string()))?;
        let target_type = target_type
            .ok_or_else(|| ObjectError::MalformedHeader("missing 'type' in tag".to_string()))?;
        let name = name
            .ok_or_else(|| ObjectError::MalformedHeader("missing 'tag' name in tag".to_string()))?;

        let message = if header_end_offset < text.len() {
            text[header_end_offset..].to_string()
        } else {
            String::new()
        };

        Ok(Self {
            target,
            target_type,
            name,
            tagger,
            message,
        })
    }
}
