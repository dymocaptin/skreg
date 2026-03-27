// crates/skreg-worker/src/bin/validate_packages.rs
//! One-off validation tool: run all three worker pipeline stages
//! against a directory of downloaded `.skill` tarballs.
//!
//! Usage:
//!   cargo run -p skreg-worker --bin validate-packages -- <dir>
//!   cargo run -p skreg-worker --bin validate-packages -- --skip-pass2 <dir>
//!
//! Environment variables:
//!   SKREG_YARA_RULES_DIR   Override the default YARA rules path
//!                          (default: crates/skreg-worker/rules)
//!   SKREG_TRACEE_SOCKET    Override the tracee eBPF socket path
//!                          (default: /var/run/tracee/tracee.sock)

use std::path::{Path, PathBuf};

use skreg_worker::stages::{
    content::check_content,
    static_analysis::{
        pass1::{self, compile_rules},
        run_static_analysis, run_yara_and_hooks, Finding,
    },
    structure::check_structure,
};

fn print_usage(prog: &str) {
    eprintln!("Usage: {prog} [--skip-pass2] <directory>");
    eprintln!();
    eprintln!("Validates all *.skill files in <directory> against the worker pipeline.");
    eprintln!("--skip-pass2  Skip subprocess tools (shellcheck, bandit, semgrep, tracee).");
    eprintln!("              Prints a warning. YARA + structure + content still run.");
    eprintln!();
    eprintln!("Environment:");
    eprintln!("  SKREG_YARA_RULES_DIR  Override default YARA rules path");
    eprintln!("  SKREG_TRACEE_SOCKET   Override tracee eBPF socket path");
}

fn tracee_socket_path() -> PathBuf {
    PathBuf::from(
        std::env::var("SKREG_TRACEE_SOCKET")
            .unwrap_or_else(|_| "/var/run/tracee/tracee.sock".into()),
    )
}

fn main() {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    let prog = &args[0];

    let (skip_pass2, dir_str) = match args.as_slice() {
        [_, flag, dir] if flag == "--skip-pass2" => (true, dir.as_str()),
        [_, dir] if !dir.starts_with('-') => (false, dir.as_str()),
        _ => {
            print_usage(prog);
            std::process::exit(2);
        }
    };

    if skip_pass2 {
        eprintln!(
            "WARNING: --skip-pass2 active — shellcheck/bandit/semgrep/tracee NOT run. \
             Structure, content, and YARA checks only."
        );
    }

    let dir = PathBuf::from(dir_str);
    if !dir.is_dir() {
        eprintln!("Error: '{}' is not a directory", dir.display());
        std::process::exit(2);
    }

    let rules_dir = PathBuf::from(
        std::env::var("SKREG_YARA_RULES_DIR")
            .unwrap_or_else(|_| "crates/skreg-worker/rules".into()),
    );

    let compiled_rules = match compile_rules(&rules_dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: YARA rule compilation failed: {e}");
            std::process::exit(1);
        }
    };

    let mut entries: Vec<_> = match std::fs::read_dir(&dir) {
        Ok(it) => it.filter_map(|e| e.ok()).collect(),
        Err(e) => {
            eprintln!("Error reading directory: {e}");
            std::process::exit(1);
        }
    };
    entries.retain(|e| e.path().extension().and_then(|x| x.to_str()) == Some("skill"));
    entries.sort_by_key(|e| e.file_name());

    if entries.is_empty() {
        eprintln!("No *.skill files found in '{}'", dir.display());
        std::process::exit(1);
    }

    let mut total: usize = 0;
    let mut passed: usize = 0;

    for entry in &entries {
        let path = entry.path();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        total += 1;

        match validate_one(&path, &compiled_rules, skip_pass2) {
            Ok(()) => {
                println!("PASS  {name}");
                passed += 1;
            }
            Err(msg) => {
                println!("FAIL  {name}  — {msg}");
            }
        }
    }

    println!();
    println!("{passed}/{total} passed");

    if passed < total {
        std::process::exit(1);
    }
}

fn validate_one(path: &Path, rules: &pass1::CompiledRules, skip_pass2: bool) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read error: {e}"))?;
    let tmp =
        skreg_pack::unpack::unpack_to_tempdir(&bytes).map_err(|e| format!("unpack failed: {e}"))?;

    check_structure(tmp.path()).map_err(|e| format!("stage 1 (structure): {e}"))?;
    check_content(tmp.path()).map_err(|e| format!("stage 2 (content): {e}"))?;

    // Stage 2.5: in skip_pass2 mode run Pass 1 (YARA) + hook scan directly
    // rather than run_static_analysis, which hard-errors on .sh/.rb scripts
    // when tracee is unavailable.
    let findings: Vec<Finding> = if skip_pass2 {
        run_yara_and_hooks(tmp.path(), rules).map_err(|e| format!("stage 2.5 (yara): {e}"))?
    } else {
        let tracee = tracee_socket_path().exists();
        run_static_analysis(tmp.path(), rules, tracee)
            .map_err(|e| format!("stage 2.5 (static analysis): {e}"))?
    };

    let blocking: Vec<_> = findings
        .iter()
        .filter(|f| f.severity.is_blocking())
        .collect();
    if !blocking.is_empty() {
        let desc = blocking
            .iter()
            .map(|f| format!("{} in {}", f.rule_id, f.file))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!("stage 2.5: blocking findings — {desc}"));
    }

    Ok(())
}
