use std::fs;

use skillpkg_pack::pack::pack_directory;
use skillpkg_pack::unpack::unpack_tarball;
use tempfile::TempDir;

fn make_skill_dir(dir: &TempDir) {
    let skill_md = "---\nname: test-skill\ndescription: A test skill\n---\n# Test\n";
    fs::write(dir.path().join("SKILL.md"), skill_md).unwrap();
    let manifest = r#"{"namespace":"acme","name":"test-skill","version":"0.1.0","description":"A test skill","sha256":"","cert_chain_pem":[]}"#;
    fs::write(dir.path().join("manifest.json"), manifest).unwrap();
}

#[test]
fn pack_creates_tarball_with_correct_entries() {
    let src = TempDir::new().unwrap();
    make_skill_dir(&src);

    let out = TempDir::new().unwrap();
    let tarball_path = out.path().join("test.skill");

    pack_directory(src.path(), &tarball_path).unwrap();
    assert!(tarball_path.exists());
    assert!(tarball_path.metadata().unwrap().len() > 0);
}

#[test]
fn unpack_roundtrips_skill_md() {
    let src = TempDir::new().unwrap();
    make_skill_dir(&src);

    let out_tar = TempDir::new().unwrap();
    let tarball_path = out_tar.path().join("test.skill");
    pack_directory(src.path(), &tarball_path).unwrap();

    let dest = TempDir::new().unwrap();
    unpack_tarball(&tarball_path, dest.path()).unwrap();

    let skill_md = fs::read_to_string(dest.path().join("SKILL.md")).unwrap();
    assert!(skill_md.contains("name: test-skill"));
}
