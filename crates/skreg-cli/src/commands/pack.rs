//! `skreg pack` — create a .skill tarball from the current directory.

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
    skreg_pack::pack::pack_directory(source_dir, tmp.path())?;
    let bytes = std::fs::read(tmp.path())?;
    let sha256 = hex::encode(Sha256::digest(&bytes));

    // Inject sha256 into manifest and repack
    manifest["sha256"] = serde_json::Value::String(sha256);
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    skreg_pack::pack::pack_directory(source_dir, output_path)?;

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
/// If `key_override` and `cert_override` are both `Some`, those key/cert files
/// are used; otherwise publisher keys are auto-generated or loaded from
/// `~/.skreg/keys/`.
///
/// # Errors
///
/// Returns an error if `manifest.json` is missing or any packing step fails.
pub fn run_pack(
    dir: &Path,
    key_override: Option<&Path>,
    cert_override: Option<&Path>,
) -> Result<()> {
    let manifest_path = dir.join("manifest.json");
    let manifest_raw = std::fs::read_to_string(&manifest_path)?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_raw)?;
    let name = manifest["name"].as_str().unwrap_or("skill");
    let namespace = manifest["namespace"].as_str().unwrap_or("unknown");
    let version = manifest["version"].as_str().unwrap_or("0.0.0");
    let output = dir.join(format!("{name}-{version}.skill"));

    // Load or generate publisher keys
    let keys = if let (Some(k), Some(c)) = (key_override, cert_override) {
        crate::keys::load_explicit_keys(k, c)?
    } else {
        let kdir = crate::keys::keys_dir()?;
        crate::keys::ensure_keys_exist(&kdir, namespace)?
    };

    // First pass: pack to a temp file to compute sha256
    let tmp = tempfile::NamedTempFile::new()?;
    skreg_pack::pack::pack_directory(dir, tmp.path())?;
    let bytes = std::fs::read(tmp.path())?;
    let sha256_hex = hex::encode(Sha256::digest(&bytes));

    // Sign the digest
    let publisher_sig_hex = crate::keys::pss_sign_digest(&keys.private_key_pem, &sha256_hex)?;

    // Inject sha256, publisher_sig_hex, cert_chain_pem into manifest and repack
    let mut manifest_signed: serde_json::Value = serde_json::from_str(&manifest_raw)?;
    manifest_signed["sha256"] = serde_json::Value::String(sha256_hex);
    manifest_signed["publisher_sig_hex"] = serde_json::Value::String(publisher_sig_hex);
    manifest_signed["cert_chain_pem"] = serde_json::Value::Array(
        keys.cert_chain_pem
            .into_iter()
            .map(serde_json::Value::String)
            .collect(),
    );

    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest_signed)?,
    )?;
    skreg_pack::pack::pack_directory(dir, &output)?;

    // Restore original manifest
    std::fs::write(&manifest_path, &manifest_raw)?;

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

    #[test]
    fn run_pack_produces_signed_skill_file() {
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

        // Use a temp keys dir so we don't touch ~
        let keys_dir = tempdir().unwrap();
        let keys = crate::keys::ensure_keys_exist(keys_dir.path(), "acme").unwrap();

        // Write keys to temp files and use explicit overrides
        let key_path = keys_dir.path().join("test.key");
        let cert_path = keys_dir.path().join("test.crt");
        fs::write(&key_path, &keys.private_key_pem).unwrap();
        fs::write(&cert_path, &keys.cert_pem).unwrap();

        run_pack(dir.path(), Some(&key_path), Some(&cert_path)).unwrap();
        let out = dir.path().join("test-1.0.0.skill");
        assert!(out.exists());
        assert!(out.metadata().unwrap().len() > 0);

        // Manifest should be restored (no sha256 field)
        let restored: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.path().join("manifest.json")).unwrap())
                .unwrap();
        assert!(restored.get("sha256").is_none());
        assert!(restored.get("publisher_sig_hex").is_none());
    }
}
