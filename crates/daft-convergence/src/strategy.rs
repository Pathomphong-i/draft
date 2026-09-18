use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    #[default]
    ManualMarkers,
    Ours,
    Theirs,
    Union,
}

impl FromStr for MergeStrategy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().trim() {
            "markers" | "manual" | "manualmarkers" | "manual_markers" => Ok(Self::ManualMarkers),
            "ours" => Ok(Self::Ours),
            "theirs" => Ok(Self::Theirs),
            "union" => Ok(Self::Union),
            other => Err(format!(
                "Invalid merge strategy '{}'. Valid choices: ours, theirs, union, manual_markers",
                other
            )),
        }
    }
}

impl fmt::Display for MergeStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManualMarkers => write!(f, "manual_markers"),
            Self::Ours => write!(f, "ours"),
            Self::Theirs => write!(f, "theirs"),
            Self::Union => write!(f, "union"),
        }
    }
}
