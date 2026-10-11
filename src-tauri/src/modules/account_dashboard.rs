//! Read-only dashboard contract. No recovery, refresh, activation, or token deserialization.
use crate::models::{AccountIndex, QuotaData};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Deserialize)]
struct SavedView {
    id: String,
    email: String,
    name: Option<String>,
    custom_label: Option<String>,
    quota: Option<QuotaData>,
    #[serde(default)]
    disabled: bool,
    #[serde(default)]
    validation_blocked: bool,
    validation_blocked_until: Option<i64>,
    #[serde(default)]
    protected_models: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DashboardSnapshot {
    pub indexed_total: usize,
    pub loaded_count: usize,
    pub failed_count: usize,
    pub current_account_id: Option<String>,
    pub current_identity_source: &'static str,
    pub accounts: Vec<DashboardEntry>,
}
#[derive(Debug, Serialize)]
pub struct DashboardEntry {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub custom_label: Option<String>,
    pub read_status: &'static str,
    pub read_error: Option<&'static str>,
    pub disabled: bool,
    pub validation_blocked: bool,
    pub validation_blocked_until: Option<i64>,
    pub protected_models: Vec<String>,
    pub quota: Option<ReadOnlyQuota>,
}
#[derive(Debug, Serialize)]
pub struct ReadOnlyQuota {
    pub last_updated: i64,
    pub is_forbidden: bool,
    pub subscription_tier: Option<String>,
    pub provenance: &'static str,
    pub models: Vec<ReadOnlyModel>,
    pub groups: Option<Vec<ReadOnlyGroup>>,
}
#[derive(Debug, Serialize)]
pub struct ReadOnlyModel {
    pub name: String,
    pub display_name: Option<String>,
    pub percentage: Option<f64>,
    pub reset_time: String,
    pub inferred_bucket_id: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct ReadOnlyGroup {
    pub display_name: String,
    pub buckets: Vec<ReadOnlyBucket>,
}
#[derive(Debug, Serialize)]
pub struct ReadOnlyBucket {
    pub bucket_id: String,
    pub window: String,
    pub remaining_fraction: Option<f64>,
    pub reset_time: String,
}
/// A live identity supersedes the saved Tools index. Ambiguous and failed
/// observations never select an arbitrary account; a closed App retains it.
pub fn apply_running_identity(snapshot: &mut DashboardSnapshot, identity: Result<Option<String>, String>) {
    match identity {
        Ok(Some(email)) => {
            let mut matches = snapshot.accounts.iter().filter(|account|
                account.read_status == "loaded" && account.email.eq_ignore_ascii_case(&email));
            let first = matches.next();
            snapshot.current_account_id = if matches.next().is_none() {
                first.map(|account| account.id.clone())
            } else { None };
            snapshot.current_identity_source = "running_app";
        }
        Ok(None) => {}
        Err(_) => {
            snapshot.current_account_id = None;
            snapshot.current_identity_source = "unavailable";
        }
    }
}
fn observed(value: f64, known: Option<bool>, maximum: f64) -> Option<f64> {
    if known == Some(false)
        || (known.is_none() && value == 0.0)
        || !value.is_finite()
        || !(0.0..=maximum).contains(&value)
    {
        None
    } else {
        Some(value)
    }
}
impl From<QuotaData> for ReadOnlyQuota {
    fn from(q: QuotaData) -> Self {
        let legacy = q
            .models
            .iter()
            .any(|m| m.observed_remaining_fraction.is_none())
            || q.quota_groups.iter().flatten().any(|g| {
                g.buckets
                    .iter()
                    .any(|b| b.remaining_fraction_known.is_none())
            });
        Self {
            last_updated: q.last_updated,
            is_forbidden: q.is_forbidden,
            subscription_tier: q.subscription_tier,
            provenance: if legacy { "legacy_cache" } else { "observed" },
            models: q
                .models
                .into_iter()
                .map(|m| ReadOnlyModel {
                    percentage: match m.observed_remaining_fraction {
                        Some(fraction) => {
                            observed(fraction, m.percentage_known, 1.0).map(|f| f * 100.0)
                        }
                        // A known legacy integer zero may still be a truncated positive fraction.
                        None => observed(
                            m.percentage as f64,
                            if m.percentage == 0 {
                                None
                            } else {
                                m.percentage_known
                            },
                            100.0,
                        ),
                    },
                    name: m.name,
                    display_name: m.display_name,
                    reset_time: m.reset_time,
                    inferred_bucket_id: m.quota_bucket_id,
                })
                .collect(),
            groups: q.quota_groups.map(|groups| {
                groups
                    .into_iter()
                    .map(|g| ReadOnlyGroup {
                        display_name: g.display_name,
                        buckets: g
                            .buckets
                            .into_iter()
                            .map(|b| ReadOnlyBucket {
                                remaining_fraction: observed(
                                    b.remaining_fraction,
                                    b.remaining_fraction_known,
                                    1.0,
                                ),
                                bucket_id: b.bucket_id,
                                window: b.window,
                                reset_time: b.reset_time,
                            })
                            .collect(),
                    })
                    .collect()
            }),
        }
    }
}
fn read_regular(path: &Path) -> Result<Vec<u8>, &'static str> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "file_unreadable")?;
    if !metadata.is_file() {
        return Err("not_regular_file");
    }
    fs::read(path).map_err(|_| "file_unreadable")
}
/// Does not create the data directory, repair indexes or rewrite account files.
pub fn snapshot_in_dir(data_dir: &Path) -> Result<DashboardSnapshot, String> {
    let bytes = read_regular(&data_dir.join("accounts.json")).map_err(|e| format!("index_{e}"))?;
    let index: AccountIndex = serde_json::from_slice(&bytes).map_err(|_| "index_invalid_json")?;
    let current_account_id = index.selected(Some("app"));
    let mut entries = Vec::new();
    let mut loaded = 0;
    for summary in index.accounts {
        let valid_id = !summary.id.is_empty()
            && summary
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_');
        let result = if valid_id {
            read_regular(
                &data_dir
                    .join("accounts")
                    .join(format!("{}.json", summary.id)),
            )
            .and_then(|bytes| {
                serde_json::from_slice::<SavedView>(&bytes).map_err(|_| "invalid_json")
            })
            .and_then(|view| {
                if view.id == summary.id {
                    Ok(view)
                } else {
                    Err("identity_mismatch")
                }
            })
        } else {
            Err("invalid_account_id")
        };
        let mut entry = DashboardEntry {
            id: summary.id,
            email: summary.email,
            name: summary.name,
            custom_label: None,
            read_status: "failed",
            read_error: None,
            disabled: summary.disabled,
            validation_blocked: false,
            validation_blocked_until: None,
            protected_models: summary.protected_models.into_iter().collect(),
            quota: None,
        };
        match result {
            Ok(view) => {
                loaded += 1;
                entry.email = view.email;
                entry.name = view.name;
                entry.custom_label = view.custom_label;
                entry.read_status = "loaded";
                entry.disabled = view.disabled;
                entry.validation_blocked = view.validation_blocked;
                entry.validation_blocked_until = view.validation_blocked_until;
                entry.protected_models = view.protected_models;
                entry.quota = view.quota.map(ReadOnlyQuota::from);
            }
            Err(code) => entry.read_error = Some(code),
        }
        entries.push(entry);
    }
    Ok(DashboardSnapshot {
        indexed_total: entries.len(),
        loaded_count: loaded,
        failed_count: entries.len() - loaded,
        current_account_id,
        current_identity_source: "tools_record",
        accounts: entries,
    })
}
pub fn snapshot() -> Result<DashboardSnapshot, String> {
    let data_dir = std::env::var("ABV_DATA_DIR")
        .ok()
        .filter(|p| !p.trim().is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".antigravity_tools")))
        .ok_or("home_directory_unavailable")?;
    snapshot_in_dir(&data_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::quota::fraction_is_known;
    use serde_json::json;
    #[test]
    fn live_identity_overrides_index_and_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("accounts")).unwrap();
        let index = json!({"version":"2.0", "current_account_id":"a", "accounts":[
            {"id":"a","email":"a@example.test","created_at":1,"last_used":1},
            {"id":"b","email":"b@example.test","created_at":1,"last_used":1}]});
        fs::write(dir.path().join("accounts.json"), index.to_string()).unwrap();
        for id in ["a", "b"] {
            fs::write(dir.path().join(format!("accounts/{id}.json")),
                json!({"id":id,"email":format!("{id}@example.test")}).to_string()).unwrap();
        }
        let mut view = snapshot_in_dir(dir.path()).unwrap();
        apply_running_identity(&mut view, Ok(None));
        assert_eq!(view.current_account_id.as_deref(), Some("a"));
        apply_running_identity(&mut view, Ok(Some("B@example.test".into())));
        assert_eq!(view.current_account_id.as_deref(), Some("b"));
        assert_eq!(view.current_identity_source, "running_app");
        apply_running_identity(&mut view, Ok(Some("unknown@example.test".into())));
        assert_eq!(view.current_account_id, None);
        view.accounts[0].email = "b@example.test".into();
        apply_running_identity(&mut view, Ok(Some("b@example.test".into())));
        assert_eq!(view.current_account_id, None);
        apply_running_identity(&mut view, Err("unverified".into()));
        assert_eq!(view.current_identity_source, "unavailable");
        assert_eq!(view.current_account_id, None);
        assert_eq!(fs::read_to_string(dir.path().join("accounts.json")).unwrap(), index.to_string());
    }
    #[test]
    fn dashboard_unknown_and_real_zero_are_distinct() {
        assert_eq!(observed(0.0, None, 100.0), None);
        assert_eq!(observed(0.0, Some(false), 100.0), None);
        assert_eq!(observed(0.0, Some(true), 100.0), Some(0.0));
        assert_eq!(observed(50.0, None, 100.0), Some(50.0));
        for f in [None, Some(-0.1), Some(1.1), Some(f64::NAN)] {
            assert!(!fraction_is_known(f));
        }
        assert!(fraction_is_known(Some(0.0)));
    }
    #[test]
    fn dashboard_legacy_and_observed_cache_projection() {
        let model = json!({"name":"gemini-test","percentage":0,"reset_time":""});
        let bucket =
            json!({"bucket_id":"shared-5h","window":"5h","remaining_fraction":0.0,"reset_time":""});
        let mut value = json!({"models":[model],"last_updated":1,"quota_groups":[{"display_name":"Shared","buckets":[bucket]}]});
        let legacy =
            ReadOnlyQuota::from(serde_json::from_value::<QuotaData>(value.clone()).unwrap());
        assert_eq!(legacy.provenance, "legacy_cache");
        assert_eq!(legacy.models[0].percentage, None);
        assert_eq!(
            legacy.groups.unwrap()[0].buckets[0].remaining_fraction,
            None
        );
        value["models"][0]["percentage_known"] = json!(true);
        value["models"][0]["quota_bucket_id"] = json!("shared-5h");
        value["models"][0]["observed_remaining_fraction"] = json!(0.0);
        value["quota_groups"][0]["buckets"][0]["remaining_fraction_known"] = json!(true);
        let explicit =
            ReadOnlyQuota::from(serde_json::from_value::<QuotaData>(value.clone()).unwrap());
        assert_eq!(explicit.provenance, "observed");
        assert_eq!(explicit.models[0].percentage, Some(0.0));
        assert_eq!(
            explicit.models[0].inferred_bucket_id.as_deref(),
            Some("shared-5h")
        );
        assert_eq!(
            explicit.groups.unwrap()[0].buckets[0].remaining_fraction,
            Some(0.0)
        );
        value["models"][0]["percentage_known"] = json!(false);
        value["quota_groups"][0]["buckets"][0]["remaining_fraction_known"] = json!(false);
        let missing = ReadOnlyQuota::from(serde_json::from_value::<QuotaData>(value).unwrap());
        assert_eq!(missing.models[0].percentage, None);
        assert_eq!(
            missing.groups.unwrap()[0].buckets[0].remaining_fraction,
            None
        );
    }
    #[test]
    fn dashboard_fraction_precision_survives_model_truncation_and_group_rounding() {
        for fraction in [0.009_f64, 0.004_f64] {
            for legacy_percentage in [(fraction * 100.0) as i32, (fraction * 100.0).round() as i32]
            {
                let value = json!({"models":[{"name":"model","percentage":legacy_percentage,
                    "percentage_known":true,"observed_remaining_fraction":fraction,"reset_time":""}],
                    "last_updated":1});
                let projected =
                    ReadOnlyQuota::from(serde_json::from_value::<QuotaData>(value).unwrap());
                assert!((projected.models[0].percentage.unwrap() - fraction * 100.0).abs() < 1e-12);
                assert!(projected.models[0].percentage.unwrap() > 0.0);
            }
        }
        let legacy_zero = json!({"models":[{"name":"model","percentage":0,"percentage_known":true,"reset_time":""}],"last_updated":1});
        let projected =
            ReadOnlyQuota::from(serde_json::from_value::<QuotaData>(legacy_zero).unwrap());
        assert_eq!(projected.models[0].percentage, None);
        assert_eq!(projected.provenance, "legacy_cache");
    }
    #[test]
    fn dashboard_preserves_failures_and_never_heals_or_returns_tokens() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("accounts")).unwrap();
        let ids = ["ok", "missing", "broken", "trailing", "../outside"];
        let index = json!({"version":"2.0", "current_account_id":"missing", "accounts":ids.map(|id| json!({"id":id,"email":"synthetic@example.test","name":null,"created_at":1,"last_used":1}))});
        fs::write(dir.path().join("accounts.json"), index.to_string()).unwrap();
        let account = json!({"id":"ok","email":"synthetic@example.test","token":{"access_token":"fixture-secret","refresh_token":"fixture-secret"}}).to_string();
        fs::write(dir.path().join("accounts/ok.json"), &account).unwrap();
        fs::write(dir.path().join("accounts/broken.json"), "broken").unwrap();
        let trailing = account.replace("\"ok\"", "\"trailing\"") + "garbage";
        fs::write(dir.path().join("accounts/trailing.json"), &trailing).unwrap();
        let snapshot = snapshot_in_dir(dir.path()).unwrap();
        assert_eq!(
            (
                snapshot.indexed_total,
                snapshot.loaded_count,
                snapshot.failed_count
            ),
            (5, 1, 4)
        );
        assert_eq!(snapshot.current_identity_source, "tools_record");
        assert_eq!(snapshot.current_account_id.as_deref(), Some("missing"));
        let output = serde_json::to_string(&snapshot).unwrap();
        assert!(!output.contains("token"));
        assert!(!output.contains("fixture-secret"));
        assert_eq!(
            fs::read_to_string(dir.path().join("accounts/trailing.json")).unwrap(),
            trailing
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("accounts.json")).unwrap(),
            index.to_string()
        );
    }
    #[test]
    fn dashboard_missing_or_corrupt_index_is_not_an_empty_account_set() {
        let dir = tempfile::tempdir().unwrap();
        assert!(snapshot_in_dir(dir.path()).is_err());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        fs::write(dir.path().join("accounts.json"), "invalid").unwrap();
        assert!(snapshot_in_dir(dir.path()).is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join("accounts.json")).unwrap(),
            "invalid"
        );
    }
}
