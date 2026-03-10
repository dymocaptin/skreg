use semver::Version;
use skreg_core::manifest::Manifest;
use skreg_core::types::{Namespace, PackageName, Sha256Digest};

#[test]
fn manifest_serialises_and_roundtrips() {
    let manifest = Manifest {
        namespace: Namespace::new("acme").unwrap(),
        name: PackageName::new("deploy-helper").unwrap(),
        version: Version::parse("1.2.3").unwrap(),
        description: "A helpful deployment skill.".to_owned(),
        category: Some("deployment".to_owned()),
        sha256: Sha256Digest::from_hex(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        )
        .unwrap(),
        cert_chain_pem: vec![],
        publisher_sig_hex: None,
    };

    let json = serde_json::to_string(&manifest).unwrap();
    let roundtripped: Manifest = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtripped.name.as_str(), "deploy-helper");
    assert_eq!(roundtripped.version.to_string(), "1.2.3");
}

#[test]
fn manifest_with_publisher_sig_roundtrips() {
    let json = r#"{
        "namespace": "acme",
        "name": "my-skill",
        "version": "1.0.0",
        "description": "a test skill that is long enough",
        "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "cert_chain_pem": [],
        "publisher_sig_hex": "deadbeef"
    }"#;
    let m: Manifest = serde_json::from_str(json).unwrap();
    assert_eq!(m.publisher_sig_hex.as_deref(), Some("deadbeef"));
    let roundtripped = serde_json::to_string(&m).unwrap();
    let m2: Manifest = serde_json::from_str(&roundtripped).unwrap();
    assert_eq!(m2.publisher_sig_hex, m.publisher_sig_hex);
}

#[test]
fn manifest_without_publisher_sig_deserializes_as_none() {
    let json = r#"{
        "namespace": "acme",
        "name": "my-skill",
        "version": "1.0.0",
        "description": "a test skill that is long enough",
        "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "cert_chain_pem": []
    }"#;
    let m: Manifest = serde_json::from_str(json).unwrap();
    assert!(m.publisher_sig_hex.is_none());
}
