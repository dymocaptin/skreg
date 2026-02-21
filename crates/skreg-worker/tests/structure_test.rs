use std::fs;

use skreg_worker::stages::structure::check_structure;
use tempfile::TempDir;

fn make_valid_dir(dir: &TempDir) {
    fs::write(dir.path().join("SKILL.md"), "---\nname: test\ndescription: hello\n---\n").unwrap();
    fs::write(dir.path().join("manifest.json"), r#"{"name":"test"}"#).unwrap();
}

#[test]
fn valid_directory_passes_structure_checks() {
    let dir = TempDir::new().unwrap();
    make_valid_dir(&dir);
    let result = check_structure(dir.path());
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn missing_skill_md_fails() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("manifest.json"), "{}").unwrap();
    assert!(check_structure(dir.path()).is_err());
}

#[test]
fn missing_manifest_fails() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("SKILL.md"), "---\n---\n").unwrap();
    assert!(check_structure(dir.path()).is_err());
}

#[test]
fn oversized_tarball_fails() {
    let dir = TempDir::new().unwrap();
    make_valid_dir(&dir);
    // Write a 6MB file to exceed the 5MB limit
    let big = vec![0u8; 6 * 1024 * 1024];
    fs::write(dir.path().join("big.md"), big).unwrap();
    assert!(check_structure(dir.path()).is_err());
}
