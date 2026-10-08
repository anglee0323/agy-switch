//! Allowlisted preferences shared by Antigravity App and agy CLI. Use the App's
//! patch RPC; never replace config.json or change hooks/plugins/migration flags.
use serde_json::{json, Value};
use std::io::Read;
const MAX_CONFIG: u64 = 1024 * 1024;
pub(crate) const FIELDS: &[&str] = &[
    "permissionPreset",
    "artifactReviewMode",
    "nonWorkspaceFileAccessPolicy",
    "internetAccessPolicy",
    "browserJsExecutionPolicy",
    "conversationWidth",
    "verboseAgentChat",
    "queuedMessageDeliveryStrategy",
    "useAiCredits",
    "globalPermissionGrants",
];

fn enumeration(key: &str) -> Option<(&'static [&'static str], i64)> {
    match key {
        "permissionPreset" => Some((&["UNSPECIFIED", "DEFAULT", "REQUEST_REVIEW", "TURBO"], 1)),
        "artifactReviewMode" => Some((&["UNSPECIFIED", "ALWAYS", "TURBO", "AUTO"], 3)),
        "nonWorkspaceFileAccessPolicy" => Some((&["UNSPECIFIED", "ALLOW", "ASK", "DENY"], 2)),
        "internetAccessPolicy" => Some((&["UNSPECIFIED", "ALLOW", "ASK", "DENY"], 1)),
        "browserJsExecutionPolicy" => Some((
            &[
                "UNSPECIFIED",
                "DISABLED",
                "ALWAYS_ASK",
                "MODEL_DECIDES",
                "TURBO",
            ],
            2,
        )),
        "conversationWidth" => Some((&["UNSPECIFIED", "DEFAULT", "NARROW", "WIDE"], 1)),
        "queuedMessageDeliveryStrategy" => {
            Some((&["UNSPECIFIED", "NEXT_INVOCATION", "WHEN_IDLE"], 2))
        }
        _ => None,
    }
}
fn normalize(key: &str, raw: Option<&Value>) -> Value {
    if let Some((names, default)) = enumeration(key) {
        let value = raw.and_then(|v| {
            v.as_i64().or_else(|| {
                v.as_str().and_then(|s| {
                    names
                        .iter()
                        .position(|name| s == *name || s.ends_with(&format!("_{name}")))
                        .map(|i| i as i64)
                })
            })
        });
        return value.map(Value::from).unwrap_or_else(|| {
            if raw.is_some() {
                Value::Null
            } else {
                Value::from(default)
            }
        });
    }
    match key {
        "verboseAgentChat" => raw.cloned().unwrap_or(Value::Bool(true)),
        "useAiCredits" => raw.cloned().unwrap_or(Value::Bool(false)),
        "globalPermissionGrants" => raw
            .cloned()
            .unwrap_or(json!({"allow":[],"ask":[],"deny":[]})),
        _ => Value::Null,
    }
}
fn project(config: &Value) -> Result<Value, String> {
    let settings = config["userSettings"]
        .as_object()
        .ok_or("app_settings_invalid")?;
    let mut result = serde_json::Map::new();
    for key in FIELDS {
        result.insert((*key).into(), normalize(key, settings.get(*key)));
    }
    if !settings.contains_key("permissionPreset") {
        result.insert(
            "permissionPreset".into(),
            json!(match settings["autoExecutionPolicy"].as_str() {
                Some("CASCADE_COMMANDS_AUTO_EXECUTION_EAGER") => 3,
                Some("CASCADE_COMMANDS_AUTO_EXECUTION_OFF") => 2,
                _ => 1,
            }),
        );
    }
    Ok(Value::Object(result))
}
pub(crate) fn current() -> Result<Value, String> {
    let path = dirs::home_dir()
        .ok_or("home_unavailable")?
        .join(".gemini/config/config.json");
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| "app_settings_unavailable")?
        .take(MAX_CONFIG + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "app_settings_unavailable")?;
    if bytes.len() as u64 > MAX_CONFIG {
        return Err("app_settings_invalid".into());
    }
    let config: Value = serde_json::from_slice(&bytes).map_err(|_| "app_settings_invalid")?;
    project(&config)
}
pub(crate) fn recommended() -> Value {
    json!({"permissionPreset":3,"artifactReviewMode":2,"nonWorkspaceFileAccessPolicy":1,"internetAccessPolicy":1,
        "browserJsExecutionPolicy":4,"conversationWidth":3,"verboseAgentChat":true,"queuedMessageDeliveryStrategy":1,"useAiCredits":false})
}
pub(crate) fn recommended_patch(current: &Value) -> Value {
    let mut result = recommended();
    let mut grants = current["globalPermissionGrants"].clone();
    for (kind, rules) in [
        (
            "ask",
            vec!["command(git push *--force*)", "command(git push *-f*)"],
        ),
        ("deny", vec!["command(rm -rf /)", "command(rm -rf /*)"]),
    ] {
        let items = grants[kind].as_array_mut();
        if let Some(items) = items {
            for rule in rules {
                if !items.iter().any(|v| v.as_str() == Some(rule)) {
                    items.push(json!(rule));
                }
            }
        }
    }
    result["globalPermissionGrants"] = grants;
    result
}
fn validate(patch: &Value) -> Result<(), String> {
    let object = patch
        .as_object()
        .filter(|o| !o.is_empty() && o.len() <= FIELDS.len())
        .ok_or("app_settings_patch_invalid")?;
    for (key, value) in object {
        if !FIELDS.contains(&key.as_str()) {
            return Err("app_settings_patch_invalid".into());
        }
        if let Some((names, _)) = enumeration(key) {
            if !value
                .as_u64()
                .is_some_and(|n| n > 0 && (n as usize) < names.len())
            {
                return Err("app_settings_patch_invalid".into());
            }
        } else if key == "globalPermissionGrants" {
            let grants = value
                .as_object()
                .filter(|o| o.len() == 3)
                .ok_or("app_permission_rules_invalid")?;
            for kind in ["allow", "ask", "deny"] {
                if !grants
                    .get(kind)
                    .and_then(Value::as_array)
                    .is_some_and(|items| {
                        items.len() <= 500
                            && items.iter().all(|v| {
                                v.as_str().is_some_and(|s| {
                                    !s.is_empty()
                                        && s.len() <= 2048
                                        && !s.chars().any(char::is_control)
                                })
                            })
                    })
                {
                    return Err("app_permission_rules_invalid".into());
                }
            }
        } else if !value.is_boolean() {
            return Err("app_settings_patch_invalid".into());
        }
    }
    Ok(())
}
pub(crate) fn write(patch: Value) -> Result<Value, String> {
    validate(&patch)?;
    #[cfg(not(target_os = "macos"))]
    {
        Err("unsupported_platform".into())
    }
    #[cfg(target_os = "macos")]
    {
        let mut wire = patch.clone();
        if let Some(preset) = wire["permissionPreset"].as_i64() {
            wire["autoExecutionPolicy"] = json!(match preset {
                2 => 1,
                3 => 3,
                _ => 4,
            });
            wire["enableTerminalSandbox"] = json!(preset == 1);
        }
        super::app_identity::with_running_connection(super::config::load_app_config()?.antigravity_executable.as_deref(),|port,csrf| {
        let client=reqwest::blocking::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(3)).build().map_err(|_| "app_settings_write_failed")?;
        let response=client.post(format!("http://127.0.0.1:{port}/exa.language_server_pb.LanguageServerService/JetboxWriteState"))
            .header("content-type","application/json").header("x-codeium-csrf-token",csrf)
            .body(json!({"userConfig":{"userSettings":wire}}).to_string()).send().map_err(|_| "app_settings_write_failed")?;
        if !response.status().is_success() {return Err("app_settings_write_failed".into());}
        Ok(())
    })?.ok_or("not_running")?;
        // Confirmation is a reread of the client's own saved preferences, not an
        // optimistic copy of the patch. Unknown/mismatched values remain explicit.
        let current = current()?;
        if patch
            .as_object()
            .unwrap()
            .iter()
            .any(|(key, v)| current[key] != *v)
        {
            return Err("app_settings_not_confirmed".into());
        }
        Ok(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_normalizes_known_values_without_exposing_other_configuration() {
        let value=project(&json!({"plugins":{"personal":"preserve"},"userSettings":{"artifactReviewMode":"ARTIFACT_REVIEW_MODE_TURBO","autoExecutionPolicy":"CASCADE_COMMANDS_AUTO_EXECUTION_EAGER","permissionGrantsV2Migrated":true,"apiKey":"secret"}})).unwrap();
        assert_eq!(value["permissionPreset"], 3);
        assert_eq!(value["artifactReviewMode"], 2);
        assert!(value.get("apiKey").is_none());
        assert!(value.get("permissionGrantsV2Migrated").is_none());
    }
    #[test]
    fn partial_patches_cannot_overwrite_plugins_or_migration_markers() {
        for value in [
            json!({"plugins":{}}),
            json!({"permissionGrantsV2Migrated":false}),
            json!({"artifactReviewMode":100}),
            json!({"globalPermissionGrants":{"allow":[]}}),
        ] {
            assert!(validate(&value).is_err());
        }
        validate(&recommended()).unwrap();
        let current = json!({"globalPermissionGrants":{"allow":["read_url(https://example.invalid/*)"],"ask":[],"deny":["command(custom denied command)"]}});
        let patch = recommended_patch(&current);
        validate(&patch).unwrap();
        assert_eq!(
            patch["globalPermissionGrants"]["allow"],
            current["globalPermissionGrants"]["allow"]
        );
        assert_eq!(
            patch["globalPermissionGrants"]["deny"][0],
            "command(custom denied command)"
        );
        assert_eq!(recommended_patch(&patch), patch);
    }
}
