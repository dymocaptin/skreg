use std::path::PathBuf;

use skreg_core::installed::{InstalledPackage, SignerKind};
use skreg_core::package_ref::PackageRef;
use skreg_core::types::Sha256Digest;

#[test]
fn signer_kind_serialises_registry() {
    let sk = SignerKind::Registry;
    let json = serde_json::to_string(&sk).unwrap();
    assert_eq!(json, r#"{"kind":"registry"}"#);
}

#[test]
fn signer_kind_serialises_publisher() {
    let sk = SignerKind::Publisher { cert_serial: 42 };
    let json = serde_json::to_string(&sk).unwrap();
    assert!(json.contains("publisher"));
    assert!(json.contains("42"));
}

#[test]
fn installed_package_roundtrips_json() {
    let pkg = InstalledPackage {
        pkg_ref: PackageRef::parse("acme/deploy-helper@1.0.0").unwrap(),
        sha256: Sha256Digest::from_hex(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        )
        .unwrap(),
        signer: SignerKind::Registry,
        install_path: PathBuf::from("/home/user/.skreg/packages/acme/deploy-helper/1.0.0"),
    };
    let json = serde_json::to_string(&pkg).unwrap();
    let back: InstalledPackage = serde_json::from_str(&json).unwrap();
    assert_eq!(back.pkg_ref.name.as_str(), "deploy-helper");
}
