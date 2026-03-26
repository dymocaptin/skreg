// crates/skreg-worker/tests/corpus_integration_test.rs
//! Full-pipeline corpus integration tests.
//!
//! Requires the integration worker environment:
//!   shellcheck, bandit, semgrep, nsjail, tracee socket present.
//!
//! Run with: cargo test --features integration -p skreg-worker corpus
#![cfg(feature = "integration")]
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use skreg_worker::stages::{
    content::check_content,
    static_analysis::{pass1, run_static_analysis, Finding},
    structure::check_structure,
};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn rules_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("rules")
}

fn compiled_rules() -> pass1::CompiledRules {
    pass1::compile_rules(&rules_dir()).expect("YARA rules must compile in integration env")
}

/// Run all three stages against `dir`. Panics if structure or content errors
/// (fixture is misconfigured). Returns static analysis findings.
fn run_all_stages(dir: &Path) -> Vec<Finding> {
    check_structure(dir).expect("structure stage errored — check fixture setup");
    check_content(dir).expect("content stage errored — check fixture setup");
    let rules = compiled_rules();
    let tracee = Path::new("/var/run/tracee/tracee.sock").exists();
    run_static_analysis(dir, &rules, tracee)
        .expect("static_analysis infrastructure error — check tool availability")
}

/// Assert no blocking findings for a clean fixture.
fn assert_no_blocking(dir: &TempDir) {
    let findings = run_all_stages(dir.path());
    let blocking: Vec<_> = findings
        .iter()
        .filter(|f| f.severity.is_blocking())
        .collect();
    assert!(
        blocking.is_empty(),
        "clean fixture produced unexpected blocking findings: {blocking:?}"
    );
}

/// Assert the pipeline produces at least one blocking finding matching `rule_id`.
fn assert_blocked_by(dir: &TempDir, rule_id: &str) {
    let findings = run_all_stages(dir.path());
    assert!(
        findings
            .iter()
            .any(|f| f.rule_id == rule_id && f.severity.is_blocking()),
        "expected blocking finding with rule_id={rule_id:?}, got: {findings:?}"
    );
}

/// Assert the pipeline produces at least one blocking finding where
/// `rule_id` starts with `prefix`. Used for magic-byte findings whose
/// rule_id includes the file extension (e.g. "binary_disguised_elf").
fn assert_blocked_by_prefix(dir: &TempDir, prefix: &str) {
    let findings = run_all_stages(dir.path());
    assert!(
        findings
            .iter()
            .any(|f| f.rule_id.starts_with(prefix) && f.severity.is_blocking()),
        "expected blocking finding with rule_id starting with {prefix:?}, got: {findings:?}"
    );
}

/// Assert the hook command scanner produces at least one blocking finding.
fn assert_hook_blocked_by(dir: &TempDir, rule_id: &str) {
    let findings = run_all_stages(dir.path());
    assert!(
        findings.iter().any(|f| f.rule_id == rule_id
            && f.file.contains("SKILL.md")
            && f.severity.is_blocking()),
        "expected blocking hook finding with rule_id={rule_id:?}, got: {findings:?}"
    );
}

// ── Fixture builders ─────────────────────────────────────────────────────────

/// Write a valid base package (passes all stages on its own).
fn make_valid_base(dir: &Path) {
    fs::write(
        dir.join("SKILL.md"),
        "---\nname: test-skill\ndescription: A test fixture package for integration testing.\n---\n# Test\nThis is a test fixture for integration testing.\n",
    )
    .unwrap();
    fs::write(
        dir.join("manifest.json"),
        r#"{"name":"test-skill","version":"1.0.0","description":"A test fixture package for integration testing."}"#,
    )
    .unwrap();
}

/// Write a malicious base package. The SKILL.md is valid so structure and
/// content stages pass; malicious content goes in scripts/ or hooks.
fn make_malicious_base(dir: &Path) {
    fs::write(
        dir.join("SKILL.md"),
        "---\nname: test-skill\ndescription: FAKE MALICIOUS TEST FIXTURE — DO NOT PUBLISH.\n---\n> **FAKE TEST FIXTURE — NOT A REAL SKILL — DO NOT PUBLISH**\n",
    )
    .unwrap();
    fs::write(
        dir.join("manifest.json"),
        r#"{"name":"test-skill","version":"1.0.0","description":"FAKE MALICIOUS TEST FIXTURE — DO NOT PUBLISH.","test-only":true}"#,
    )
    .unwrap();
}

/// Write `content` to `scripts/<filename>` inside `dir`.
fn add_script(dir: &Path, filename: &str, content: &[u8]) {
    fs::create_dir_all(dir.join("scripts")).unwrap();
    fs::write(dir.join("scripts").join(filename), content).unwrap();
}

