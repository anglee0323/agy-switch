//! Explicitly requested App preferences and opt-in runtime translation.
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
    preset_signature: Option<String>,
}
fn save(prefs: &Preferences) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(prefs).map_err(|_| "experimental_preferences_invalid")?;
    crate::utils::fs::write_atomic(
        &super::account::get_data_dir()?.join("app_experiments.json"),
        &bytes,
    )
    .map_err(|_| "experimental_preferences_write_failed".into())
}
fn signature(settings: &Value, native: &Value) -> String {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    json!({"settings":settings,"native":native})
        .to_string()
        .hash(&mut hash);
    format!("{:016x}", hash.finish())
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
            Ok((mut connection,pages,version)) => {
                let native = connection.app_preferences(&pages[0],None).map_err(|_| "app_preferences_unavailable")?;
                let settings = super::app_preferences::current()?;
                let preset = prefs.preset_signature.as_deref()==Some(signature(&settings,&native).as_str());
                Ok(json!({"available":true,"version":version,"translation_enabled":prefs.translation_enabled,
                    "translated":TRANSLATED.load(Ordering::Relaxed),"native":native,"settings":settings,"recommended":preset,"state":"connected"}))
            },
            Err(reason) => Ok(json!({"available":false,"translation_enabled":prefs.translation_enabled,
                "translated":0,"native":{},"state":reason})),
        }
    }).await.map_err(|_| "app_preferences_unavailable".to_string())?
}

#[tauri::command]
pub async fn set_app_native_preferences(patch: Value) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = CONTROL
            .lock()
            .map_err(|_| "experimental_preferences_unavailable")?;
        let (mut connection, pages, _) = super::app_connection::connect()?;
        let result = connection
            .app_preferences(&pages[0], Some(&patch))
            .map_err(|_| "app_preferences_write_failed")?;
        let mut prefs = read()?;
        prefs.preset_signature = None;
        save(&prefs)?;
        Ok(result)
    })
    .await
    .map_err(|_| "app_preferences_write_failed".to_string())?
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

#[tauri::command]
pub async fn set_app_shared_preferences(patch: Value) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = CONTROL
            .lock()
            .map_err(|_| "experimental_preferences_unavailable")?;
        let result = super::app_preferences::write(patch)?;
        let mut prefs = read()?;
        prefs.preset_signature = None;
        save(&prefs)?;
        Ok(result)
    })
    .await
    .map_err(|_| "app_settings_write_failed".to_string())?
}

#[tauri::command]
pub async fn set_app_preset(recommended: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = CONTROL
            .lock()
            .map_err(|_| "experimental_preferences_unavailable")?;
        let mut prefs = read()?;
        prefs.preset_signature = None;
        if recommended {
            let current = super::app_preferences::current()?;
            let settings =
                super::app_preferences::write(super::app_preferences::recommended_patch(&current))?;
            let (mut connection, pages, _) = super::app_connection::connect()?;
            let native = connection
                .app_preferences(
                    &pages[0],
                    Some(&json!({"keepComputerAwake":true,"runInBackground":true})),
                )
                .map_err(|_| "app_preferences_write_failed")?;
            prefs.preset_signature = Some(signature(&settings, &native));
        }
        save(&prefs)
    })
    .await
    .map_err(|_| "app_settings_write_failed".to_string())?
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
}
