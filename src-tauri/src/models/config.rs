use serde::{Deserialize, Serialize};

/// Application configuration for the account and quota dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub dashboard: DashboardPreferences,
    pub desktop: DesktopPreferences,
    pub menu_bar: MenuBarPreferences,
    pub language: String,
    pub theme: String,
    pub check_updates_on_startup: bool,
    pub auto_refresh: bool,
    pub refresh_interval: i32,
    pub auto_sync: bool,
    pub sync_interval: i32,
    pub antigravity_executable: Option<String>,
    pub antigravity_ide_executable: Option<String>,
    pub antigravity_args: Option<Vec<String>>,
    pub quota_protection: QuotaProtectionConfig,
    pub pinned_quota_models: PinnedQuotaModelsConfig,
}

/// Stable card identifiers used by the homepage and settings editor.
const DEFAULT_DASHBOARD_CARDS: [&str; 10] = [
    "total_tokens", "input_tokens", "output_tokens", "cache_hit_rate", "api_cost",
    "first_text_latency", "body_speed", "account_status", "aggregate_quota", "quota_reset",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DashboardPreferences {
    /// Selected cards in display order. An explicit empty list hides all cards.
    pub cards: Vec<String>,
}
impl Default for DashboardPreferences {
    fn default() -> Self {
        Self { cards: DEFAULT_DASHBOARD_CARDS.iter().map(|id| (*id).to_string()).collect() }
    }
}
impl DashboardPreferences {
    pub fn normalize(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.cards.retain(|id| DEFAULT_DASHBOARD_CARDS.contains(&id.as_str()) && seen.insert(id.clone()));
    }
}

/// Preferences are opt-in and migrate safely from older config files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DesktopPreferences {
    pub launch_at_login: bool,
    pub hide_dock_icon: bool,
    pub start_minimized: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuBarQuotaScope {
    #[default]
    All,
    Gemini,
    Other,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuBarLabelStyle { #[default] EmailThenLabel, LabelThenEmail, EmailOnly }

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuBarResetTimeDisplay { Hidden, #[default] Hover, Always }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MenuBarPreferences {
    pub quota_scope: MenuBarQuotaScope,
    pub display_scope: MenuBarQuotaScope,
    pub hide_unavailable: bool,
    pub label_style: MenuBarLabelStyle,
    pub show_aggregate: bool,
    pub show_session: bool,
    pub show_weekly: bool,
    pub show_icons: bool,
    pub show_reset_on_hover: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_time_display: Option<MenuBarResetTimeDisplay>,
    pub green_above: u8,
    pub red_below: u8,
}
impl Default for MenuBarPreferences {
    fn default() -> Self { Self { quota_scope: MenuBarQuotaScope::All, display_scope: MenuBarQuotaScope::All,
        hide_unavailable: true, label_style: MenuBarLabelStyle::EmailThenLabel,
        show_aggregate: true, show_session: true, show_weekly: true, show_icons: true, show_reset_on_hover: true, reset_time_display: None, green_above: 60, red_below: 20 } }
}
impl MenuBarPreferences {
    pub fn reset_time_mode(&self) -> MenuBarResetTimeDisplay {
        self.reset_time_display.unwrap_or(if self.show_reset_on_hover { MenuBarResetTimeDisplay::Hover } else { MenuBarResetTimeDisplay::Hidden })
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MenuBarPreferencesPatch {
    pub quota_scope: Option<MenuBarQuotaScope>, pub display_scope: Option<MenuBarQuotaScope>,
    pub hide_unavailable: Option<bool>, pub label_style: Option<MenuBarLabelStyle>,
    pub show_aggregate: Option<bool>, pub show_session: Option<bool>, pub show_weekly: Option<bool>, pub show_icons: Option<bool>,
    pub show_reset_on_hover: Option<bool>,
    pub reset_time_display: Option<MenuBarResetTimeDisplay>,
    pub green_above: Option<u8>, pub red_below: Option<u8>,
}
impl MenuBarPreferencesPatch {
    pub fn apply(self, preferences: &mut MenuBarPreferences) -> Result<(), String> {
        macro_rules! apply { ($($field:ident),*) => { $(if let Some(value) = self.$field { preferences.$field = value; })* }; }
        apply!(quota_scope, display_scope, hide_unavailable, label_style, show_aggregate, show_session, show_weekly, show_icons, show_reset_on_hover, green_above, red_below);
        if let Some(mode) = self.reset_time_display { preferences.reset_time_display = Some(mode); }
        if preferences.red_below >= preferences.green_above || preferences.green_above > 100 {
            return Err("Choose color thresholds with 0 ≤ red < green ≤ 100.".into());
        }
        if !preferences.show_session && !preferences.show_weekly { return Err("Show at least one quota window.".into()); }
        Ok(())
    }
}

/// Quota protection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaProtectionConfig {
    /// Whether quota protection is enabled
    pub enabled: bool,

    /// Reserved quota percentage (1-99)
    pub threshold_percentage: u32,

    /// List of monitored models (e.g. gemini-3-flash, gemini-3-pro-high, gemini-3.1-pro-high, claude-sonnet-4-6)
    #[serde(default = "default_monitored_models")]
    pub monitored_models: Vec<String>,
}

fn default_monitored_models() -> Vec<String> {
    vec![
        "claude".to_string(),
        "gemini-3-pro-high".to_string(),
        "gemini-3-flash".to_string(),
        "gemini-3.1-flash-image".to_string(),
    ]
}

impl QuotaProtectionConfig {
    pub fn new() -> Self {
        Self {
            enabled: false,
            threshold_percentage: 10, // Default 10% reserve
            monitored_models: default_monitored_models(),
        }
    }
}

impl Default for QuotaProtectionConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Pinned quota models configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinnedQuotaModelsConfig {
    /// List of pinned models (displayed outside the account list)
    #[serde(default = "default_pinned_models")]
    pub models: Vec<String>,
}

fn default_pinned_models() -> Vec<String> {
    vec![
        "gemini-3-pro-high".to_string(),
        "gemini-3-flash".to_string(),
        "gemini-3.1-flash-image".to_string(),
        "claude-sonnet-4-6-thinking".to_string(),
    ]
}

impl PinnedQuotaModelsConfig {
    pub fn new() -> Self {
        Self {
            models: default_pinned_models(),
        }
    }
}

impl Default for PinnedQuotaModelsConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl AppConfig {
    pub fn new() -> Self {
        Self {
            dashboard: DashboardPreferences::default(),
            desktop: DesktopPreferences::default(),
            menu_bar: MenuBarPreferences::default(),
            language: crate::modules::i18n::default_language(),
            theme: "system".to_string(),
            check_updates_on_startup: true,
            auto_refresh: true,
            refresh_interval: 15,
            auto_sync: false,
            sync_interval: 5,
            antigravity_executable: None,
            antigravity_ide_executable: None,
            antigravity_args: None,
            quota_protection: QuotaProtectionConfig::default(),
            pinned_quota_models: PinnedQuotaModelsConfig::default(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn legacy_dashboard_defaults_to_all_cards_without_overriding_empty_selection() {
        let old: AppConfig = serde_json::from_str(r#"{"language":"zh"}"#).unwrap();
        assert_eq!(old.dashboard.cards.len(), 10);
        let empty: AppConfig = serde_json::from_str(r#"{"dashboard":{"cards":[]}}"#).unwrap();
        assert!(empty.dashboard.cards.is_empty());
        let restored: AppConfig = serde_json::from_str(&serde_json::to_string(&empty).unwrap()).unwrap();
        assert!(restored.dashboard.cards.is_empty());
    }

    #[test]
    fn dashboard_expansion_preserves_explicit_selection_and_new_card_order() {
        let mut config: AppConfig = serde_json::from_str(r#"{"dashboard":{"cards":["total_tokens","cache_hit_rate","first_text_latency","body_speed","api_cost"]}}"#).unwrap();
        config.dashboard.normalize();
        assert_eq!(config.dashboard.cards, ["total_tokens", "cache_hit_rate", "first_text_latency", "body_speed", "api_cost"]);
        config.dashboard.cards = ["quota_reset", "account_status", "aggregate_quota", "quota_reset", "unknown"].map(String::from).to_vec();
        config.dashboard.normalize();
        assert_eq!(config.dashboard.cards, ["quota_reset", "account_status", "aggregate_quota"]);
    }

    #[test]
    fn desktop_preferences_are_opt_in_for_new_and_legacy_configs() {
        for config in [
            AppConfig::new(),
            serde_json::from_str::<AppConfig>(r#"{"language":"zh"}"#).unwrap(),
        ] {
            assert!(!config.desktop.launch_at_login);
            assert!(!config.desktop.hide_dock_icon);
            assert!(!config.desktop.start_minimized);
        }
    }

    #[test]
    fn partial_desktop_config_keeps_new_fields_disabled() {
        let config: AppConfig =
            serde_json::from_str(r#"{"desktop":{"hide_dock_icon":true}}"#).unwrap();
        assert!(config.desktop.hide_dock_icon);
        assert!(!config.desktop.launch_at_login);
        assert!(!config.desktop.start_minimized);
    }

    #[test]
    fn removed_localization_preference_is_ignored_in_legacy_config() {
        let saved: AppConfig =
            serde_json::from_str(r#"{"language":"en","app_localization":{"enabled":true},"desktop":{"launch_at_login":true}}"#)
                .unwrap();
        assert_eq!(saved.language, "en");
        assert!(saved.desktop.launch_at_login);
        assert!(serde_json::to_value(saved).unwrap().get("app_localization").is_none());
    }

    #[test]
    fn saved_language_is_preserved_when_loading_config() {
        let mut config = AppConfig::new();
        for language in ["en", "zh"] {
            config.language = language.to_string();
            let saved = serde_json::to_string(&config).unwrap();
            let restored: AppConfig = serde_json::from_str(&saved).unwrap();
            assert_eq!(restored.language, language);
        }
    }
}
