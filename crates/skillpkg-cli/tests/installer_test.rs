use skillpkg_cli::installer::InstallError;

#[test]
fn install_error_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<InstallError>();
}
