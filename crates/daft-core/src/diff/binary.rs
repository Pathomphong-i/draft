//! Binary content detection heuristic.

/// Checks whether the given byte slice represents binary content.
///
/// Scans up to the first 8,000 bytes. If a NUL byte (`0x00`) is encountered,
/// the content is classified as binary.
pub fn is_binary(data: &[u8]) -> bool {
    let check_len = data.len().min(8000);
    data[..check_len].contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_is_not_binary() {
        assert!(!is_binary(b"Hello world!\nThis is plain text."));
    }

    #[test]
    fn test_binary_with_null_byte() {
        assert!(is_binary(&[0x00, 0xFF, 0xFE, 0x01]));
        assert!(is_binary(&[b'h', b'e', b'l', b'l', b'o', 0x00, b'w']));
    }

    #[test]
    fn test_empty_is_not_binary() {
        assert!(!is_binary(&[]));
    }
}
