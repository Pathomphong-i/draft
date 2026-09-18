use super::error::ObjectError;
use super::signature::Signature;
use crate::cas::ObjectId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub tree: ObjectId,
    pub parents: Vec<ObjectId>,
    pub author: Signature,
    pub committer: Signature,
    pub gpg_sig: Option<String>,
    pub extra_headers: Vec<(String, String)>,
    pub message: String,
}

impl Commit {
    pub fn new(
        tree: ObjectId,
        parents: Vec<ObjectId>,
        author: Signature,
        committer: Signature,
        message: impl Into<String>,
    ) -> Self {
        Self {
            tree,
            parents,
            author,
            committer,
            gpg_sig: None,
            extra_headers: Vec::new(),
            message: message.into(),
        }
    }

    pub fn is_root(&self) -> bool {
        self.parents.is_empty()
    }

    pub fn is_merge(&self) -> bool {
        self.parents.len() >= 2
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = String::new();
        out.push_str(&format!("tree {}\n", self.tree.to_hex()));
        for parent in &self.parents {
            out.push_str(&format!("parent {}\n", parent.to_hex()));
        }
        out.push_str(&format!("author {}\n", self.author));
        out.push_str(&format!("committer {}\n", self.committer));

        for (k, v) in &self.extra_headers {
            out.push_str(&format!("{} {}\n", k, v));
        }

        if let Some(ref sig) = self.gpg_sig {
            out.push_str("gpgsig ");
            let lines: Vec<&str> = sig.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                if i == 0 {
                    out.push_str(line);
                    out.push('\n');
                } else {
                    out.push(' ');
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }

        out.push('\n'); // Demarcation between headers and message
        out.push_str(&self.message);
        out.into_bytes()
    }

    pub fn deserialize(data: &[u8]) -> Result<Self, ObjectError> {
        let text = std::str::from_utf8(data)
            .map_err(|e| ObjectError::MalformedEnvelope(format!("commit UTF-8 error: {}", e)))?;

        let mut tree: Option<ObjectId> = None;
        let mut parents = Vec::new();
        let mut author: Option<Signature> = None;
        let mut committer: Option<Signature> = None;
        let mut gpg_sig: Option<String> = None;
        let mut extra_headers = Vec::new();

        let mut header_end_offset = 0;
        let mut current_header_name: Option<String> = None;
        let mut current_header_val = String::new();
        let mut char_idx = 0;

        for line in text.split_inclusive('\n') {
            if line == "\n" || line == "\r\n" {
                char_idx += line.len();
                header_end_offset = char_idx;
                break;
            }

            let trimmed_line = line.trim_end_matches(&['\r', '\n'][..]);
            if let Some(rest) = trimmed_line.strip_prefix(' ') {
                // Multiline header continuation
                if current_header_name.as_deref() == Some("gpgsig") {
                    current_header_val.push('\n');
                    current_header_val.push_str(rest);
                }
            } else {
                if let Some(ref name) = current_header_name {
                    if name == "gpgsig" {
                        gpg_sig = Some(current_header_val.clone());
                    }
                }
                current_header_name = None;
                current_header_val.clear();

                if let Some((key, val)) = trimmed_line.split_once(' ') {
                    match key {
                        "tree" => {
                            tree = Some(ObjectId::from_hex(val).map_err(|e| {
                                ObjectError::InvalidHash(format!("tree hash invalid: {}", e))
                            })?);
                        }
                        "parent" => {
                            let p = ObjectId::from_hex(val).map_err(|e| {
                                ObjectError::InvalidHash(format!("parent hash invalid: {}", e))
                            })?;
                            parents.push(p);
                        }
                        "author" => {
                            author = Some(Signature::parse(val)?);
                        }
                        "committer" => {
                            committer = Some(Signature::parse(val)?);
                        }
                        "gpgsig" => {
                            current_header_name = Some("gpgsig".to_string());
                            current_header_val = val.to_string();
                        }
                        other => {
                            extra_headers.push((other.to_string(), val.to_string()));
                        }
                    }
                }
            }
            char_idx += line.len();
        }

        if let Some(ref name) = current_header_name {
            if name == "gpgsig" {
                gpg_sig = Some(current_header_val);
            }
        }

        let tree = tree.ok_or_else(|| ObjectError::MissingCommitHeader("tree".to_string()))?;
        let author =
            author.ok_or_else(|| ObjectError::MissingCommitHeader("author".to_string()))?;
        let committer =
            committer.ok_or_else(|| ObjectError::MissingCommitHeader("committer".to_string()))?;

        let message = if header_end_offset < text.len() {
            text[header_end_offset..].to_string()
        } else {
            String::new()
        };

        Ok(Self {
            tree,
            parents,
            author,
            committer,
            gpg_sig,
            extra_headers,
            message,
        })
    }
}
