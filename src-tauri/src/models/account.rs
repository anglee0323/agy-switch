use super::{quota::QuotaData, token::TokenData};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// 账号数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub token: TokenData,
    /// 可选的设备指纹，用于切换账号时固定机器信息
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_profile: Option<DeviceProfile>,
    pub quota: Option<QuotaData>,
    /// Marks accounts with invalid or revoked credentials.
    #[serde(default)]
    pub disabled: bool,
    /// Optional human-readable reason for disabling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,
    /// Unix timestamp when the account was disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_at: Option<i64>,
    /// 受配额保护禁用的模型列表 [NEW #621]
    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub protected_models: HashSet<String>,
    /// [NEW] 403 验证阻止状态 (VALIDATION_REQUIRED)
    #[serde(default)]
    pub validation_blocked: bool,
    /// [NEW] 验证阻止截止时间戳
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_blocked_until: Option<i64>,
    /// [NEW] 验证阻止原因
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_blocked_reason: Option<String>,
    /// [NEW] 验证链接 URL (#1522)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_url: Option<String>,
    pub created_at: i64,
    pub last_used: i64,
    /// 用户自定义标签
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_label: Option<String>,
}

impl Account {
    pub fn new(id: String, email: String, token: TokenData) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id,
            email,
            name: None,
            token,
            device_profile: None,
            quota: None,
            disabled: false,
            disabled_reason: None,
            disabled_at: None,
            protected_models: HashSet::new(),
            validation_blocked: false,
            validation_blocked_until: None,
            validation_blocked_reason: None,
            validation_url: None,
            created_at: now,
            last_used: now,
            custom_label: None,
        }
    }

    pub fn update_last_used(&mut self) {
        self.last_used = chrono::Utc::now().timestamp();
    }

    pub fn update_quota(&mut self, mut quota: QuotaData) {
        if let Some(ref existing) = self.quota {
            if quota.subscription_tier.is_none() {
                quota.subscription_tier = existing.subscription_tier.clone();
            }
        }
        self.quota = Some(quota);
    }
}

/// 账号索引数据（accounts.json）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountIndex {
    pub version: String,
    pub accounts: Vec<AccountSummary>,
    pub current_account_id: Option<String>,
    #[serde(default)]
    pub current_target_ide: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_ide_account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_vscode_account_id: Option<String>,
}

/// 账号摘要信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSummary {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    #[serde(default)]
    pub disabled: bool,
    /// 受保护的模型列表 [NEW] 供 UI 显示锁定图标
    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub protected_models: HashSet<String>,
    pub created_at: i64,
    pub last_used: i64,
}

impl AccountIndex {
    pub fn new() -> Self {
        Self {
            version: "2.0".to_string(),
            accounts: Vec::new(),
            current_account_id: None,
            current_target_ide: None,
            current_ide_account_id: None,
            current_vscode_account_id: None,
        }
    }

    pub fn selected(&self, target: Option<&str>) -> Option<String> {
        match target {
            Some("ide") => self.current_ide_account_id.clone().or_else(|| {
                (self.current_target_ide.as_deref() == Some("ide")).then(|| self.current_account_id.clone()).flatten()
            }),
            Some("vscode") => self.current_vscode_account_id.clone().or_else(|| {
                (self.current_target_ide.as_deref() == Some("vscode")).then(|| self.current_account_id.clone()).flatten()
            }),
            _ if matches!(self.current_target_ide.as_deref(), Some("ide" | "vscode")) => None,
            _ => self.current_account_id.clone(),
        }
    }

    pub fn record_selected(&mut self, id: &str, target: Option<&str>) {
        // Migrate the legacy single selection before an App switch replaces it.
        if self.current_ide_account_id.is_none() && self.current_target_ide.as_deref() == Some("ide") {
            self.current_ide_account_id = self.current_account_id.clone();
        }
        if self.current_vscode_account_id.is_none() && self.current_target_ide.as_deref() == Some("vscode") {
            self.current_vscode_account_id = self.current_account_id.clone();
        }
        match target {
            Some("ide") => self.current_ide_account_id = Some(id.into()),
            Some("vscode") => self.current_vscode_account_id = Some(id.into()),
            _ => {
                self.current_account_id = Some(id.into());
                self.current_target_ide = target.map(str::to_owned);
                if target == Some("all") {
                    self.current_ide_account_id = Some(id.into());
                    self.current_vscode_account_id = Some(id.into());
                }
            }
        }
    }
}

impl Default for AccountIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// 设备指纹（storage.json 中 telemetry 相关字段）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceProfile {
    pub machine_id: String,
    pub mac_machine_id: String,
    pub dev_device_id: String,
    pub sqm_id: String,
}

/// 导出账号项（用于备份/迁移）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountExportItem {
    #[serde(default)]
    pub email: String,
    pub refresh_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth_client_key: Option<String>,
    #[serde(default)]
    pub is_gcp_tos: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_label: Option<String>,
}

/// 导出账号响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountExportResponse {
    #[serde(default)]
    pub schema_version: u32,
    pub accounts: Vec<AccountExportItem>,
}

#[cfg(test)]
mod selection_tests {
    use super::*;

    #[test]
    fn app_and_ide_selections_do_not_overwrite_each_other() {
        let mut index = AccountIndex::new();
        index.record_selected("A", Some("app"));
        index.record_selected("B", Some("ide"));
        assert_eq!(index.selected(None).as_deref(), Some("A"));
        assert_eq!(index.selected(Some("ide")).as_deref(), Some("B"));
        index.record_selected("C", Some("app"));
        assert_eq!(index.selected(Some("ide")).as_deref(), Some("B"));
    }

    #[test]
    fn legacy_ide_record_is_preserved_without_claiming_app_identity() {
        let mut index: AccountIndex = serde_json::from_str(r#"{"version":"2.0","accounts":[],"current_account_id":"B","current_target_ide":"ide"}"#).unwrap();
        assert_eq!(index.selected(Some("ide")).as_deref(), Some("B"));
        assert!(index.selected(Some("app")).is_none());
        index.record_selected("A", Some("app"));
        assert_eq!(index.selected(Some("ide")).as_deref(), Some("B"));
        index.record_selected("C", Some("all"));
        assert_eq!(index.selected(Some("ide")).as_deref(), Some("C"));
    }
}
