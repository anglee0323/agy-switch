use super::{CliError, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

/// An allow-list, deliberately independent of the credential-bearing Account model.
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct AccountView {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub custom_label: Option<String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub validation_blocked: bool,
    #[serde(default)]
    pub last_used: i64,
    #[serde(default)]
    pub is_current: bool,
    pub quota: Option<QuotaView>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct QuotaView {
    pub last_updated: i64,
    pub subscription_tier: Option<String>,
    #[serde(default)]
    pub is_forbidden: bool,
    #[serde(default)]
    pub models: Vec<ModelView>,
    pub quota_groups: Option<Vec<GroupView>>,
}
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct ModelView {
    pub(super) name: String,
    pub(super) percentage: i32,
    pub(super) reset_time: String,
}
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct GroupView {
    pub(super) display_name: String,
    pub(super) buckets: Vec<BucketView>,
}
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct BucketView {
    pub(super) bucket_id: String,
    pub(super) window: String,
    pub(super) remaining_fraction: f64,
    pub(super) reset_time: String,
}

#[derive(Deserialize)]
struct Index {
    accounts: Vec<Summary>,
    current_account_id: Option<String>,
    current_target_ide: Option<String>,
}
#[derive(Deserialize)]
struct Summary {
    id: String,
}

pub(super) struct Snapshot {
    pub accounts: Vec<AccountView>,
    pub current_target: Option<String>,
}

impl Snapshot {
    pub fn read(root: &Path) -> Result<Self> {
        let bytes = match fs::read(root.join("accounts.json")) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match fs::read_dir(root.join("accounts")) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Ok(mut entries) => {
                        if entries.next().is_some() {
                            return Err(CliError::data(
                                "Account index is missing. Open the agy-switch GUI to recover it.",
                            ));
                        }
                    }
                    _ => {
                        return Err(CliError::data(
                            "Account index is missing. Open the agy-switch GUI to recover it.",
                        ))
                    }
                }
                return Ok(Self {
                    accounts: vec![],
                    current_target: None,
                });
            }
            Err(_) => {
                return Err(CliError::data(
                    "Cannot read the account index. Check data-directory permissions.",
                ))
            }
        };
        let content = std::str::from_utf8(&bytes).map_err(|_| {
            CliError::data("Account index is invalid. Open the agy-switch GUI to recover it.")
        })?;
        let index: Index = serde_json::from_str(content.trim_start_matches(['\u{feff}', '\0']))
            .map_err(|_| {
                CliError::data("Account index is invalid. Open the agy-switch GUI to recover it.")
            })?;
        let mut accounts = vec![];
        for summary in index.accounts {
            // IDs are normally UUIDs. Do not allow a corrupted index to read outside accounts/.
            if summary.id.is_empty()
                || !summary
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
            {
                return Err(CliError::data(
                    "Account index contains an invalid account ID.",
                ));
            }
            if accounts
                .iter()
                .any(|account: &AccountView| account.id == summary.id)
            {
                return Err(CliError::data(
                    "Account index contains duplicate account IDs.",
                ));
            }
            let bytes = fs::read(root.join("accounts").join(format!("{}.json", summary.id)))
                .map_err(|_| {
                    CliError::data(
                        "An indexed account cannot be read. Open the agy-switch GUI to inspect it.",
                    )
                })?;
            let mut account: AccountView = serde_json::from_slice(&bytes).map_err(|_| {
                CliError::data("An account file is invalid. Open the agy-switch GUI to inspect it.")
            })?;
            if account.id != summary.id {
                return Err(CliError::data(
                    "An account ID does not match its index entry.",
                ));
            }
            account.is_current = index.current_account_id.as_ref() == Some(&account.id);
            accounts.push(account);
        }
        Ok(Self {
            accounts,
            current_target: index.current_target_ide.map(|target| {
                if target == "agy" {
                    "cli".into()
                } else {
                    target
                }
            }),
        })
    }
    pub fn current(&self) -> Result<&AccountView> {
        self.accounts
            .iter()
            .find(|account| account.is_current)
            .ok_or_else(CliError::missing)
    }
    pub fn select(&self, selector: &str) -> Result<&AccountView> {
        if let Some(account) = self.accounts.iter().find(|account| account.id == selector) {
            return Ok(account);
        }
        let mut matches = self
            .accounts
            .iter()
            .filter(|account| account.email.eq_ignore_ascii_case(selector));
        let account = matches.next().ok_or_else(CliError::missing)?;
        if matches.next().is_some() {
            return Err(CliError {
                code: 2,
                message: "More than one account matches this email. Use an exact account ID.",
            });
        }
        Ok(account)
    }
}

