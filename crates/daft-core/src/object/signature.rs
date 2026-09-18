use super::error::ObjectError;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub name: String,
    pub email: String,
    pub time: i64,
    pub tz_offset: i32, // Offset in minutes
}

impl Signature {
    pub fn new(
        name: impl Into<String>,
        email: impl Into<String>,
        time: i64,
        tz_offset: i32,
    ) -> Self {
        Self {
            name: name.into(),
            email: email.into(),
            time,
            tz_offset,
        }
    }

    pub fn now(name: impl Into<String>, email: impl Into<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            name: name.into(),
            email: email.into(),
            time: now.timestamp(),
            tz_offset: 0,
        }
    }

    pub fn format_tz(offset_minutes: i32) -> String {
        let sign = if offset_minutes >= 0 { '+' } else { '-' };
        let abs = offset_minutes.abs();
        let hours = abs / 60;
        let mins = abs % 60;
        format!("{}{:02}{:02}", sign, hours, mins)
    }

    pub fn parse_tz(tz_str: &str) -> Result<i32, ObjectError> {
        if tz_str.len() != 5 {
            return Err(ObjectError::MalformedSignature(format!(
                "invalid tz format '{}', expected 5 characters (+HHMM or -HHMM)",
                tz_str
            )));
        }
        let sign = match &tz_str[0..1] {
            "+" => 1,
            "-" => -1,
            _ => {
                return Err(ObjectError::MalformedSignature(format!(
                    "invalid tz sign in '{}'",
                    tz_str
                )))
            }
        };
        let hours: i32 = tz_str[1..3]
            .parse()
            .map_err(|e| ObjectError::MalformedSignature(format!("invalid tz hours: {}", e)))?;
        let mins: i32 = tz_str[3..5]
            .parse()
            .map_err(|e| ObjectError::MalformedSignature(format!("invalid tz mins: {}", e)))?;
        Ok(sign * (hours * 60 + mins))
    }

    pub fn parse(s: &str) -> Result<Self, ObjectError> {
        let open_angle = s
            .rfind('<')
            .ok_or_else(|| ObjectError::MalformedSignature("missing '<'".to_string()))?;
        let close_angle = s
            .rfind('>')
            .ok_or_else(|| ObjectError::MalformedSignature("missing '>'".to_string()))?;
        if close_angle <= open_angle {
            return Err(ObjectError::MalformedSignature(
                "'>' appears before '<'".to_string(),
            ));
        }

        let name = s[..open_angle].trim().to_string();
        let email = s[open_angle + 1..close_angle].to_string();

        let remainder = s[close_angle + 1..].trim();
        let mut tokens = remainder.split_whitespace();
        let time_str = tokens
            .next()
            .ok_or_else(|| ObjectError::MalformedSignature("missing timestamp".to_string()))?;
        let tz_str = tokens
            .next()
            .ok_or_else(|| ObjectError::MalformedSignature("missing timezone".to_string()))?;

        let time: i64 = time_str.parse().map_err(|e| {
            ObjectError::MalformedSignature(format!("invalid timestamp '{}': {}", time_str, e))
        })?;
        let tz_offset = Self::parse_tz(tz_str)?;

        Ok(Self {
            name,
            email,
            time,
            tz_offset,
        })
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} <{}> {} {}",
            self.name,
            self.email,
            self.time,
            Self::format_tz(self.tz_offset)
        )
    }
}

impl std::str::FromStr for Signature {
    type Err = ObjectError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}
