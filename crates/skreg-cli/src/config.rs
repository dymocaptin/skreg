//! CLI configuration — re-exported from `skreg-core`.
// Re-export config types from skreg-core so existing skreg-cli code keeps working.
pub use skreg_core::config::{
    default_config_path, load_config, save_config, CliConfig, ContextConfig,
};
