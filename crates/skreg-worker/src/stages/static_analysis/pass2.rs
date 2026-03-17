//! Pass 2: deep static analysis for `scripts/` files.

use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;

use super::{Analyzer, Finding, Severity, StaticAnalysisError};

/// Per-subprocess timeout.
#[allow(dead_code)]
const TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

// ── Severity normalisation helpers ───────────────────────────────────────

/// Normalise a `shellcheck` severity string to [`Severity`].
#[must_use]
pub fn shellcheck_severity(level: &str) -> Severity {
    match level {
        "error" => Severity::Error,
        "warning" => Severity::Warning,
        _ => Severity::Info, // "info" | "style"
    }
}

/// Normalise `bandit` severity + confidence to [`Severity`].
#[must_use]
pub fn bandit_severity(severity: &str, confidence: &str) -> Severity {
    match (
        severity.to_uppercase().as_str(),
        confidence.to_uppercase().as_str(),
    ) {
        ("HIGH", _) => Severity::Error,
        ("MEDIUM", _) | ("LOW", "HIGH") => Severity::Warning,
        _ => Severity::Info,
    }
}

/// Normalise a `semgrep` severity string to [`Severity`].
#[must_use]
pub fn semgrep_severity(level: &str) -> Severity {
    match level.to_uppercase().as_str() {
        "ERROR" => Severity::Error,
        "WARNING" => Severity::Warning,
        _ => Severity::Info,
    }
}

// ── Analyzer implementations ──────────────────────────────────────────────

/// Runs `shellcheck --format=json` on bash/sh scripts.
pub struct ShellcheckAnalyzer;

