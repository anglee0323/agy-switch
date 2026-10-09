pub mod account;
pub mod account_dashboard;
pub mod account_service;
pub mod account_sync;
pub mod api_pricing;
pub(crate) mod app_identity;
pub(crate) mod app_hub;
#[cfg(any(target_os = "macos", target_os = "windows", test))]
pub(crate) mod app_metadata_macos;
pub mod cli_credentials;
pub mod config;
pub mod db;
pub mod device;
pub mod i18n;
pub mod integration;
#[cfg(target_os = "linux")]
pub mod linux_credentials;
#[cfg(target_os = "linux")]
pub mod linux_paths;
pub mod logger;
pub mod migration;
pub mod native_token_stats;
pub mod menu_bar_usage;
pub mod oauth;
pub mod oauth_server;
pub mod process;
pub mod project_resolver;
pub mod quota;
pub mod quota_refresh;
pub mod tray;
pub mod version;

// Re-export commonly used functions to the top level of the modules namespace for easy external calling
pub use account::*;
pub use config::*;
pub use quota::*;
// pub use device::*;

pub mod desktop;
pub mod menu_bar_projection;
#[cfg(target_os = "macos")]
pub mod native_menu;
pub mod auto_switch;
pub(crate) mod agent_activity;
pub(crate) mod app_transport;
pub(crate) mod app_connection;
pub mod app_experiments;
#[cfg(any(target_os = "windows", test))]
pub(crate) mod app_metadata_windows;
pub mod updater;
