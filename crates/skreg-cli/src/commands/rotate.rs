//! `skreg rotate` — rotate the publisher key (requires email confirmation).

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::{default_config_path, load_config};
use crate::keys::{generate_self_signed_cert, keys_dir, pss_sign_digest};

/// Rotation token sent to the registry for key-rotation requests.
///
/// Both the old and new keys sign over the canonical JSON serialization of
/// this token to prove possession of both private keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationToken {
    /// Authenticated namespace slug.
    pub namespace: String,
    /// SHA-256 SPKI fingerprint (hex) of the current key being replaced.
    pub old_key_fingerprint: String,
    /// SHA-256 SPKI fingerprint (hex) of the new key.
    pub new_key_fingerprint: String,
    /// PEM-encoded cert chain for the new key (leaf first).
    pub new_cert_chain_pem: Vec<String>,
    /// 32-byte random nonce (hex) — prevents replay.
    pub nonce: String,
    /// RFC 3339 timestamp when this token was created.
    pub issued_at: String,
    /// RFC 3339 timestamp when this token expires (5 min after `issued_at`).
    pub expires_at: String,
}

/// Serialize `token` to canonical JSON bytes (UTF-8).
///
/// `serde_json::to_string` produces a deterministic representation for a
/// given set of field values because the fields are declared in struct order
/// and `serde_json` does not reorder them.
///
/// # Errors
///
/// Returns an error if serialization fails (in practice this never happens
/// for a plain-struct type with only `String`/`Vec<String>` fields).
pub fn canonical_json_bytes(token: &RotationToken) -> Result<Vec<u8>> {
    Ok(serde_json::to_string(token)?.into_bytes())
}

/// Compute the SHA-256 SPKI fingerprint of a PEM certificate.
///
/// Parses the cert using `x509_cert`, DER-encodes the
/// `SubjectPublicKeyInfo`, and returns `hex::encode(sha256(spki_der))`.
///
/// # Errors
///
/// Returns an error if the cert cannot be parsed or DER-encoded.
fn spki_fingerprint(cert_pem: &str) -> Result<String> {
    use der::{DecodePem, Encode};
    use x509_cert::Certificate;

    let cert = Certificate::from_pem(cert_pem).context("parsing certificate PEM")?;
    let spki_der = cert
        .tbs_certificate
        .subject_public_key_info
        .to_der()
        .context("DER-encoding SPKI")?;
    Ok(hex::encode(Sha256::digest(&spki_der)))
}

/// Generate a fresh RSA-2048 key + self-signed cert and return
/// `(private_key_pem, cert_pem)`.
fn generate_new_key(namespace: &str) -> Result<(String, String)> {
    use rand::rngs::OsRng;
    use rsa::pkcs8::EncodePrivateKey;

    let private_key =
        rsa::RsaPrivateKey::new(&mut OsRng, 2048).context("generating RSA-2048 key")?;
    let private_key_pem = private_key
        .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
        .context("encoding RSA key to PKCS#8 PEM")?
        .to_string();

    let cert_pem = generate_self_signed_cert(namespace, &private_key_pem)?;
    Ok((private_key_pem, cert_pem))
}

