//! Opt-in real App acceptance. Isolated Switch storage, no account operations.
#![allow(dead_code)]
mod models {
    pub mod config {
        include!("../../src-tauri/src/models/config.rs");
    }
    pub use config::AppConfig;
}
mod utils {
    pub mod fs {
        include!("../../src-tauri/src/utils/fs.rs");
    }
    pub mod process {
        include!("../../src-tauri/src/utils/process.rs");
    }
}
mod modules {
    pub mod account {
        pub fn get_data_dir() -> Result<std::path::PathBuf, String> {
            let root = std::env::var_os("AGY_APP_FIXTURE_DATA")
                .map(std::path::PathBuf::from)
                .ok_or("Missing isolated fixture")?;
            if !root.join(".app-fixture").is_file() {
                return Err("Not an isolated fixture".into());
            }
            Ok(root)
        }
    }
    pub mod config {
        pub fn load_app_config() -> Result<crate::models::AppConfig, String> {
            Ok(crate::models::AppConfig::new())
        }
    }
    pub mod i18n {
        pub fn default_language() -> String {
            "en".into()
        }
    }
    pub use crate::app_experiments;
}
#[path = "../../src-tauri/src/modules/app_connection.rs"]
pub mod app_connection;
#[path = "../../src-tauri/src/modules/app_experiments.rs"]
pub mod app_experiments;
#[path = "../../src-tauri/src/modules/app_metadata_macos.rs"]
pub mod app_metadata_macos;
#[cfg(target_os = "windows")]
#[path = "../../src-tauri/src/modules/app_metadata_windows.rs"]
pub mod app_metadata_windows;
#[path = "../../src-tauri/src/modules/app_transport.rs"]
pub mod app_transport;
pub use modules::{account, config};
fn controller_active() -> bool {
    let (mut connection, pages, version) = crate::app_connection::connect().unwrap();
    pages.into_iter().any(|page| {
        connection
            .run(&page, &version, crate::app_transport::RuntimeAction::Probe)
            .unwrap()
            .active
    })
}
async fn wait_for_controller(active: bool) {
    for _ in 0..30 {
        if controller_active() == active {
            let status = modules::app_experiments::get_app_experiments()
                .await
                .unwrap();
            if !active || status["translated"].as_u64().unwrap() > 0 {
                return;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    panic!("Translation controller did not reach the expected state");
}
fn main() {
    tauri::async_runtime::block_on(async {
        let status = modules::app_experiments::get_app_experiments()
            .await
            .expect("App connection status must be readable");
        assert_eq!(status["available"], true, "App must be connected");
        assert!(status.get("settings").is_none());
        assert!(status.get("native").is_none());
        println!(
            "{}",
            serde_json::json!({"stage":"connected","version":status["version"]})
        );
        if std::env::var("AGY_APP_FIXTURE_CLI").as_deref() == Ok("1") {
            let root = modules::account::get_data_dir().unwrap();
            modules::app_experiments::configure_translation_at(&root, true).unwrap();
            let worker = modules::app_experiments::start_foreground(&root);
            wait_for_controller(true).await;
            let status = modules::app_experiments::get_app_experiments()
                .await
                .unwrap();
            assert!(status["translated"].as_u64().unwrap() > 0);
            // A maintainer's already-running Switch can keep renewing the same
            // App controller. Never stop that process or change its real flag
            // merely to manufacture an isolated lease-expiry test.
            if std::env::var("AGY_APP_FIXTURE_OTHER_RUNNER").as_deref() == Ok("1") {
                drop(worker);
                tokio::time::sleep(std::time::Duration::from_secs(17)).await;
                assert!(
                    controller_active(),
                    "The other runner must retain its translation"
                );
                modules::app_experiments::configure_translation_at(&root, false).unwrap();
                println!(
                    "{}",
                    serde_json::json!({"stage":"cli_coexisting_runner","translated":status["translated"],"other_runner_remains_active":true,"isolated_switch_restored":true})
                );
                return;
            }
            modules::app_experiments::configure_translation_at(&root, false).unwrap();
            wait_for_controller(false).await;
            modules::app_experiments::configure_translation_at(&root, true).unwrap();
            wait_for_controller(true).await;
            drop(worker);
            tokio::time::sleep(std::time::Duration::from_secs(17)).await;
            assert!(
                !controller_active(),
                "Stopping the foreground runner must expire its lease"
            );
            assert!(modules::app_experiments::translation_enabled_at(&root).unwrap());
            modules::app_experiments::configure_translation_at(&root, false).unwrap();
            println!(
                "{}",
                serde_json::json!({"stage":"cli_translation_restored","translated":status["translated"],"external_off_observed":true,"foreground_lease_expired":true})
            );
            return;
        }
        modules::app_experiments::set_app_translation(true)
            .await
            .expect("Translation must start");
        modules::app_experiments::initialize();
        println!(
            "{}",
            serde_json::json!({"stage":"translation_active","continue_file":modules::account::get_data_dir().unwrap().join("continue")})
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
        while std::time::Instant::now() < deadline
            && !modules::account::get_data_dir()
                .unwrap()
                .join("continue")
                .exists()
        {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        let status = modules::app_experiments::get_app_experiments()
            .await
            .unwrap();
        modules::app_experiments::set_app_translation(false)
            .await
            .expect("Translation must restore");
        let (mut connection, pages, version) = crate::app_connection::connect().unwrap();
        for page in pages {
            let report = connection
                .run(&page, &version, crate::app_transport::RuntimeAction::Probe)
                .unwrap();
            assert!(
                !report.active,
                "The translation controller must be disposed"
            );
        }
        assert!(
            status["translated"].as_u64().unwrap() > 0,
            "At least one label must translate"
        );
        println!(
            "{}",
            serde_json::json!({"stage":"translation_restored","translated":status["translated"],"controller_disposed":true})
        );
    });
}
