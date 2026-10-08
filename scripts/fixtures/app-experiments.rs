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
    pub use crate::{agent_activity, app_experiments, app_preferences};
}
#[path = "../../src-tauri/src/modules/agent_activity.rs"]
pub mod agent_activity;
#[path = "../../src-tauri/src/modules/app_connection.rs"]
pub mod app_connection;
#[path = "../../src-tauri/src/modules/app_experiments.rs"]
pub mod app_experiments;
#[path = "../../src-tauri/src/modules/app_identity.rs"]
pub mod app_identity;
#[path = "../../src-tauri/src/modules/app_metadata_macos.rs"]
pub mod app_metadata_macos;
#[path = "../../src-tauri/src/modules/app_preferences.rs"]
pub mod app_preferences;
#[path = "../../src-tauri/src/modules/app_transport.rs"]
pub mod app_transport;
pub use modules::{account, config};
fn main() {
    tauri::async_runtime::block_on(async {
        let status = modules::app_experiments::get_app_experiments()
            .await
            .expect("App preferences must be readable");
        assert_eq!(status["available"], true, "App must be connected");
        let native = status["native"].clone();
        let awake = native["keepComputerAwake"].as_bool().unwrap();
        let changed = modules::app_experiments::set_app_native_preferences(
            serde_json::json!({"keepComputerAwake":!awake}),
        )
        .await;
        let restored = modules::app_experiments::set_app_native_preferences(native.clone())
            .await
            .expect("Native preferences must restore");
        assert_eq!(restored, native);
        assert_eq!(changed.unwrap()["keepComputerAwake"], !awake);
        // Send the same saved value through the actual core-settings patch RPC.
        // No account, keyring, migration flag or plugin write is requested.
        let core = modules::app_preferences::current().unwrap();
        let same = core["artifactReviewMode"].clone();
        let confirmed = tauri::async_runtime::spawn_blocking(move || {
            modules::app_preferences::write(serde_json::json!({"artifactReviewMode":same}))
        })
        .await
        .unwrap()
        .expect("Core setting patch must confirm");
        assert_eq!(confirmed, core, "Unrelated preferences must remain intact");
        let restore_patch = serde_json::json!({"conversationWidth":core["conversationWidth"],"verboseAgentChat":core["verboseAgentChat"]});
        let changed_patch = serde_json::json!({"conversationWidth":if core["conversationWidth"]==3 {1} else {3},"verboseAgentChat":!core["verboseAgentChat"].as_bool().unwrap()});
        let changed = tauri::async_runtime::spawn_blocking(move || {
            modules::app_preferences::write(changed_patch)
        })
        .await
        .unwrap();
        let restored = tauri::async_runtime::spawn_blocking(move || {
            modules::app_preferences::write(restore_patch)
        })
        .await
        .unwrap()
        .expect("Shared display preferences must restore");
        assert!(
            changed.is_ok(),
            "Shared display preferences must confirm changes"
        );
        assert_eq!(restored, core, "Shared preferences must restore exactly");
        #[cfg(target_os = "macos")]
        let app_activity = tauri::async_runtime::spawn_blocking(|| {
            crate::app_identity::with_running_connection(None, crate::agent_activity::observe)
        })
        .await
        .unwrap()
        .expect("App task observations must be available");
        #[cfg(not(target_os = "macos"))]
        let app_activity: Option<crate::agent_activity::Activity> = None;
        println!(
            "{}",
            serde_json::json!({"stage":"preferences","version":status["version"],"native_patch_and_restore":true,"core_noop_confirmed":true,"app_task_state":format!("{:?}",app_activity),"combined_task_state":format!("{:?}",tauri::async_runtime::spawn_blocking(modules::agent_activity::running).await.unwrap())})
        );
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
        println!(
            "{}",
            serde_json::json!({"stage":"translation_restored","translated":status["translated"]})
        );
    });
}
