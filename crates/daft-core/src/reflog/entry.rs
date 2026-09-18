use crate::cas::ObjectId;
use crate::error::ReflogError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflogEntry {
    pub old_oid: ObjectId,
    pub new_oid: ObjectId,
    pub committer_name: String,
    pub committer_email: String,
    pub timestamp: i64,
    pub tz_offset: String,
    pub message: String,
}

impl ReflogEntry {
    pub fn new(
        old_oid: ObjectId,
        new_oid: ObjectId,
        committer_name: impl Into<String>,
        committer_email: impl Into<String>,
        timestamp: i64,
        tz_offset: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            old_oid,
            new_oid,
            committer_name: committer_name.into(),
            committer_email: committer_email.into(),
            timestamp,
            tz_offset: tz_offset.into(),
            message: message.into(),
        }
    }

    pub fn format_line(&self) -> String {
        format!(
            "{} {} {} <{}> {} {}\t{}\n",
            self.old_oid.to_hex(),
            self.new_oid.to_hex(),
            self.committer_name,
            self.committer_email,
            self.timestamp,
            self.tz_offset,
            self.message
        )
    }

    pub fn parse_line(line: &str) -> Result<Self, ReflogError> {
        let (header, message) = line
            .split_once('\t')
            .ok_or_else(|| ReflogError::MalformedLine(line.to_string()))?;

        let message = message.trim_end_matches(['\r', '\n']).to_string();

        let mut parts = header.split_whitespace();
        let old_hex = parts
            .next()
            .ok_or_else(|| ReflogError::MalformedLine(line.to_string()))?;
        let new_hex = parts
            .next()
            .ok_or_else(|| ReflogError::MalformedLine(line.to_string()))?;

        let old_oid = ObjectId::from_hex(old_hex)
            .map_err(|e| ReflogError::MalformedLine(format!("invalid old oid: {}", e)))?;
        let new_oid = ObjectId::from_hex(new_hex)
            .map_err(|e| ReflogError::MalformedLine(format!("invalid new oid: {}", e)))?;

        // Find <email> in the remaining header string
        let after_hashes = header[old_hex.len() + 1 + new_hex.len()..].trim_start();
        let email_start = after_hashes
            .find('<')
            .ok_or_else(|| ReflogError::MalformedLine(line.to_string()))?;
        let email_end = after_hashes
            .find('>')
            .ok_or_else(|| ReflogError::MalformedLine(line.to_string()))?;

        let committer_name = after_hashes[..email_start].trim().to_string();
        let committer_email = after_hashes[email_start + 1..email_end].to_string();

        let after_email = after_hashes[email_end + 1..].trim_start();
        let mut time_parts = after_email.split_whitespace();
        let timestamp_str = time_parts
            .next()
            .ok_or_else(|| ReflogError::MalformedLine(line.to_string()))?;
        let tz_offset = time_parts.next().unwrap_or("+0000").to_string();

        let timestamp = timestamp_str
            .parse::<i64>()
            .map_err(|_| ReflogError::MalformedLine(line.to_string()))?;

        Ok(Self {
            old_oid,
            new_oid,
            committer_name,
            committer_email,
            timestamp,
            tz_offset,
            message,
        })
    }
}
