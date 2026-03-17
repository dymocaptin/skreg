//! Stage: Static Analysis — YARA + language-specific tools.

pub mod pass1;
pub mod pass2;
pub mod startup;

use std::path::Path;

use thiserror::Error;

/// A single finding from any analysis tool.
#[derive(Debug, Clone)]
pub struct Finding {
    /// Relative path of the file that triggered the finding.
    pub file: String,
    /// Which tool produced this finding.
    pub tool: String,
    /// Tool-specific rule or check identifier.
    pub rule_id: String,
    /// Normalised severity.
    pub severity: Severity,
    /// Human-readable description (internal use only — never sent to publishers).
    pub message: String,
}

/// Normalised severity across all tools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    /// Finding blocks publishing.
    Error,
    /// Finding blocks publishing.
    Warning,
    /// Informational — recorded but does not block.
    Info,
}

impl Severity {
    /// Returns `true` if this severity causes rejection.
    #[must_use]
    pub fn is_blocking(&self) -> bool {
        matches!(self, Severity::Error | Severity::Warning)
    }
}

/// Trait implemented by every analysis tool adapter.
pub trait Analyzer: Send + Sync {
    /// Run the analyser against `file` and return a list of findings.
    ///
    /// # Errors
    ///
    /// Returns `Err` on subprocess failure, timeout, or I/O error (distinct
    /// from a non-empty findings list, which is a normal result).
    fn analyze(&self, file: &Path) -> Result<Vec<Finding>, StaticAnalysisError>;
}

/// Errors produced by the static analysis stage.
#[derive(Debug, Error)]
pub enum StaticAnalysisError {
    /// A YARA rule file failed to compile.
    #[error("YARA compilation error: {0}")]
    YaraCompilation(String),
    /// A subprocess tool was not found or failed to execute.
    #[error("tool '{tool}' failed: {reason}")]
    ToolError {
        /// Name of the tool that failed.
        tool: String,
        /// Reason for the failure.
        reason: String,
    },
    /// A subprocess exceeded its per-file timeout.
    #[error("tool '{0}' timed out")]
    Timeout(String),
    /// A required tool or resource is missing at startup.
    #[error("missing required component: {0}")]
    MissingComponent(String),
    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Entry point for the static analysis stage.
///
/// Runs Pass 1 (magic bytes + YARA) then Pass 2 (deep per-language analysis)
/// on the unpacked package directory. Returns `Ok(())` if no blocking findings.
///
/// # Errors
///
/// Returns `Err` on infrastructure failure or when blocking findings are detected.
#[allow(unreachable_code)]
pub fn run_static_analysis(path: &Path) -> Result<(), StaticAnalysisError> {
    // Implemented in Task 9 after pass1 and pass2 are written.
    let _ = path;
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_severity_is_blocking() {
        assert!(Severity::Error.is_blocking());
    }

    #[test]
    fn warning_severity_is_blocking() {
        assert!(Severity::Warning.is_blocking());
    }

    #[test]
    fn info_severity_is_not_blocking() {
        assert!(!Severity::Info.is_blocking());
    }

    #[test]
    fn findings_with_only_info_do_not_block() {
        let findings = [Finding {
            file: "scripts/setup.py".into(),
            tool: "bandit".into(),
            rule_id: "B101".into(),
            severity: Severity::Info,
            message: "assert used".into(),
        }];
        assert!(!findings.iter().any(|f| f.severity.is_blocking()));
    }

    #[test]
    fn findings_with_warning_block() {
        let findings = [Finding {
            file: "scripts/setup.py".into(),
            tool: "bandit".into(),
            rule_id: "B601".into(),
            severity: Severity::Warning,
            message: "shell injection risk".into(),
        }];
        assert!(findings.iter().any(|f| f.severity.is_blocking()));
    }

    /// Mock analyzer that returns a canned list of findings.
    struct MockAnalyzer(Vec<Finding>);
    impl Analyzer for MockAnalyzer {
        fn analyze(&self, _file: &Path) -> Result<Vec<Finding>, StaticAnalysisError> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn mock_analyzer_returns_findings() {
        let analyzer = MockAnalyzer(vec![Finding {
            file: "scripts/foo.py".into(),
            tool: "mock".into(),
            rule_id: "MOCK001".into(),
            severity: Severity::Warning,
            message: "mock finding".into(),
        }]);
        let findings = analyzer.analyze(Path::new("scripts/foo.py")).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warning);
    }
}
