use skreg_cli::installer::InstallError;

#[test]
fn install_error_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<InstallError>();
}

#[test]
fn install_command_module_exists() {
    // Proves skreg_cli::commands::install compiles.
    let _ = skreg_cli::commands::install::run_install;
}
