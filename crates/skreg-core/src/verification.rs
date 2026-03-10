//! Publisher verification tier.

use serde::{Deserialize, Serialize};

/// The verification tier of a published package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationKind {
    /// Publisher used a self-generated key. Key consistency enforced at namespace level.
    SelfSigned,
    /// Publisher holds a CA-issued cert verified by the skreg Publisher CA.
    Publisher,
}

impl VerificationKind {
    /// Short display string for TUI/CLI table columns (7 chars max).
    #[must_use]
    pub fn short_label(&self) -> &'static str {
        match self {
            Self::SelfSigned => "◈ self",
            Self::Publisher => "✦ pub ",
        }
    }

    /// Full display label.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::SelfSigned => "self-signed",
            Self::Publisher => "publisher",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_snake_case() {
        assert_eq!(
            serde_json::to_string(&VerificationKind::SelfSigned).unwrap(),
            "\"self_signed\""
        );
        assert_eq!(
            serde_json::to_string(&VerificationKind::Publisher).unwrap(),
            "\"publisher\""
        );
    }

    #[test]
    fn deserializes_from_snake_case() {
        let v: VerificationKind = serde_json::from_str("\"publisher\"").unwrap();
        assert_eq!(v, VerificationKind::Publisher);
    }

    #[test]
    fn short_label_is_at_most_7_chars() {
        assert!(VerificationKind::SelfSigned.short_label().chars().count() <= 7);
        assert!(VerificationKind::Publisher.short_label().chars().count() <= 7);
    }
}
