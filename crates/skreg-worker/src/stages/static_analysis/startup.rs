//! Worker startup checks — verify all required tools and resources are present.

use std::path::Path;
use std::process::Command;

use super::StaticAnalysisError;

/// All tools that must be present and executable before processing any job.
#[allow(dead_code)]
const REQUIRED_TOOLS: &[&str] = &["shellcheck", "bandit", "semgrep", "nsjail"];

/// Path to the tracee Unix socket.
#[allow(dead_code)]
const TRACEE_SOCKET: &str = "/var/run/tracee/tracee.sock";

/// Check that all required subprocess tools are present and executable.
///
/// # Errors
///
/// Returns `StaticAnalysisError::MissingComponent` naming the first missing tool.
#[allow(dead_code)]
pub fn check_tools() -> Result<(), StaticAnalysisError> {
    for tool in REQUIRED_TOOLS {
        let ok = Command::new(tool)
            .arg("--version")
            .output()
            .map(|o| o.status.success() || !o.stdout.is_empty() || !o.stderr.is_empty())
            .unwrap_or(false);
        if !ok {
            return Err(StaticAnalysisError::MissingComponent(format!(
                "required tool not found or not executable: {tool}"
            )));
        }
    }
    Ok(())
}

/// Check that the tracee Unix socket is present.
///
/// # Errors
///
/// Returns `StaticAnalysisError::MissingComponent` if the socket is absent.
#[allow(dead_code)]
pub fn check_tracee_socket() -> Result<(), StaticAnalysisError> {
    if !Path::new(TRACEE_SOCKET).exists() {
        return Err(StaticAnalysisError::MissingComponent(format!(
            "tracee socket not found at {TRACEE_SOCKET}"
        )));
    }
    Ok(())
}

/// Check that WASM interpreter modules are present.
///
/// # Errors
///
/// Returns `StaticAnalysisError::MissingComponent` naming the first missing module.
pub fn check_wasm_modules(wasm_dir: &Path) -> Result<(), StaticAnalysisError> {
    for module in &["python.wasm", "quickjs.wasm"] {
        let path = wasm_dir.join(module);
        if !path.exists() {
            return Err(StaticAnalysisError::MissingComponent(format!(
                "WASM module not found: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

/// Compile YARA rules and record startup timing.
///
/// Logs a warning if compilation exceeds 10 seconds but does not abort.
///
/// # Errors
///
/// Returns `StaticAnalysisError::YaraCompilation` if any rule file fails to compile.
pub fn check_yara_rules(
    rules_dir: &Path,
) -> Result<super::pass1::CompiledRules, StaticAnalysisError> {
    use std::time::Instant;
    let start = Instant::now();
    let rules = super::pass1::compile_rules(rules_dir)?;
    let elapsed = start.elapsed();
    if elapsed.as_secs() > 10 {
        log::warn!(
            "YARA rule compilation took {:.1}s (budget: 10s)",
            elapsed.as_secs_f32()
        );
    }
    Ok(rules)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn missing_wasm_module_returns_error() {
        let dir = tempdir().unwrap();
        // python.wasm is absent
        let err = check_wasm_modules(dir.path()).unwrap_err();
        assert!(
            matches!(err, StaticAnalysisError::MissingComponent(ref msg) if msg.contains("python.wasm")),
            "got: {err}"
        );
    }

    #[test]
    fn present_wasm_modules_pass() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("python.wasm"), b"fake").unwrap();
        std::fs::write(dir.path().join("quickjs.wasm"), b"fake").unwrap();
        assert!(check_wasm_modules(dir.path()).is_ok());
    }

    #[test]
    fn absent_tracee_socket_returns_error() {
        // This test assumes /var/run/tracee/tracee.sock is absent in the
        // test environment (it will be on dev machines). If running in the
        // K8s worker-pool environment, this test will pass through instead
        // — it's specifically testing the error path.
        if Path::new(TRACEE_SOCKET).exists() {
            // Socket present — test the happy path instead.
            assert!(check_tracee_socket().is_ok());
        } else {
            let err = check_tracee_socket().unwrap_err();
            assert!(
                matches!(err, StaticAnalysisError::MissingComponent(_)),
                "got: {err}"
            );
        }
    }
}
