//! Publisher key management — auto-keygen and RSA-PSS signing for `skreg pack`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::Sha256;

/// Publisher keys loaded from disk or freshly generated.
pub struct PublisherKeys {
    /// PEM-encoded RSA private key.
    pub private_key_pem: String,
    /// PEM-encoded leaf certificate.
    pub cert_pem: String,
    /// Certificate chain (leaf first, then any intermediates).
    pub cert_chain_pem: Vec<String>,
}

/// Returns `~/.skreg/keys/`.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined.
pub fn keys_dir() -> Result<PathBuf> {
    let home = home::home_dir().context("cannot determine home directory")?;
    Ok(home.join(".skreg").join("keys"))
}

/// Load or generate publisher keys in `dir`.
///
/// If `publisher.key` and `publisher.crt` already exist they are loaded.
/// If `publisher-ca.crt` also exists the cert chain is `[leaf, ca]`.
///
/// If any key or cert is missing, a fresh RSA-2048 key and self-signed
/// certificate are generated and written to disk. A notice is printed to
/// stderr.
///
/// # Errors
///
/// Returns an error if key generation or file I/O fails.
pub fn ensure_keys_exist(dir: &Path, namespace: &str) -> Result<PublisherKeys> {
    let key_path = dir.join("publisher.key");
    let cert_path = dir.join("publisher.crt");
    let ca_path = dir.join("publisher-ca.crt");

    if key_path.exists() && cert_path.exists() {
        let private_key_pem = std::fs::read_to_string(&key_path)
            .with_context(|| format!("reading {}", key_path.display()))?;
        let cert_pem = std::fs::read_to_string(&cert_path)
            .with_context(|| format!("reading {}", cert_path.display()))?;
        let cert_chain_pem = if ca_path.exists() {
            let ca_pem = std::fs::read_to_string(&ca_path)
                .with_context(|| format!("reading {}", ca_path.display()))?;
            vec![cert_pem.clone(), ca_pem]
        } else {
            vec![cert_pem.clone()]
        };
        return Ok(PublisherKeys {
            private_key_pem,
            cert_pem,
            cert_chain_pem,
        });
    }

    eprintln!(
        "skreg: no publisher keys found in {}; generating RSA-2048 key pair…",
        dir.display()
    );

    let (private_key_pem, cert_pem) = generate_key_and_cert(namespace)?;

    create_dir_secure(dir)?;
    write_secure(&key_path, &private_key_pem)?;
    write_file(&cert_path, &cert_pem)?;

    eprintln!(
        "skreg: wrote {} and {}",
        key_path.display(),
        cert_path.display()
    );

    Ok(PublisherKeys {
        private_key_pem,
        cert_pem: cert_pem.clone(),
        cert_chain_pem: vec![cert_pem],
    })
}

/// Load keys from explicit paths.
///
/// # Errors
///
/// Returns an error if either file cannot be read.
pub fn load_explicit_keys(key_path: &Path, cert_path: &Path) -> Result<PublisherKeys> {
    let private_key_pem = std::fs::read_to_string(key_path)
        .with_context(|| format!("reading {}", key_path.display()))?;
    let cert_pem = std::fs::read_to_string(cert_path)
        .with_context(|| format!("reading {}", cert_path.display()))?;
    let cert_chain_pem = vec![cert_pem.clone()];
    Ok(PublisherKeys {
        private_key_pem,
        cert_pem,
        cert_chain_pem,
    })
}

/// Generate a fresh RSA-2048 private key and a self-signed certificate valid
/// for 90 days.  Returns `(private_key_pem, cert_pem)`.
fn generate_key_and_cert(namespace: &str) -> Result<(String, String)> {
    use rand::rngs::OsRng;
    use rsa::pkcs8::EncodePrivateKey;

    let private_key =
        rsa::RsaPrivateKey::new(&mut OsRng, 2048).context("generating RSA-2048 key")?;
    let private_key_pem = private_key
        .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
        .context("encoding private key to PEM")?
        .to_string();

    let cert_pem = generate_self_signed_cert(namespace, &private_key_pem)?;
    Ok((private_key_pem, cert_pem))
}

