use crate::error::RefError;

/// Validate reference name syntax according to Git / POSIX reference rules.
pub fn validate_ref_name(name: &str) -> Result<(), RefError> {
    if name.is_empty() {
        return Err(RefError::InvalidRefName("empty ref name".to_string()));
    }
    if name == "@" {
        return Err(RefError::InvalidRefName(
            "ref name cannot be single '@'".to_string(),
        ));
    }
    if name.starts_with('/') || name.ends_with('/') || name.contains("//") {
        return Err(RefError::InvalidRefName(format!(
            "ref name '{}' cannot start/end with slash or contain consecutive slashes",
            name
        )));
    }
    if name.ends_with(".lock") {
        return Err(RefError::InvalidRefName(format!(
            "ref name '{}' cannot end with '.lock'",
            name
        )));
    }
    if name.contains("..") {
        return Err(RefError::InvalidRefName(format!(
            "ref name '{}' cannot contain '..'",
            name
        )));
    }
    if name.contains("@{") {
        return Err(RefError::InvalidRefName(format!(
            "ref name '{}' cannot contain '@{{'",
            name
        )));
    }

    for comp in name.split('/') {
        if comp.starts_with('.') || comp.ends_with('.') {
            return Err(RefError::InvalidRefName(format!(
                "ref component '{}' cannot start or end with '.'",
                comp
            )));
        }
    }

    for ch in name.chars() {
        if ch.is_ascii_control()
            || ch == ' '
            || ch == '~'
            || ch == '^'
            || ch == ':'
            || ch == '?'
            || ch == '*'
            || ch == '['
            || ch == '\\'
        {
            return Err(RefError::InvalidRefName(format!(
                "ref name '{}' contains invalid character '{}'",
                name, ch
            )));
        }
    }

    Ok(())
}
