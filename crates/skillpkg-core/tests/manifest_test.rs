use semver::Version;
use skillpkg_core::manifest::Manifest;
use skillpkg_core::types::{Namespace, PackageName, Sha256Digest};

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
    };

    let json = serde_json::to_string(&manifest).unwrap();
    let roundtripped: Manifest = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtripped.name.as_str(), "deploy-helper");
    assert_eq!(roundtripped.version.to_string(), "1.2.3");
}
