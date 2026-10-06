use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use super::account::get_data_dir;
use crate::models::config::DesktopPreferences;
use crate::models::AppConfig;

const CONFIG_FILE: &str = "gui_config.json";
// All in-process configuration reads and writes share one lock. In particular,
// stale whole-settings snapshots preserve separately managed preferences.
static CONFIG_LOCK: Mutex<()> = Mutex::new(());

fn lock_config() -> Result<MutexGuard<'static, ()>, String> {
    CONFIG_LOCK
        .lock()
        .map_err(|_| "config_lock_unavailable".to_string())
}

// These helpers are called only while CONFIG_LOCK is held. Missing files are
// distinct from unreadable or malformed files, which must never be overwritten.
fn read_config_unlocked(path: &Path) -> Result<Option<AppConfig>, String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("failed_to_read_config_file: {error}")),
    };
    serde_json::from_str(&content)
        .map(|mut config: AppConfig| {
            config.dashboard.normalize();
            Some(config)
        })
        .map_err(|error| format!("failed_to_parse_config_file: {error}"))
}

fn write_config_unlocked(path: &Path, config: &AppConfig) -> Result<(), String> {
    let content = serde_json::to_string_pretty(config)
        .map_err(|error| format!("failed_to_serialize_config: {error}"))?;
    crate::utils::fs::write_atomic(path, content.as_bytes())
        .map_err(|error| format!("failed_to_save_config: {error}"))
}

fn load_config_at(path: &Path) -> Result<AppConfig, String> {
    let _guard = lock_config()?;
    if let Some(config) = read_config_unlocked(path)? {
        return Ok(config);
    }
    let config = AppConfig::new();
    // Preserve best-effort first-run persistence without recursively locking.
    let _ = write_config_unlocked(path, &config);
    Ok(config)
}

fn save_config_at(path: &Path, config: &AppConfig) -> Result<(), String> {
    let _guard = lock_config()?;
    let mut next = config.clone();
    // Dedicated setters manage desktop, menu and dashboard preferences. Ordinary
    // settings saves preserve current disk values if their UI snapshot is stale.
    let current = read_config_unlocked(path)?.unwrap_or_default();
    next.desktop = current.desktop;
    next.menu_bar = current.menu_bar;
    next.dashboard = current.dashboard;
    write_config_unlocked(path, &next)
}

fn set_desktop_preferences_at(path: &Path, preferences: &DesktopPreferences) -> Result<(), String> {
    let _guard = lock_config()?;
    let mut config = read_config_unlocked(path)?.unwrap_or_default();
    config.desktop = preferences.clone();
    write_config_unlocked(path, &config)
}

/// Load application configuration.
pub fn load_app_config() -> Result<AppConfig, String> {
    load_config_at(&get_data_dir()?.join(CONFIG_FILE))
}

/// Save ordinary application settings atomically, preserving the separately
/// managed desktop, menu and dashboard preferences from disk configuration.
pub fn save_app_config(config: &AppConfig) -> Result<(), String> {
    save_config_at(&get_data_dir()?.join(CONFIG_FILE), config)
}

/// Persist only the desktop preferences after their OS-backed transaction.
/// The dedicated writer retains ordinary settings saved by
/// another window while the OS operation was in progress.
pub fn set_saved_desktop_preferences(preferences: &DesktopPreferences) -> Result<(), String> {
    set_desktop_preferences_at(&get_data_dir()?.join(CONFIG_FILE), preferences)
}

pub fn set_menu_bar_preferences(
    patch: crate::models::config::MenuBarPreferencesPatch,
) -> Result<crate::models::config::MenuBarPreferences, String> {
    patch_menu_bar_preferences_at(&get_data_dir()?.join(CONFIG_FILE), patch)
}

#[cfg(test)]
fn set_menu_bar_preferences_at(
    path: &Path,
    scope: crate::models::config::MenuBarQuotaScope,
) -> Result<crate::models::config::MenuBarPreferences, String> {
    patch_menu_bar_preferences_at(path, crate::models::config::MenuBarPreferencesPatch { quota_scope: Some(scope), ..Default::default() })
}