/// Write a SKILL.md with a hooks block containing `command`.
fn make_malicious_hook(dir: &Path, command: &str) {
    let skill_md = format!(
        "---\nname: test-skill\ndescription: FAKE MALICIOUS TEST FIXTURE — DO NOT PUBLISH.\nhooks:\n  PreToolUse:\n    - matcher: Bash\n      hooks:\n        - type: command\n          command: \"{command}\"\n---\n> **FAKE TEST FIXTURE — NOT A REAL SKILL — DO NOT PUBLISH**\n"
    );
    fs::write(dir.join("SKILL.md"), skill_md).unwrap();
    fs::write(
        dir.join("manifest.json"),
        r#"{"name":"test-skill","version":"1.0.0","description":"FAKE MALICIOUS TEST FIXTURE — DO NOT PUBLISH.","test-only":true}"#,
    )
    .unwrap();
}

// ── Clean fixtures ────────────────────────────────────────────────────────────

#[test]
fn clean_minimal_passes() {
    let dir = TempDir::new().unwrap();
    make_valid_base(dir.path());
    assert_no_blocking(&dir);
}

#[test]
fn clean_with_references_passes() {
    let dir = TempDir::new().unwrap();
    make_valid_base(dir.path());
    fs::create_dir(dir.path().join("references")).unwrap();
    fs::write(
        dir.path().join("references/setup.md"),
        "# Setup Guide\nInstall dependencies using your system package manager.\nConfigure your environment by editing the provided config file.\n",
    )
    .unwrap();
    assert_no_blocking(&dir);
}

#[test]
fn clean_with_python_script_passes() {
    let dir = TempDir::new().unwrap();
    make_valid_base(dir.path());
    add_script(
        dir.path(),
        "check.py",
        b"import sys\n\ndef check_version():\n    \"\"\"Verify Python version is 3.8+.\"\"\"\n    if sys.version_info < (3, 8):\n        print('Python 3.8+ required')\n        return False\n    return True\n\nif __name__ == '__main__':\n    if check_version():\n        print('Requirements met')\n",
    );
    assert_no_blocking(&dir);
}

#[test]
fn clean_with_shell_script_passes() {
    let dir = TempDir::new().unwrap();
    make_valid_base(dir.path());
    add_script(
        dir.path(),
        "verify.sh",
        b"#!/bin/sh\nset -e\necho 'Checking environment...'\nif [ -z \"$HOME\" ]; then\n    echo 'HOME is not set'\n    exit 1\nfi\necho 'Environment looks good.'\n",
    );
    assert_no_blocking(&dir);
}

#[test]
fn clean_with_assets_passes() {
    let dir = TempDir::new().unwrap();
    make_valid_base(dir.path());
    fs::create_dir(dir.path().join("assets")).unwrap();
    // Minimal valid PNG header
    fs::write(
        dir.path().join("assets/diagram.png"),
        b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02",
    )
    .unwrap();
    fs::write(
        dir.path().join("assets/notes.txt"),
        b"Architecture notes.\n",
    )
    .unwrap();
    assert_no_blocking(&dir);
}

#[test]
fn clean_with_benign_hook_passes() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("SKILL.md"),
        "---\nname: test-skill\ndescription: A test fixture package for integration testing.\nhooks:\n  PreToolUse:\n    - matcher: Bash\n      hooks:\n        - type: command\n          command: \"bash scripts/verify.sh\"\n---\n# Test\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("manifest.json"),
        r#"{"name":"test-skill","version":"1.0.0","description":"A test fixture package for integration testing."}"#,
    )
    .unwrap();
    add_script(dir.path(), "verify.sh", b"#!/bin/sh\necho 'hook ran'\n");
    assert_no_blocking(&dir);
}

#[test]
fn clean_full_package_passes() {
    let dir = TempDir::new().unwrap();
    make_valid_base(dir.path());
    fs::write(dir.path().join("LICENSE"), b"Apache License 2.0\n").unwrap();
    fs::create_dir(dir.path().join("references")).unwrap();
    fs::write(
        dir.path().join("references/guide.md"),
        b"# Guide\nFollow these steps to configure the tool.\n",
    )
    .unwrap();
    fs::create_dir(dir.path().join("assets")).unwrap();
    fs::write(
        dir.path().join("assets/overview.txt"),
        b"Overview of the skill.\n",
    )
    .unwrap();
    add_script(dir.path(), "setup.py", b"print('setup complete')\n");
    add_script(dir.path(), "check.sh", b"#!/bin/sh\necho 'ok'\n");
    assert_no_blocking(&dir);
}
