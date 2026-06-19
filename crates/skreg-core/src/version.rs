//! Validation for version-segment strings used in registry URLs and CLI args.

/// Returns `true` when `segment` is a syntactically valid version segment:
/// the literal `"latest"`, or a non-empty string of ASCII alphanumerics plus
/// `.`, `-`, `+`, at most 32 bytes long.
///
/// This is a syntactic guard only (it rejects path traversal and oversized
/// input); it does not check that the version exists in the registry.
#[must_use]
pub fn is_valid_segment(segment: &str) -> bool {
    if segment == "latest" {
        return true;
    }
    if segment.is_empty() || segment.len() > 32 {
        return false;
    }
    segment
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '+')
}

#[cfg(test)]
mod tests {
    use super::is_valid_segment;

    #[test]
    fn accepts_latest() {
        assert!(is_valid_segment("latest"));
    }

    #[test]
    fn accepts_semver() {
        assert!(is_valid_segment("1.2.3"));
        assert!(is_valid_segment("1.0.0-alpha.1"));
        assert!(is_valid_segment("2.0.0+build.1"));
    }

    #[test]
    fn rejects_empty() {
        assert!(!is_valid_segment(""));
    }

    #[test]
    fn rejects_too_long() {
        assert!(!is_valid_segment(&"1".repeat(33)));
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(!is_valid_segment("../etc/passwd"));
        assert!(!is_valid_segment("1.0/bad"));
    }

    #[test]
    fn rejects_special_chars() {
        assert!(!is_valid_segment("1.0.0 beta"));
        assert!(!is_valid_segment("1.0.0@tag"));
    }
}