pub fn set_dashboard_cards(cards: Vec<String>) -> Result<crate::models::config::DashboardPreferences, String> {
    set_dashboard_cards_at(&get_data_dir()?.join(CONFIG_FILE), cards)
}

fn set_dashboard_cards_at(path: &Path, cards: Vec<String>) -> Result<crate::models::config::DashboardPreferences, String> {
    let _guard = lock_config()?;
    let mut config = read_config_unlocked(path)?.unwrap_or_default();
    config.dashboard.cards = cards;
    config.dashboard.normalize();
    write_config_unlocked(path, &config)?;
    Ok(config.dashboard)
}
fn patch_menu_bar_preferences_at(path: &Path, patch: crate::models::config::MenuBarPreferencesPatch) -> Result<crate::models::config::MenuBarPreferences, String> {
    let _guard = lock_config()?;
    let mut config = read_config_unlocked(path)?.unwrap_or_default();
    patch.apply(&mut config.menu_bar)?;
    write_config_unlocked(path, &config)?;
    Ok(config.menu_bar)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dashboard_selection_order_and_empty_choice_survive_stale_settings_saves() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        let mut stale = load_config_at(&path).unwrap();
        let desktop = DesktopPreferences { launch_at_login: true, ..Default::default() };
        set_desktop_preferences_at(&path, &desktop).unwrap();
        let selected = vec!["quota_reset".into(), "account_status".into(), "aggregate_quota".into(), "body_speed".into(), "total_tokens".into(), "first_text_latency".into()];
        set_dashboard_cards_at(&path, selected.clone()).unwrap();
        stale.language = "en".into();
        save_config_at(&path, &stale).unwrap();
        let actual = load_config_at(&path).unwrap();
        assert_eq!(actual.dashboard.cards, selected);
        assert_eq!(actual.language, "en");
        assert!(actual.desktop.launch_at_login);
        set_dashboard_cards_at(&path, Vec::new()).unwrap();
        save_config_at(&path, &stale).unwrap();
        assert!(load_config_at(&path).unwrap().dashboard.cards.is_empty());
    }

    #[test]
    fn dashboard_normalizes_unknown_duplicates_and_preserves_malformed_files() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        fs::write(&path, r#"{"dashboard":{"cards":["body_speed","unknown","body_speed","api_cost"]}}"#).unwrap();
        assert_eq!(load_config_at(&path).unwrap().dashboard.cards, ["body_speed", "api_cost"]);
        fs::write(&path, b"{broken configuration").unwrap();
        let original = fs::read(&path).unwrap();
        assert!(set_dashboard_cards_at(&path, Vec::new()).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
    }
    #[test]
    fn menu_patches_preserve_other_preferences_and_survive_stale_general_saves() {
        use crate::models::config::{MenuBarPreferencesPatch, MenuBarQuotaScope, MenuBarResetTimeDisplay};
        let root = tempfile::tempdir().unwrap(); let path = root.path().join(CONFIG_FILE);
        let stale = load_config_at(&path).unwrap();
        patch_menu_bar_preferences_at(&path, MenuBarPreferencesPatch { display_scope: Some(MenuBarQuotaScope::Other), green_above: Some(75), ..Default::default() }).unwrap();
        patch_menu_bar_preferences_at(&path, MenuBarPreferencesPatch { hide_unavailable: Some(false), show_reset_on_hover: Some(false), ..Default::default() }).unwrap();
        save_config_at(&path, &stale).unwrap();
        let actual = load_config_at(&path).unwrap().menu_bar;
        assert_eq!(actual.display_scope, MenuBarQuotaScope::Other); assert_eq!(actual.green_above, 75);
        assert!(!actual.hide_unavailable && !actual.show_reset_on_hover);
        assert_eq!(actual.reset_time_mode(), MenuBarResetTimeDisplay::Hidden);
        patch_menu_bar_preferences_at(&path, MenuBarPreferencesPatch { reset_time_display: Some(MenuBarResetTimeDisplay::Always), ..Default::default() }).unwrap();
        save_config_at(&path, &stale).unwrap();
        assert_eq!(load_config_at(&path).unwrap().menu_bar.reset_time_mode(), MenuBarResetTimeDisplay::Always);
        let before = fs::read(&path).unwrap();
        assert!(patch_menu_bar_preferences_at(&path, MenuBarPreferencesPatch { red_below: Some(80), ..Default::default() }).is_err());
        assert!(patch_menu_bar_preferences_at(&path, MenuBarPreferencesPatch { show_session: Some(false), show_weekly: Some(false), ..Default::default() }).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    #[test]
    fn legacy_menu_preferences_receive_new_defaults() {
        use crate::models::config::{MenuBarPreferences, MenuBarQuotaScope, MenuBarLabelStyle, MenuBarResetTimeDisplay};
        let actual: MenuBarPreferences = serde_json::from_str(r#"{"quota_scope":"other"}"#).unwrap();
        assert_eq!(actual.quota_scope, MenuBarQuotaScope::Other); assert_eq!(actual.display_scope, MenuBarQuotaScope::All);
        assert_eq!(actual.label_style, MenuBarLabelStyle::EmailThenLabel);
        assert!(actual.hide_unavailable && actual.show_session && actual.show_weekly && actual.show_reset_on_hover);
        assert_eq!(actual.reset_time_mode(), MenuBarResetTimeDisplay::Hover);
        let disabled: MenuBarPreferences = serde_json::from_str(r#"{"show_reset_on_hover":false}"#).unwrap();
        assert_eq!(disabled.reset_time_mode(), MenuBarResetTimeDisplay::Hidden);
        let always: MenuBarPreferences = serde_json::from_str(r#"{"reset_time_display":"always","show_reset_on_hover":false}"#).unwrap();
        assert_eq!(always.reset_time_mode(), MenuBarResetTimeDisplay::Always);
        assert!(serde_json::from_str::<MenuBarPreferences>(r#"{"reset_time_display":"invalid"}"#).is_err());
        assert_eq!((actual.red_below, actual.green_above), (20, 60));
    }

    #[test]
    fn menu_bar_scope_survives_stale_settings_and_preserves_desktop_preferences() {
        use crate::models::config::MenuBarQuotaScope;
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        let mut stale = load_config_at(&path).unwrap();
        assert_eq!(stale.menu_bar.quota_scope, MenuBarQuotaScope::All);
        set_desktop_preferences_at(&path, &DesktopPreferences {
            launch_at_login: true, hide_dock_icon: true, start_minimized: true,
        }).unwrap();
        set_menu_bar_preferences_at(&path, MenuBarQuotaScope::Gemini).unwrap();
        stale.language = "en".into();
        save_config_at(&path, &stale).unwrap();
        let saved = load_config_at(&path).unwrap();
        assert_eq!(saved.menu_bar.quota_scope, MenuBarQuotaScope::Gemini);
        assert!(saved.desktop.launch_at_login);
        assert_eq!(saved.language, "en");
        set_menu_bar_preferences_at(&path, MenuBarQuotaScope::Other).unwrap();
        set_desktop_preferences_at(&path, &DesktopPreferences::default()).unwrap();
        assert_eq!(load_config_at(&path).unwrap().menu_bar.quota_scope, MenuBarQuotaScope::Other);
    }

    #[test]
    fn new_configuration_persists_defaults_without_recursive_locking() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        let config = load_config_at(&path).unwrap();
        assert!(!config.desktop.launch_at_login);
        assert!(path.is_file());
        assert!(!load_config_at(&path).unwrap().desktop.launch_at_login);
    }

    #[test]
    fn malformed_configuration_is_not_overwritten_by_any_writer() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        let original = b"{\"theme\": broken existing data";
        fs::write(&path, original).unwrap();
        assert!(load_config_at(&path)
            .unwrap_err()
            .starts_with("failed_to_parse_config_file:"));
        assert!(save_config_at(&path, &AppConfig::new())
            .unwrap_err()
            .starts_with("failed_to_parse_config_file:"));
        assert!(set_desktop_preferences_at(&path, &DesktopPreferences::default())
            .unwrap_err()
            .starts_with("failed_to_parse_config_file:"));
        assert_eq!(fs::read(&path).unwrap(), original);
    }

    #[test]
    fn unreadable_configuration_is_not_treated_as_a_missing_file() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        fs::create_dir(&path).unwrap();
        fs::write(path.join("existing"), b"preserve").unwrap();
        assert!(load_config_at(&path)
            .unwrap_err()
            .starts_with("failed_to_read_config_file:"));
        assert!(save_config_at(&path, &AppConfig::new())
            .unwrap_err()
            .starts_with("failed_to_read_config_file:"));
        assert!(set_desktop_preferences_at(&path, &DesktopPreferences::default())
            .unwrap_err()
            .starts_with("failed_to_read_config_file:"));
        assert_eq!(fs::read(path.join("existing")).unwrap(), b"preserve");
    }

    #[test]
    fn ordinary_first_save_cannot_enable_desktop_preferences() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        let mut proposed = AppConfig::new();
        proposed.desktop = DesktopPreferences {
            launch_at_login: true,
            hide_dock_icon: true,
            start_minimized: true,
        };
        save_config_at(&path, &proposed).unwrap();
        let actual = load_config_at(&path).unwrap();
        assert!(!actual.desktop.launch_at_login);
        assert!(!actual.desktop.hide_dock_icon);
        assert!(!actual.desktop.start_minimized);
    }

    #[test]
    fn desktop_preferences_survive_stale_ordinary_writes() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        let mut stale = load_config_at(&path).unwrap();
        let desktop = DesktopPreferences {
            launch_at_login: true,
            hide_dock_icon: true,
            start_minimized: true,
        };
        set_desktop_preferences_at(&path, &desktop).unwrap();
        stale.theme = "dark".into();
        save_config_at(&path, &stale).unwrap();
        let actual = load_config_at(&path).unwrap();
        assert!(actual.desktop.launch_at_login);
        assert!(actual.desktop.hide_dock_icon);
        assert!(actual.desktop.start_minimized);
        assert_eq!(actual.theme, "dark");

        let mut stale = actual;
        set_desktop_preferences_at(&path, &DesktopPreferences::default()).unwrap();
        stale.language = "en".into();
        save_config_at(&path, &stale).unwrap();
        let actual = load_config_at(&path).unwrap();
        assert!(!actual.desktop.launch_at_login);
        assert!(!actual.desktop.hide_dock_icon);
        assert!(!actual.desktop.start_minimized);
        assert_eq!(actual.theme, "dark");
        assert_eq!(actual.language, "en");
    }

    #[test]
    fn desktop_writer_refuses_to_overwrite_malformed_configuration() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        let original = b"{broken existing config";
        fs::write(&path, original).unwrap();
        assert!(set_desktop_preferences_at(&path, &DesktopPreferences::default()).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
    }

    #[test]
    fn concurrent_desktop_and_ordinary_writers_preserve_final_opt_outs() {
        use std::sync::{Arc, Barrier};
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(CONFIG_FILE);
        let enabled = DesktopPreferences {
            launch_at_login: true,
            hide_dock_icon: true,
            start_minimized: true,
        };
        set_desktop_preferences_at(&path, &enabled).unwrap();
        let mut stale = load_config_at(&path).unwrap();
        stale.theme = "dark".into();
        let start = Arc::new(Barrier::new(2));
        std::thread::scope(|scope| {
            let ordinary_path = path.clone();
            let ordinary_start = start.clone();
            scope.spawn(move || {
                ordinary_start.wait();
                for _ in 0..16 {
                    save_config_at(&ordinary_path, &stale).unwrap();
                }
            });
            start.wait();
            for _ in 0..16 {
                set_desktop_preferences_at(&path, &enabled).unwrap();
                set_desktop_preferences_at(&path, &DesktopPreferences::default()).unwrap();
            }
        });
        let actual = load_config_at(&path).unwrap();
        assert!(!actual.desktop.launch_at_login);
        assert!(!actual.desktop.hide_dock_icon);
        assert!(!actual.desktop.start_minimized);
        assert_eq!(actual.theme, "dark");
    }
}
