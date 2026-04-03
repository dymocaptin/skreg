use std::fs;

use semver::Version;
use skreg_core::manifest::Manifest;
use skreg_core::types::{Namespace, PackageName, Sha256Digest};
use skreg_pack::pack::pack_with_manifest;
use tempfile::TempDir;

fn minimal_skill_dir(dir: &TempDir) {
    fs::write(
        dir.path().join("SKILL.md"),
        "---\nname: test\ndescription: hello\nmetadata:\n  version: \"1.0.0\"\n---\n",
    )
    .unwrap();
}

fn stub_manifest() -> Manifest {
    Manifest {
        namespace: Namespace::new("acme").unwrap(),
        name: PackageName::new("test").unwrap(),
        version: Version::parse("1.0.0").unwrap(),
        description: "hello".to_owned(),
        category: None,
        sha256: Sha256Digest::from_hex(&"a".repeat(64)).unwrap(),
        cert_chain_pem: vec![],
        publisher_sig_hex: None,
    }
}

#[test]
fn pack_with_manifest_creates_skill_file() {
    let dir = TempDir::new().unwrap();
    minimal_skill_dir(&dir);
    let out = dir.path().join("out.skill");
    pack_with_manifest(dir.path(), &stub_manifest(), &out).unwrap();
    assert!(out.exists());
    assert!(out.metadata().unwrap().len() > 0);
}

#[test]
fn pack_with_manifest_does_not_modify_source_dir() {
    let dir = TempDir::new().unwrap();
    minimal_skill_dir(&dir);
    let out = dir.path().join("out.skill");
    pack_with_manifest(dir.path(), &stub_manifest(), &out).unwrap();
    assert!(!dir.path().join("manifest.json").exists());
}

#[test]
fn pack_with_manifest_tarball_contains_manifest_entry() {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let dir = TempDir::new().unwrap();
    minimal_skill_dir(&dir);
    let out = dir.path().join("out.skill");
    pack_with_manifest(dir.path(), &stub_manifest(), &out).unwrap();

    let bytes = fs::read(&out).unwrap();
    let gz = GzDecoder::new(bytes.as_slice());
    let mut archive = Archive::new(gz);
    let has_manifest = archive
        .entries()
        .unwrap()
        .any(|e| e.unwrap().path().unwrap().to_str() == Some("manifest.json"));
    assert!(has_manifest, "tarball must contain manifest.json entry");
}

#[test]
fn pack_with_manifest_missing_skill_md_fails() {
    let dir = TempDir::new().unwrap();
    // No SKILL.md
    let out = dir.path().join("out.skill");
    let result = pack_with_manifest(dir.path(), &stub_manifest(), &out);
    assert!(result.is_err());
}
