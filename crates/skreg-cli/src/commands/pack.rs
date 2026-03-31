//! `skreg pack` — create a .skill tarball from the current directory.

use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use skreg_core::manifest::Manifest;
use skreg_core::types::{Namespace, PackageName, Sha256Digest};

/// Pack the current directory into a `.skill` tarball.
///
/// Reads metadata from `SKILL.md` frontmatter instead of requiring a
/// `manifest.json` in the source directory.  The source directory is never
/// modified; `manifest.json` is injected synthetically into the tarball.
///
/// If `key_override` and `cert_override` are both `Some`, those key/cert files
/// are used; otherwise publisher keys are auto-generated or loaded from
/// `~/.skreg/keys/`.
///
/// # Errors
///
/// Returns an error if `SKILL.md` is missing or malformed, any packing step
/// fails, or signing fails.
pub fn run_pack(
    dir: &Path,
    key_override: Option<&Path>,
    cert_override: Option<&Path>,
) -> Result<()> {
    // Step 1: Read SKILL.md frontmatter.
    let skill_md = dir.join("SKILL.md");
    let meta = crate::frontmatter::read_skill_metadata(&skill_md)?;

    // Step 2: Resolve namespace — from config, or fall back to "local".
    let namespace_str = resolve_namespace();

    // Load or generate publisher keys.
    let keys = if let (Some(k), Some(c)) = (key_override, cert_override) {
        crate::keys::load_explicit_keys(k, c)?
    } else {
        let kdir = crate::keys::keys_dir()?;
        crate::keys::ensure_keys_exist(&kdir, &namespace_str)?
    };

    let namespace = Namespace::new(&namespace_str)
        .with_context(|| format!("invalid namespace slug {namespace_str:?}"))?;
    let name =
        PackageName::new(&meta.name).with_context(|| format!("invalid name {:?}", meta.name))?;
    let version = meta.version.clone();
    let description = meta.description.clone();

    let output = dir.join(format!("{}-{}.skill", meta.name, meta.version));

    // Step 3: Build a draft Manifest with placeholder sha256 (64 zeros).
    let draft_sha256 =
        Sha256Digest::from_hex(&"0".repeat(64)).context("building placeholder sha256 digest")?;
    let draft = Manifest {
        namespace: namespace.clone(),
        name: name.clone(),
        version: version.clone(),
        description: description.clone(),
        category: None,
        sha256: draft_sha256,
        cert_chain_pem: vec![],
        publisher_sig_hex: None,
    };

    // Step 4: First pass — pack to a temp file to compute sha256.
    let tmp = tempfile::NamedTempFile::new().context("creating temporary file")?;
    skreg_pack::pack::pack_with_manifest(dir, &draft, tmp.path())?;
    let bytes = std::fs::read(tmp.path()).context("reading temporary tarball")?;
    let sha256_hex = hex::encode(Sha256::digest(&bytes));

    // Step 5: Sign the sha256 digest.
    let publisher_sig_hex = crate::keys::pss_sign_digest(&keys.private_key_pem, &sha256_hex)?;

    // Step 6: Build the signed Manifest.
    let real_sha256 =
        Sha256Digest::from_hex(&sha256_hex).context("building sha256 digest from hex")?;
    let signed = Manifest {
        namespace,
        name,
        version,
        description,
        category: None,
        sha256: real_sha256,
        cert_chain_pem: keys.cert_chain_pem,
        publisher_sig_hex: Some(publisher_sig_hex),
    };

    // Step 7: Second pass — write the final tarball with real manifest.
    skreg_pack::pack::pack_with_manifest(dir, &signed, &output)?;

    println!("packed: {}", output.display());
    Ok(())
}

/// Resolve the namespace from the CLI config, falling back to `"local"` when
/// the config file is absent (e.g. in tests or fresh installs).
fn resolve_namespace() -> String {
    let config_path = crate::config::default_config_path();
    crate::config::load_config(&config_path)
        .map_or_else(|_| "local".to_owned(), |cfg| cfg.namespace().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn make_skill_dir(dir: &std::path::Path, version: &str) {
        fs::write(
            dir.join("SKILL.md"),
            format!(
                "---\nname: test\ndescription: a test skill that is long enough\nmetadata:\n  version: \"{version}\"\n---\n# Test"
            ),
        )
        .unwrap();
    }

    #[test]
    fn run_pack_without_manifest_json_produces_skill_file() {
        let dir = tempdir().unwrap();
        make_skill_dir(dir.path(), "1.0.0");
        assert!(!dir.path().join("manifest.json").exists());

        let keys_dir = tempdir().unwrap();
        let keys = crate::keys::ensure_keys_exist(keys_dir.path(), "acme").unwrap();
        let key_path = keys_dir.path().join("test.key");
        let cert_path = keys_dir.path().join("test.crt");
        fs::write(&key_path, &keys.private_key_pem).unwrap();
        fs::write(&cert_path, &keys.cert_pem).unwrap();

        run_pack(dir.path(), Some(&key_path), Some(&cert_path)).unwrap();

        let out = dir.path().join("test-1.0.0.skill");
        assert!(out.exists(), "expected {}", out.display());
        assert!(out.metadata().unwrap().len() > 0);
    }

    #[test]
    fn run_pack_does_not_modify_source_dir() {
        let dir = tempdir().unwrap();
        make_skill_dir(dir.path(), "2.0.0");

        let keys_dir = tempdir().unwrap();
        let keys = crate::keys::ensure_keys_exist(keys_dir.path(), "acme").unwrap();
        let key_path = keys_dir.path().join("test.key");
        let cert_path = keys_dir.path().join("test.crt");
        fs::write(&key_path, &keys.private_key_pem).unwrap();
        fs::write(&cert_path, &keys.cert_pem).unwrap();

        run_pack(dir.path(), Some(&key_path), Some(&cert_path)).unwrap();

        // Source dir must not gain a manifest.json
        assert!(!dir.path().join("manifest.json").exists());
    }

    #[test]
    fn run_pack_missing_metadata_version_errors() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("SKILL.md"),
            "---\nname: test\ndescription: a test skill that is long enough\n---\n# Test",
        )
        .unwrap();
        let keys_dir = tempdir().unwrap();
        let keys = crate::keys::ensure_keys_exist(keys_dir.path(), "acme").unwrap();
        let key_path = keys_dir.path().join("test.key");
        let cert_path = keys_dir.path().join("test.crt");
        fs::write(&key_path, &keys.private_key_pem).unwrap();
        fs::write(&cert_path, &keys.cert_pem).unwrap();

        let err = run_pack(dir.path(), Some(&key_path), Some(&cert_path)).unwrap_err();
        assert!(err.to_string().contains("metadata.version"), "got: {err}");
    }
}
