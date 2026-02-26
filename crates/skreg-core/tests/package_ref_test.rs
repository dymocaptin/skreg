use skreg_core::package_ref::PackageRef;

#[test]
fn package_ref_parses_with_version() {
    let r = PackageRef::parse("acme/deploy-helper@1.2.3").unwrap();
    assert_eq!(r.namespace.as_str(), "acme");
    assert_eq!(r.name.as_str(), "deploy-helper");
    assert_eq!(r.version.unwrap().to_string(), "1.2.3");
}

#[test]
fn package_ref_parses_without_version() {
    let r = PackageRef::parse("acme/deploy-helper").unwrap();
    assert!(r.version.is_none());
}

#[test]
fn package_ref_rejects_missing_slash() {
    assert!(PackageRef::parse("acme-deploy-helper").is_err());
}