/// Run `skreg rotate` — initiate a publisher key rotation.
///
/// Steps:
/// 1. Load config (namespace + `api_key`).
/// 2. Read the current `publisher.key` and `publisher.crt` from `~/.skreg/keys/`.
/// 3. Generate a new RSA-2048 key + self-signed cert (or read from `new_key_override`).
/// 4. Compute SPKI fingerprints for both keys.
/// 5. Build a [`RotationToken`] with a 32-byte random nonce and 5-min expiry.
/// 6. Sign `sha256(canonical_json)` with both old and new private keys.
/// 7. POST `{ token, old_sig, new_sig }` to `{registry}/v1/namespaces/{ns}/rotate-key`.
/// 8. Save the new key to `~/.skreg/keys/pending/`.
/// 9. Print instructions to check email.
///
/// # Errors
///
/// Returns an error if config, keys, or the registry request fails.
pub async fn run_rotate(new_key_override: Option<&Path>) -> Result<()> {
    let cfg_path = default_config_path();
    let cfg =
        load_config(&cfg_path).context("not logged in — run `skreg login <namespace>` first")?;

    let namespace = cfg.namespace().to_owned();
    let api_key = cfg.api_key().to_owned();
    let registry = cfg.registry().to_owned();

    let kdir = keys_dir()?;
    let old_key_path = kdir.join("publisher.key");
    let old_cert_path = kdir.join("publisher.crt");

    let old_key_pem = std::fs::read_to_string(&old_key_path)
        .with_context(|| format!("reading {}", old_key_path.display()))?;
    let old_cert_pem = std::fs::read_to_string(&old_cert_path)
        .with_context(|| format!("reading {}", old_cert_path.display()))?;

    // New key: either read from override path or generate fresh.
    let (new_key_pem, new_cert_pem) = if let Some(key_path) = new_key_override {
        let pem = std::fs::read_to_string(key_path)
            .with_context(|| format!("reading new key from {}", key_path.display()))?;
        let cert = generate_self_signed_cert(&namespace, &pem)?;
        (pem, cert)
    } else {
        generate_new_key(&namespace)?
    };

    // Compute SPKI fingerprints.
    let old_fp = spki_fingerprint(&old_cert_pem)
        .context("computing fingerprint of current publisher cert")?;
    let new_fp =
        spki_fingerprint(&new_cert_pem).context("computing fingerprint of new publisher cert")?;

    // Build rotation token.
    let now = chrono::Utc::now();
    let expires = now + chrono::Duration::minutes(5);
    let nonce = hex::encode(rand::random::<[u8; 32]>());

    let token = RotationToken {
        namespace: namespace.clone(),
        old_key_fingerprint: old_fp,
        new_key_fingerprint: new_fp,
        new_cert_chain_pem: vec![new_cert_pem.clone()],
        nonce,
        issued_at: now.to_rfc3339(),
        expires_at: expires.to_rfc3339(),
    };

    // Sign sha256(canonical_json) with both keys.
    let token_bytes = canonical_json_bytes(&token)?;
    let digest_hex = hex::encode(Sha256::digest(&token_bytes));

    let old_sig = pss_sign_digest(&old_key_pem, &digest_hex)
        .context("signing rotation token with old key")?;
    let new_sig = pss_sign_digest(&new_key_pem, &digest_hex)
        .context("signing rotation token with new key")?;

    // POST to the registry.
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{registry}/v1/namespaces/{namespace}/rotate-key"))
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&serde_json::json!({
            "token": token,
            "old_sig": old_sig,
            "new_sig": new_sig,
        }))
        .send()
        .await
        .context("sending rotate-key request")?;

    if !resp.status().is_success() {
        bail!(
            "rotate-key failed: {} — {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        );
    }

    // Save new key to ~/.skreg/keys/pending/.
    let pending_dir = kdir.join("pending");
    std::fs::create_dir_all(&pending_dir)
        .with_context(|| format!("creating pending dir {}", pending_dir.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&pending_dir, std::fs::Permissions::from_mode(0o700))
            .context("setting permissions on pending dir")?;
    }

    let pending_key_path = pending_dir.join("publisher.key");
    let pending_cert_path = pending_dir.join("publisher.crt");

    std::fs::write(&pending_key_path, &new_key_pem)
        .with_context(|| format!("writing {}", pending_key_path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&pending_key_path, std::fs::Permissions::from_mode(0o600))
            .context("setting permissions on pending key")?;
    }

    std::fs::write(&pending_cert_path, &new_cert_pem)
        .with_context(|| format!("writing {}", pending_cert_path.display()))?;

    println!("Rotation initiated. Check your email to confirm.");
    println!(
        "New key saved to {} (active after confirmation).",
        pending_key_path.display()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_token_serializes_deterministically() {
        let t = RotationToken {
            namespace: "acme".into(),
            old_key_fingerprint: "aaa".into(),
            new_key_fingerprint: "bbb".into(),
            new_cert_chain_pem: vec!["cert".into()],
            nonce: "nnn".into(),
            issued_at: "2026-01-01T00:00:00Z".into(),
            expires_at: "2026-01-01T00:05:00Z".into(),
        };
        let b1 = canonical_json_bytes(&t).unwrap();
        let b2 = canonical_json_bytes(&t.clone()).unwrap();
        assert_eq!(b1, b2);
    }
}
