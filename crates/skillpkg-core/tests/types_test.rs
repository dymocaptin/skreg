use skillpkg_core::types::{Namespace, PackageName, Sha256Digest};

#[test]
fn namespace_rejects_uppercase() {
    assert!(Namespace::new("Acme").is_err());
}

#[test]
fn namespace_rejects_empty() {
    assert!(Namespace::new("").is_err());
}

#[test]
fn namespace_rejects_too_long() {
    assert!(Namespace::new(&"a".repeat(65)).is_err());
}

#[test]
fn namespace_accepts_valid() {
    let ns = Namespace::new("acme-corp").unwrap();
    assert_eq!(ns.as_str(), "acme-corp");
}

#[test]
fn package_name_accepts_valid() {
    let name = PackageName::new("deploy-helper").unwrap();
    assert_eq!(name.as_str(), "deploy-helper");
}

#[test]
fn sha256_digest_rejects_wrong_length() {
    assert!(Sha256Digest::from_hex("abc").is_err());
}

#[test]
fn sha256_digest_accepts_64_hex_chars() {
    let hex = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let digest = Sha256Digest::from_hex(hex).unwrap();
    assert_eq!(digest.as_hex(), hex);
}