pub(super) fn terminal_text(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect()
}
impl AccountView {
    pub fn line(&self) -> String {
        format!(
            "{} {}  {}{}",
            if self.is_current { "*" } else { " " },
            terminal_text(&self.id),
            terminal_text(&self.email),
            if self.disabled {
                " [disabled]"
            } else if self.validation_blocked {
                " [verification required]"
            } else {
                ""
            }
        )
    }
}
impl QuotaView {
    pub fn human(&self, email: &str) -> String {
        let mut lines = vec![format!(
            "{}: cached quota (updated Unix {})",
            terminal_text(email),
            self.last_updated
        )];
        if let Some(tier) = &self.subscription_tier {
            lines.push(format!("Plan: {}", terminal_text(tier)));
        }
        if self.is_forbidden {
            lines.push("Quota access is forbidden; cached figures may be stale.".into());
        }
        for group in self.quota_groups.iter().flatten() {
            lines.push(terminal_text(&group.display_name));
            for bucket in &group.buckets {
                lines.push(format!(
                    "  {} ({}): {:.0}% remaining; resets {}",
                    terminal_text(&bucket.bucket_id),
                    terminal_text(&bucket.window),
                    bucket.remaining_fraction * 100.0,
                    terminal_text(&bucket.reset_time)
                ));
            }
        }
        for model in &self.models {
            lines.push(format!(
                "  {}: {}% remaining; resets {}",
                terminal_text(&model.name),
                model.percentage,
                terminal_text(&model.reset_time)
            ));
        }
        if self.models.is_empty() && self.quota_groups.as_ref().is_none_or(Vec::is_empty) {
            lines.push("No model quota recorded.".into());
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(root: &Path) {
        fs::create_dir_all(root.join("accounts")).unwrap();
        fs::write(
            root.join("accounts.json"),
            r#"{"accounts":[{"id":"acc-1"}],"current_account_id":"acc-1"}"#,
        )
        .unwrap();
        fs::write(root.join("accounts/acc-1.json"), r#"{"id":"acc-1","email":"test@example.invalid","token":{"refresh_token":"SECRET-REFRESH","access_token":"SECRET-ACCESS"},"validation_url":"SECRET-URL","disabled_reason":"SECRET-REASON","quota":{"models":[],"last_updated":123,"forbidden_reason":"SECRET-SERVER"}}"#).unwrap();
    }
    #[test]
    fn reads_do_not_create_or_recover_files() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("absent");
        assert!(Snapshot::read(&missing).unwrap().accounts.is_empty());
        assert!(!missing.exists());
        fixture(temp.path());
        fs::write(temp.path().join("accounts.json"), "corrupt").unwrap();
        assert!(Snapshot::read(temp.path()).is_err());
        assert_eq!(
            fs::read_to_string(temp.path().join("accounts.json")).unwrap(),
            "corrupt"
        );
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 2);
    }
    #[test]
    fn output_excludes_all_credential_and_error_fields() {
        let temp = tempfile::tempdir().unwrap();
        fixture(temp.path());
        let before = fs::read(temp.path().join("accounts/acc-1.json")).unwrap();
        let snapshot = Snapshot::read(temp.path()).unwrap();
        let account = snapshot.current().unwrap();
        assert_eq!(snapshot.select("TEST@example.invalid").unwrap().id, "acc-1");
        let output = serde_json::to_string(account).unwrap();
        assert!(!output.contains("SECRET"));
        assert!(!output.contains("token"));
        assert_eq!(
            before,
            fs::read(temp.path().join("accounts/acc-1.json")).unwrap()
        );
        assert_eq!(snapshot.accounts.len(), 1);
    }
    #[test]
    fn rejects_paths_in_index_and_ambiguous_email() {
        let temp = tempfile::tempdir().unwrap();
        fixture(temp.path());
        fs::write(
            temp.path().join("accounts.json"),
            r#"{"accounts":[{"id":"../outside"}]}"#,
        )
        .unwrap();
        assert!(Snapshot::read(temp.path()).is_err());
        fixture(temp.path());
        let mut snapshot = Snapshot::read(temp.path()).unwrap();
        let copy: AccountView = serde_json::from_value(
            serde_json::json!({"id":"acc-2","email":"test@example.invalid"}),
        )
        .unwrap();
        snapshot.accounts.push(copy);
        assert_eq!(snapshot.select("test@example.invalid").unwrap_err().code, 2);
        assert_eq!(snapshot.select("acc-1").unwrap().id, "acc-1");
    }
    #[test]
    fn removes_terminal_escape_control_characters() {
        assert_eq!(terminal_text("user\x1b[31m\nname"), "user [31m name");
    }
}
