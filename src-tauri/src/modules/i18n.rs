use serde_json::Value;
use std::collections::HashMap;

/// Choose the first supported OS language when creating a new configuration.
/// Saved configurations keep their explicit language selection.
pub fn default_language() -> String {
    language_from_locales(sys_locale::get_locales()).to_string()
}

fn language_from_locales(locales: impl IntoIterator<Item = impl AsRef<str>>) -> &'static str {
    locales
        .into_iter()
        .find_map(|locale| supported_language(locale.as_ref()))
        .unwrap_or("en")
}

/// Normalize OS locale tags to the identifiers shared by the UI and tray menu.
fn supported_language(locale: &str) -> Option<&'static str> {
    let locale = locale
        .split(['.', '@'])
        .next()?
        .replace('_', "-")
        .to_ascii_lowercase();
    match locale.split('-').next()? {
        "zh" => Some("zh"),
        "en" => Some("en"),
        _ => None,
    }
}

/// Tray text structure
#[derive(Debug, Clone)]
pub struct TrayTexts {
    pub current: String,
    pub quota: String,
    pub switch_next: String,
    pub refresh_current: String,
    pub show_window: String,
    pub quit: String,
    pub no_account: String,
    pub unknown_quota: String,
    pub forbidden: String,
    pub identity_checking: String,
    pub identity_unavailable: String,
}

/// Load translations from JSON
fn load_translations(lang: &str) -> HashMap<String, String> {
    let json_content = match lang.split(['-', '_']).next().unwrap_or(lang) {
        "zh" => include_str!("../../../src/locales/zh.json"),
        "en" => include_str!("../../../src/locales/en.json"),
        _ => include_str!("../../../src/locales/zh.json"),
    };

    let v: Value = serde_json::from_str(json_content).unwrap_or_else(|_| serde_json::json!({}));

    let mut map = HashMap::new();

    if let Some(tray) = v.get("tray").and_then(|t| t.as_object()) {
        for (key, value) in tray {
            if let Some(s) = value.as_str() {
                map.insert(key.clone(), s.to_string());
            }
        }
    }

    map
}

/// Get tray texts (based on language)
pub fn get_tray_texts(lang: &str) -> TrayTexts {
    let t = load_translations(lang);

    TrayTexts {
        current: t
            .get("current")
            .cloned()
            .unwrap_or_else(|| "Current".to_string()),
        quota: t
            .get("quota")
            .cloned()
            .unwrap_or_else(|| "Quota".to_string()),
        switch_next: t
            .get("switch_next")
            .cloned()
            .unwrap_or_else(|| "Switch to Next Account".to_string()),
        refresh_current: t
            .get("refresh_current")
            .cloned()
            .unwrap_or_else(|| "Refresh Current Quota".to_string()),
        show_window: t
            .get("show_window")
            .cloned()
            .unwrap_or_else(|| "Show Main Window".to_string()),
        quit: t
            .get("quit")
            .cloned()
            .unwrap_or_else(|| "Quit Application".to_string()),
        no_account: t
            .get("no_account")
            .cloned()
            .unwrap_or_else(|| "No Account".to_string()),
        unknown_quota: t
            .get("unknown_quota")
            .cloned()
            .unwrap_or_else(|| "Unknown".to_string()),
        forbidden: t
            .get("forbidden")
            .cloned()
            .unwrap_or_else(|| "Account Forbidden".to_string()),
        identity_checking: t.get("identity_checking").cloned().unwrap_or_else(|| "Checking".into()),
        identity_unavailable: t.get("identity_unavailable").cloned().unwrap_or_else(|| "Current unknown".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::{get_tray_texts, language_from_locales};

    #[test]
    fn detects_only_english_and_simplified_chinese() {
        for (locale, expected) in [
            ("en-US", "en"),
            ("en-GB", "en"),
            ("zh-CN", "zh"),
            ("zh-SG", "zh"),
            ("zh-Hans-TW", "zh"),
            ("zh-TW", "zh"),
        ] {
            assert_eq!(language_from_locales([locale]), expected, "{locale}");
        }
    }

    #[test]
    fn skips_unsupported_languages_and_falls_back_to_english() {
        assert_eq!(language_from_locales(["de-DE", "ru-RU", "en-US"]), "en");
        assert_eq!(language_from_locales(["en-GB", "zh-CN"]), "en");
        assert_eq!(language_from_locales(["de-DE", "zh-TW"]), "zh");
        assert_eq!(language_from_locales(Vec::<String>::new()), "en");
        for locale in ["", "C", "POSIX", "C.UTF-8", "de-DE", "my-MM"] {
            assert_eq!(language_from_locales([locale]), "en", "{locale}");
        }
    }

    #[test]
    fn tray_uses_the_selected_language() {
        let zh_texts = get_tray_texts("zh");
        let zh: serde_json::Value =
            serde_json::from_str(include_str!("../../../src/locales/zh.json")).unwrap();
        assert_eq!(zh_texts.quit, zh["tray"]["quit"].as_str().unwrap());

        let en_texts = get_tray_texts("en");
        let en: serde_json::Value =
            serde_json::from_str(include_str!("../../../src/locales/en.json")).unwrap();
        assert_eq!(en_texts.quit, en["tray"]["quit"].as_str().unwrap());
    }
}
