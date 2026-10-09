//! Opt-in, reversible App interface translation.
use super::app_transport::RuntimeAction;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

static CONTROL: Mutex<()> = Mutex::new(());
static STARTED: AtomicBool = AtomicBool::new(false);
static TRANSLATED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Preferences {
    translation_enabled: bool,
    // Read the old quick-setup marker for upgrade compatibility; never write it.
    #[serde(rename = "preset_signature", skip_serializing)]
    _legacy_preset_signature: Option<String>,
}
fn save(prefs: &Preferences) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(prefs).map_err(|_| "experimental_preferences_invalid")?;
    crate::utils::fs::write_atomic(
        &super::account::get_data_dir()?.join("app_experiments.json"),
        &bytes,
    )
    .map_err(|_| "experimental_preferences_write_failed".into())
}
fn read() -> Result<Preferences, String> {
    let path = super::account::get_data_dir()?.join("app_experiments.json");
    match std::fs::read(&path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|_| "experimental_preferences_invalid".into())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Preferences::default()),
        Err(_) => Err("experimental_preferences_unavailable".into()),
    }
}
fn reconcile(enabled: bool) -> Result<(String, u64), String> {
    let (mut connection, pages, version) = super::app_connection::connect()?;
    let mut count = 0;
    for page in pages {
        let report = connection
            .run(
                &page,
                &version,
                if enabled {
                    RuntimeAction::Apply
                } else {
                    RuntimeAction::Dispose
                },
            )
            .map_err(|_| "app_translation_failed")?;
        count += report.translated;
    }
    TRANSLATED.store(count, Ordering::Relaxed);
    Ok((version, count))
}
pub fn initialize() {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| loop {
        if let Ok(_guard) = CONTROL.lock() {
            if read().is_ok_and(|p| p.translation_enabled) {
                let _ = reconcile(true);
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(3));
    });
}

#[tauri::command]
pub async fn get_app_experiments() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let _guard = CONTROL.lock().map_err(|_| "experimental_preferences_unavailable")?;
        let prefs = read()?;
        let connected = super::app_connection::connect();
        match connected {
            Ok((_, _, version)) => Ok(json!({"available":true,"version":version,"translation_enabled":prefs.translation_enabled,
                "translated":TRANSLATED.load(Ordering::Relaxed),"state":"connected"})),
            Err(reason) => Ok(json!({"available":false,"translation_enabled":prefs.translation_enabled,
                "translated":0,"state":reason})),
        }
    }).await.map_err(|_| "app_connection_failed".to_string())?
}

#[tauri::command]
pub async fn set_app_translation(enabled: bool) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = CONTROL
            .lock()
            .map_err(|_| "experimental_preferences_unavailable")?;
        let mut prefs = read()?;
        if enabled {
            reconcile(true)?;
        }
        // Stop renewals even when a page becomes unavailable. Its existing
        // lease restores the original text instead of trapping the user on.
        prefs.translation_enabled = enabled;
        save(&prefs)?;
        if !enabled {
            let result = reconcile(false);
            TRANSLATED.store(0, Ordering::Relaxed);
            if result
                .as_ref()
                .is_err_and(|e| e == "app_translation_failed")
            {
                return Err("app_translation_restore_pending".into());
            }
        }
        Ok(enabled)
    })
    .await
    .map_err(|_| "app_translation_failed".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn experiments_are_opt_in_and_reject_unrelated_configuration() {
        assert!(
            !serde_json::from_str::<Preferences>("{}")
                .unwrap()
                .translation_enabled
        );
        assert!(serde_json::from_str::<Preferences>(r#"{"token":"must-not-be-saved"}"#).is_err());
    }

    #[test]
    fn existing_translation_survives_removing_quick_setup() {
        let prefs: Preferences = serde_json::from_str(
            r#"{"translation_enabled":true,"preset_signature":"legacy-marker"}"#,
        )
        .unwrap();
        assert!(prefs.translation_enabled);
        assert_eq!(
            serde_json::to_value(prefs).unwrap(),
            json!({"translation_enabled":true})
        );
    }
}