/// Generate a self-signed certificate for `namespace` using the provided PEM
/// private key.  Uses `rcgen::PKCS_RSA_SHA256` (PKCS#1) and 90-day validity.
///
/// # Errors
///
/// Returns an error if cert generation fails.
pub(crate) fn generate_self_signed_cert(namespace: &str, key_pem: &str) -> Result<String> {
    use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType, KeyPair};

    let key_pair = KeyPair::from_pem(key_pem).context("parsing private key PEM for rcgen")?;

    let now = chrono::Utc::now();
    let expiry = now + chrono::Duration::days(90);

    let not_before = rcgen::date_time_ymd(
        now.format("%Y").to_string().parse::<i32>().unwrap_or(2025),
        now.format("%m").to_string().parse::<u8>().unwrap_or(1),
        now.format("%d").to_string().parse::<u8>().unwrap_or(1),
    );
    let not_after = rcgen::date_time_ymd(
        expiry
            .format("%Y")
            .to_string()
            .parse::<i32>()
            .unwrap_or(2025),
        expiry.format("%m").to_string().parse::<u8>().unwrap_or(1),
        expiry.format("%d").to_string().parse::<u8>().unwrap_or(1),
    );

    let mut params = CertificateParams::default();
    params.alg = &rcgen::PKCS_RSA_SHA256;
    params.key_pair = Some(key_pair);
    params.not_before = not_before;
    params.not_after = not_after;

    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, namespace);
    params.distinguished_name = dn;

    let cert = Certificate::from_params(params).context("generating self-signed certificate")?;
    cert.serialize_pem()
        .context("serializing certificate to PEM")
}

/// Sign a SHA-256 digest (hex-encoded) with RSA-PSS using the provided PEM
/// private key.  Returns the hex-encoded signature.
///
/// # Errors
///
/// Returns an error if the key cannot be parsed or signing fails.
pub fn pss_sign_digest(private_key_pem: &str, digest_hex: &str) -> Result<String> {
    use rand::rngs::OsRng;
    use rsa::pkcs8::DecodePrivateKey;
    use rsa::pss::BlindedSigningKey;
    use rsa::signature::hazmat::RandomizedPrehashSigner;
    use rsa::signature::SignatureEncoding;

    let digest_bytes = hex::decode(digest_hex).context("decoding digest hex")?;

    let private_key =
        rsa::RsaPrivateKey::from_pkcs8_pem(private_key_pem).context("parsing private key PEM")?;
    let signing_key = BlindedSigningKey::<Sha256>::new(private_key);

    let sig = signing_key
        .sign_prehash_with_rng(&mut OsRng, &digest_bytes)
        .context("signing digest")?;

    Ok(hex::encode(sig.to_bytes()))
}

/// Create directory (and parents) with mode 0o700 on Unix.
fn create_dir_secure(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)
        .with_context(|| format!("creating directory {}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .with_context(|| format!("setting permissions on {}", dir.display()))?;
    }
    Ok(())
}

/// Write file with mode 0o600 on Unix.
fn write_secure(path: &Path, contents: &str) -> Result<()> {
    std::fs::write(path, contents).with_context(|| format!("writing {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("setting permissions on {}", path.display()))?;
    }
    Ok(())
}

/// Write file with default permissions.
fn write_file(path: &Path, contents: &str) -> Result<()> {
    std::fs::write(path, contents).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn generates_key_and_cert_when_absent() {
        let dir = tempdir().unwrap();
        let keys = ensure_keys_exist(dir.path(), "testns").unwrap();
        assert!(dir.path().join("publisher.key").exists());
        assert!(dir.path().join("publisher.crt").exists());
        assert!(!keys.private_key_pem.is_empty());
        assert!(!keys.cert_pem.is_empty());
        assert_eq!(keys.cert_chain_pem.len(), 1);
    }

    #[test]
    fn returns_existing_keys_without_regenerating() {
        let dir = tempdir().unwrap();
        ensure_keys_exist(dir.path(), "testns").unwrap();
        let mtime1 = std::fs::metadata(dir.path().join("publisher.key"))
            .unwrap()
            .modified()
            .unwrap();
        ensure_keys_exist(dir.path(), "testns").unwrap();
        let mtime2 = std::fs::metadata(dir.path().join("publisher.key"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(mtime1, mtime2);
    }

    #[test]
    fn pss_sign_and_verify_roundtrip() {
        use sha2::Digest;
        let dir = tempdir().unwrap();
        let keys = ensure_keys_exist(dir.path(), "testns").unwrap();
        let digest_hex = hex::encode(sha2::Sha256::new().finalize());
        let sig_hex = pss_sign_digest(&keys.private_key_pem, &digest_hex).unwrap();
        assert!(!sig_hex.is_empty());
        // RSA-2048 signatures are 256 bytes = 512 hex chars
        assert_eq!(sig_hex.len(), 512);
    }
}
