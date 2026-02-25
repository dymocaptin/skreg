//! `skillpkg pack` — create a .skill tarball from the current directory.

use std::path::Path;

use anyhow::Result;
use sha2::{Digest, Sha256};

/// Pack `source_dir` into `output_path`, injecting the tarball sha256 into `manifest.json`.
///
/// The sha256 is computed from a first-pass tarball, injected into the manifest,
/// then the tarball is re-packed with the updated manifest. The manifest is restored
/// to its original state afterwards so the source directory is not permanently modified.
///
/// # Errors
///
/// Returns an error if any file I/O or packing step fails.
pub fn pack_directory_with_sha(source_dir: &Path, output_path: &Path) -> Result<()> {
    let manifest_path = source_dir.join("manifest.json");
    let manifest_raw = std::fs::read_to_string(&manifest_path)?;
    let mut manifest: serde_json::Value = serde_json::from_str(&manifest_raw)?;

    // First pass: pack to a temp file to compute sha256
    let tmp = tempfile::NamedTempFile::new()?;
    skillpkg_pack::pack::pack_directory(source_dir, tmp.path())?;
    let bytes = std::fs::read(tmp.path())?;
    let sha256 = hex::encode(Sha256::digest(&bytes));

    // Inject sha256 into manifest and repack
    manifest["sha256"] = serde_json::Value::String(sha256);
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    skillpkg_pack::pack::pack_directory(source_dir, output_path)?;

    // Restore original manifest (without sha256 stamped in source)
    manifest
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("manifest.json is not a JSON object"))?
        .remove("sha256");
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Ok(())
}

/// Pack the current directory and write the `.skill` file next to `manifest.json`.
///
/// # Errors
///
/// Returns an error if `manifest.json` is missing or any packing step fails.
pub fn run_pack(dir: &Path) -> Result<()> {
    let manifest_raw = std::fs::read_to_string(dir.join("manifest.json"))?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_raw)?;
    let name = manifest["name"].as_str().unwrap_or("skill");
    let version = manifest["version"].as_str().unwrap_or("0.0.0");
    let output = dir.join(format!("{name}-{version}.skill"));

    pack_directory_with_sha(dir, &output)?;
    println!("packed: {}", output.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn pack_produces_skill_file() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("SKILL.md"),
            "---\nname: test\ndescription: a test skill that is long enough\n---\n# Test",
        )
        .unwrap();
        fs::write(
            dir.path().join("manifest.json"),
            r#"{"name":"test","namespace":"acme","version":"1.0.0","description":"a test skill that is long enough"}"#,
        ).unwrap();

        let out = dir.path().join("test-1.0.0.skill");
        pack_directory_with_sha(dir.path(), &out).unwrap();
        assert!(out.exists());
        assert!(out.metadata().unwrap().len() > 0);
    }
}
