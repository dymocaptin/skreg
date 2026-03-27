//! Integration tests for the static analysis stage.
//!
//! Requires the full worker environment: shellcheck, bandit, semgrep, nsjail,
//! python.wasm, quickjs.wasm, and a running tracee DaemonSet.
//!
//! Run with: `cargo test --features integration -p skreg-worker`
//! Skipped on GitHub-hosted runners (no worker-pool environment).

#![cfg(feature = "integration")]

use std::path::Path;

use skreg_worker::stages::static_analysis::{pass1, pass2, startup, Severity};

fn rules_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("rules")
}

/// Verify all required tools are present. Fails hard if any are missing.
#[test]
fn required_tools_are_present() {
    startup::check_tools().expect(
        "All required tools (shellcheck, bandit, semgrep, nsjail) must be present in integration environment"
    );
}

#[test]
fn tracee_socket_is_present() {
    startup::check_tracee_socket().expect(
        "tracee socket must be present at /var/run/tracee/tracee.sock in integration environment",
    );
}

#[test]
fn yara_rules_compile_in_integration_env() {
    startup::check_yara_rules(&rules_dir()).expect("YARA rules must compile");
}

#[test]
fn reverse_shell_python_fixture_blocked_by_pass1() {
    let rules = startup::check_yara_rules(&rules_dir()).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bad_reverse_shell.py");
    let findings = pass1::run_pass1(&fixture, Path::new("scripts/bad.py"), &rules).unwrap();
    assert!(
        findings.iter().any(|f| f.severity.is_blocking()),
        "reverse shell fixture must produce blocking findings in pass1: {findings:?}"
    );
}

#[test]
fn clean_python_script_passes_bandit() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clean_script.py");
    let findings = pass2::run_pass2_file(&fixture, Path::new("scripts/clean.py"), true).unwrap();
    assert!(
        !findings.iter().any(|f| f.severity == Severity::Error),
        "clean script should not produce error-severity bandit findings: {findings:?}"
    );
}
