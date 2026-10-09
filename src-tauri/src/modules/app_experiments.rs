//! Opt-in, reversible App interface translation.
use super::app_transport::RuntimeAction;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
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
fn save_at(root: &std::path::Path, prefs: &Preferences) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(prefs).map_err(|_| "experimental_preferences_invalid")?;
    std::fs::create_dir_all(root).map_err(|_| "experimental_preferences_write_failed")?;
    crate::utils::fs::write_atomic(&root.join("app_experiments.json"), &bytes)
        .map_err(|_| "experimental_preferences_write_failed".into())
}
fn read_at(root: &std::path::Path) -> Result<Preferences, String> {
    let path = root.join("app_experiments.json");
    match std::fs::read(&path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|_| "experimental_preferences_invalid".into())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Preferences::default()),
        Err(_) => Err("experimental_preferences_unavailable".into()),
    }
}
fn read() -> Result<Preferences, String> {
    read_at(&super::account::get_data_dir()?)
}

pub(crate) fn translation_enabled_at(root: &std::path::Path) -> Result<bool, String> {
    read_at(root).map(|prefs| prefs.translation_enabled)
}

/// GUI and CLI share one saved switch. This only edits Switch's own preferences;
/// the desktop worker or an explicit foreground CLI runner handles injection.
pub(crate) fn configure_translation_at(
    root: &std::path::Path,
    enabled: bool,
) -> Result<(), String> {
    let _guard = CONTROL
        .lock()
        .map_err(|_| "experimental_preferences_unavailable")?;
    let mut prefs = read_at(root)?;
    prefs.translation_enabled = enabled;
    save_at(root, &prefs)
}
fn reconcile(enabled: bool) -> Result<(String, u64), String> {
    reconcile_until_stopped(enabled, None)
}
fn reconcile_until_stopped(
    enabled: bool,
    stop: Option<&AtomicBool>,
) -> Result<(String, u64), String> {
    let cancelled = || stop.is_some_and(|flag| flag.load(Ordering::Acquire));
    if cancelled() {
        return Err("app_translation_stopped".into());
    }
    let (mut connection, pages, version) = super::app_connection::connect()?;
    let mut count = 0;
    for page in pages {
        if cancelled() {
            return Err("app_translation_stopped".into());
        }
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
    let Ok(root) = super::account::get_data_dir() else {
        return;
    };
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    spawn_worker(root, Arc::new(AtomicBool::new(false)));
}

/// The CLI runner owns this lifetime, including when it returns to the TUI.
/// Stopping one runner lets its lease expire without disposing another runner's
/// translations or changing the shared saved switch.
pub(crate) struct ForegroundWorker(Arc<AtomicBool>);
impl Drop for ForegroundWorker {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
pub(crate) fn start_foreground(root: &std::path::Path) -> ForegroundWorker {
    let stop = Arc::new(AtomicBool::new(false));
    spawn_worker(root.to_path_buf(), Arc::clone(&stop));
    ForegroundWorker(stop)
}
fn spawn_worker(root: std::path::PathBuf, stop: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let mut was_enabled = false;
        while !stop.load(Ordering::Acquire) {
            if let Ok(_guard) = CONTROL.lock() {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                let enabled = read_at(&root).is_ok_and(|p| p.translation_enabled);
                if enabled || was_enabled {
                    let _ = reconcile_until_stopped(enabled, Some(&stop));
                }
                was_enabled = enabled;
            }
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
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
        save_at(&super::account::get_data_dir()?, &prefs)?;
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

    #[test]
    fn shared_switch_is_read_only_by_default_and_never_repairs_corrupt_preferences() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("data");
        assert!(!translation_enabled_at(&root).unwrap());
        assert!(!root.exists());
        configure_translation_at(&root, true).unwrap();
        assert!(translation_enabled_at(&root).unwrap());
        configure_translation_at(&root, false).unwrap();
        assert!(!translation_enabled_at(&root).unwrap());
        let path = root.join("app_experiments.json");
        std::fs::write(&path, "corrupt").unwrap();
        assert!(configure_translation_at(&root, false).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "corrupt");
    }
}
