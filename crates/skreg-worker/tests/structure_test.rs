use std::fs;

use skreg_worker::stages::structure::check_structure;
use tempfile::TempDir;

fn make_valid_dir(dir: &TempDir) {
    fs::write(
        dir.path().join("SKILL.md"),
        "---\nname: test\ndescription: A valid skill description here\nmetadata:\n  version: \"1.0.0\"\n---\n",
    )
    .unwrap();
}

#[test]
fn valid_directory_passes_structure_checks() {
    let dir = TempDir::new().unwrap();
    make_valid_dir(&dir);
    assert!(check_structure(dir.path()).is_ok());
}

#[test]
fn manifest_json_present_in_package_is_allowed() {
    // manifest.json is still present in wire-format tarballs; it must not be rejected.
    let dir = TempDir::new().unwrap();
    make_valid_dir(&dir);
    fs::write(dir.path().join("manifest.json"), r#"{"name":"test"}"#).unwrap();
    assert!(check_structure(dir.path()).is_ok());
}

#[test]
fn missing_skill_md_fails() {
    let dir = TempDir::new().unwrap();
    assert!(check_structure(dir.path()).is_err());
}

#[test]
fn missing_metadata_version_fails() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("SKILL.md"),
        "---\nname: test\ndescription: A valid skill description here\n---\n",
    )
    .unwrap();
    let err = check_structure(dir.path()).unwrap_err();
    assert!(err.to_string().contains("metadata.version"), "got: {err}");
}

#[test]
fn invalid_semver_version_fails() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("SKILL.md"),
        "---\nname: test\ndescription: A valid skill description here\nmetadata:\n  version: not-semver\n---\n",
    )
    .unwrap();
    let err = check_structure(dir.path()).unwrap_err();
    assert!(err.to_string().contains("metadata.version"), "got: {err}");
}

#[test]
fn oversized_tarball_fails() {
    let dir = TempDir::new().unwrap();
    make_valid_dir(&dir);
    fs::create_dir(dir.path().join("assets")).unwrap();
    let big = vec![0u8; 6 * 1024 * 1024];
    fs::write(dir.path().join("assets/big.md"), big).unwrap();
    assert!(check_structure(dir.path()).is_err());
}