impl Analyzer for ShellcheckAnalyzer {
    fn analyze(&self, file: &Path) -> Result<Vec<Finding>, StaticAnalysisError> {
        let output = Command::new("shellcheck")
            .args(["--format=json", &file.to_string_lossy()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| StaticAnalysisError::ToolError {
                tool: "shellcheck".into(),
                reason: e.to_string(),
            })?;

        let json: Value = serde_json::from_slice(&output.stdout).unwrap_or(Value::Array(vec![]));
        let mut findings = Vec::new();

        if let Some(items) = json.as_array() {
            for item in items {
                let level = item["level"].as_str().unwrap_or("info");
                let code = item["code"].as_u64().unwrap_or(0).to_string();
                let message = item["message"].as_str().unwrap_or("").to_owned();
                findings.push(Finding {
                    file: file.to_string_lossy().into_owned(),
                    tool: "shellcheck".into(),
                    rule_id: format!("SC{code}"),
                    severity: shellcheck_severity(level),
                    message,
                });
            }
        }
        Ok(findings)
    }
}

/// Runs `bandit -f json` on Python scripts.
pub struct BanditAnalyzer;

impl Analyzer for BanditAnalyzer {
    fn analyze(&self, file: &Path) -> Result<Vec<Finding>, StaticAnalysisError> {
        let output = Command::new("bandit")
            .args(["-f", "json", &file.to_string_lossy()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| StaticAnalysisError::ToolError {
                tool: "bandit".into(),
                reason: e.to_string(),
            })?;

        let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_default();
        let mut findings = Vec::new();

        if let Some(results) = json["results"].as_array() {
            for item in results {
                let sev = item["issue_severity"].as_str().unwrap_or("LOW");
                let conf = item["issue_confidence"].as_str().unwrap_or("LOW");
                let rule_id = item["test_id"].as_str().unwrap_or("UNKNOWN").to_owned();
                let message = item["issue_text"].as_str().unwrap_or("").to_owned();
                findings.push(Finding {
                    file: file.to_string_lossy().into_owned(),
                    tool: "bandit".into(),
                    rule_id,
                    severity: bandit_severity(sev, conf),
                    message,
                });
            }
        }
        Ok(findings)
    }
}

/// Runs `semgrep --json` on JS/TS and Ruby scripts.
pub struct SemgrepAnalyzer {
    /// Semgrep config/ruleset identifier (e.g. `"p/javascript"`).
    pub config: String,
}

impl Analyzer for SemgrepAnalyzer {
    fn analyze(&self, file: &Path) -> Result<Vec<Finding>, StaticAnalysisError> {
        let output = Command::new("semgrep")
            .args(["--config", &self.config, "--json", &file.to_string_lossy()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| StaticAnalysisError::ToolError {
                tool: "semgrep".into(),
                reason: e.to_string(),
            })?;

        let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_default();
        let mut findings = Vec::new();

        if let Some(results) = json["results"].as_array() {
            for item in results {
                let sev = item["extra"]["severity"].as_str().unwrap_or("INFO");
                let rule_id = item["check_id"].as_str().unwrap_or("UNKNOWN").to_owned();
                let message = item["extra"]["message"].as_str().unwrap_or("").to_owned();
                findings.push(Finding {
                    file: file.to_string_lossy().into_owned(),
                    tool: "semgrep".into(),
                    rule_id,
                    severity: semgrep_severity(sev),
                    message,
                });
            }
        }
        Ok(findings)
    }
}

// ── Sandbox stub implementations ─────────────────────────────────────────
//
// NOTE: The WASM sandbox (wasmtime + custom WasiCtx) and nsjail execution
// are required by the spec but are NOT fully implemented here. These stubs
// exist so the dispatch logic and Finding types are in place. Full
// implementation requires:
//   - WasmSandboxAnalyzer: wasmtime + pre-compiled python.wasm/quickjs.wasm
//     with a custom WasiCtx that records denied WASI calls as Warning findings
//   - NsjailAnalyzer: nsjail subprocess with seccomp-bpf + tracee eBPF event
//     stream parsing
// These are tracked as follow-up tasks to this branch.

/// Stub WASM sandbox analyzer for Python and JS/TS scripts.
/// Full implementation: wasmtime + custom `WasiCtx` that intercepts WASI calls.
pub struct WasmSandboxAnalyzer;

impl Analyzer for WasmSandboxAnalyzer {
    fn analyze(&self, _file: &Path) -> Result<Vec<Finding>, StaticAnalysisError> {
        // TODO: run script inside pre-compiled python.wasm or quickjs.wasm
        // with a custom WasiCtx that records denied WASI capability attempts
        // as Warning-severity findings.
        Ok(vec![])
    }
}

/// Stub nsjail sandbox analyzer for Bash and Ruby scripts.
/// Full implementation: nsjail subprocess + tracee eBPF event stream.
pub struct NsjailAnalyzer;

impl Analyzer for NsjailAnalyzer {
    fn analyze(&self, _file: &Path) -> Result<Vec<Finding>, StaticAnalysisError> {
        // TODO: execute script inside nsjail with:
        //   - network namespace isolation
        //   - read-only tmpfs
        //   - seccomp-bpf whitelist
        //   - resource limits
        // Collect tracee eBPF events via /var/run/tracee/tracee.sock and
        // normalise suspicious syscall patterns into Warning findings.
        Ok(vec![])
    }
}

/// Dispatch Pass 2 analysis for a single file under `scripts/`.
///
/// Runs both static analysis and sandbox execution (where implemented).
/// Extensions not in `SCRIPT_EXTENSIONS` are silently ignored (structure
/// stage guards this earlier).
///
/// # Errors
///
/// Returns `Err(StaticAnalysisError::MissingComponent)` when `tracee_available`
/// is `false` and the file extension requires sandbox execution (`sh`, `bash`,
/// `rb`). Returns `Err(StaticAnalysisError::ToolError)` if a subprocess tool
/// fails to launch.
pub fn run_pass2_file(
    file: &Path,
    rel: &Path,
    tracee_available: bool,
) -> Result<Vec<Finding>, StaticAnalysisError> {
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
    let mut findings = Vec::new();
    match ext {
        "py" => {
            findings.extend(BanditAnalyzer.analyze(file)?);
            findings.extend(WasmSandboxAnalyzer.analyze(file)?);
        }
        "js" | "ts" => {
            findings.extend(
                SemgrepAnalyzer {
                    config: "p/javascript".into(),
                }
                .analyze(file)?,
            );
            findings.extend(WasmSandboxAnalyzer.analyze(file)?);
        }
        "rb" => {
            if !tracee_available {
                return Err(StaticAnalysisError::MissingComponent(
                    "tracee socket absent — cannot process Ruby scripts safely".into(),
                ));
            }
            findings.extend(
                SemgrepAnalyzer {
                    config: "p/ruby-security-audit".into(),
                }
                .analyze(file)?,
            );
            findings.extend(NsjailAnalyzer.analyze(file)?);
        }
        "sh" | "bash" => {
            if !tracee_available {
                return Err(StaticAnalysisError::MissingComponent(
                    "tracee socket absent — cannot process bash scripts safely".into(),
                ));
            }
            findings.extend(ShellcheckAnalyzer.analyze(file)?);
            findings.extend(NsjailAnalyzer.analyze(file)?);
        }
        _ => {}
    }
    let _ = rel;
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // ── shellcheck severity normalisation ─────────────────────────────────

    #[test]
    fn shellcheck_error_maps_to_error() {
        assert_eq!(shellcheck_severity("error"), Severity::Error);
    }

    #[test]
    fn shellcheck_warning_maps_to_warning() {
        assert_eq!(shellcheck_severity("warning"), Severity::Warning);
    }

    #[test]
    fn shellcheck_info_maps_to_info() {
        assert_eq!(shellcheck_severity("info"), Severity::Info);
    }

    #[test]
    fn shellcheck_style_maps_to_info() {
        assert_eq!(shellcheck_severity("style"), Severity::Info);
    }

    // ── bandit severity normalisation ─────────────────────────────────────

    #[test]
    fn bandit_high_any_confidence_maps_to_error() {
        assert_eq!(bandit_severity("HIGH", "LOW"), Severity::Error);
        assert_eq!(bandit_severity("HIGH", "MEDIUM"), Severity::Error);
        assert_eq!(bandit_severity("HIGH", "HIGH"), Severity::Error);
    }

    #[test]
    fn bandit_medium_any_confidence_maps_to_warning() {
        assert_eq!(bandit_severity("MEDIUM", "LOW"), Severity::Warning);
        assert_eq!(bandit_severity("MEDIUM", "MEDIUM"), Severity::Warning);
        assert_eq!(bandit_severity("MEDIUM", "HIGH"), Severity::Warning);
    }

    #[test]
    fn bandit_low_high_confidence_maps_to_warning() {
        assert_eq!(bandit_severity("LOW", "HIGH"), Severity::Warning);
    }

    #[test]
    fn bandit_low_medium_confidence_maps_to_info() {
        assert_eq!(bandit_severity("LOW", "MEDIUM"), Severity::Info);
    }

    #[test]
    fn bandit_low_low_confidence_maps_to_info() {
        assert_eq!(bandit_severity("LOW", "LOW"), Severity::Info);
    }

    // ── semgrep severity normalisation ────────────────────────────────────

    #[test]
    fn semgrep_error_maps_to_error() {
        assert_eq!(semgrep_severity("ERROR"), Severity::Error);
    }

    #[test]
    fn semgrep_warning_maps_to_warning() {
        assert_eq!(semgrep_severity("WARNING"), Severity::Warning);
    }

    #[test]
    fn semgrep_info_maps_to_info() {
        assert_eq!(semgrep_severity("INFO"), Severity::Info);
    }

    // ── dispatch routing ──────────────────────────────────────────────────

    #[test]
    fn bash_without_tracee_returns_missing_component_error() {
        // Tracee unavailable → bash scripts must be rejected, not silently skipped.
        use std::path::PathBuf;
        let err = run_pass2_file(
            &PathBuf::from("/tmp/test.sh"),
            Path::new("scripts/test.sh"),
            false, // tracee_available = false
        )
        .unwrap_err();
        assert!(
            matches!(err, StaticAnalysisError::MissingComponent(_)),
            "got: {err}"
        );
    }

    #[test]
    fn unknown_extension_returns_empty_findings() {
        use std::path::PathBuf;
        // A file that passed structure validation but has an extension we
        // don't dispatch to any tool (shouldn't happen in production, but
        // the dispatch layer should be safe).
        let findings = run_pass2_file(
            &PathBuf::from("/tmp/test.unknown"),
            Path::new("scripts/test.unknown"),
            true,
        )
        .unwrap();
        assert!(findings.is_empty());
    }
}
