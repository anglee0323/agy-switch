use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaPool { Gemini, ThirdParty, Argon }

/// Canonical bucket IDs take precedence over translated or abbreviated labels.
/// A model-specific Gemini bucket must never become the shared Gemini pool.
pub fn quota_pool(bucket_id: &str, group_name: &str) -> Option<QuotaPool> {
    let id = bucket_id.trim().to_ascii_lowercase();
    match id.as_str() {
        "gemini-5h" | "gemini-weekly" => return Some(QuotaPool::Gemini),
        "3p-5h" | "3p-weekly" => return Some(QuotaPool::ThirdParty),
        "gemini-argon-wtu-weekly" => return Some(QuotaPool::Argon),
        _ => {}
    }
    if id.starts_with("gemini-") || id.starts_with("3p-") { return None; }
    match group_name.trim().to_ascii_lowercase().as_str() {
        "gemini" | "gemini models" => Some(QuotaPool::Gemini),
        "claude" | "claude / gpt" | "claude/gpt" | "claude/gpt models" | "claude and gpt models" => Some(QuotaPool::ThirdParty),
        "gemini 4 argon" => Some(QuotaPool::Argon),
        _ => None,
    }
}

pub fn model_quota_pool(name: &str, display_name: Option<&str>) -> Option<QuotaPool> {
    let lower = name.to_ascii_lowercase();
    if lower.starts_with("gemini") {
        let argon = [name, display_name.unwrap_or("")].iter().any(|value|
            value.split(|c: char| !c.is_ascii_alphanumeric()).any(|word| word.eq_ignore_ascii_case("argon")));
        Some(if argon { QuotaPool::Argon } else { QuotaPool::Gemini })
    } else if lower.starts_with("claude") || lower.starts_with("gpt") { Some(QuotaPool::ThirdParty) }
    else { None }
}

pub fn quota_group_label(name: &str) -> &str {
    match quota_pool("", name) {
        Some(QuotaPool::Gemini) => "Gemini",
        Some(QuotaPool::ThirdParty) => "Claude/GPT",
        _ => name,
    }
}

/// 单个配额桶 (对应 retrieveUserQuotaSummary 里的一个 bucket)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaBucket {
    /// 桶 ID,如 "gemini-weekly" / "gemini-5h" / "3p-weekly" / "3p-5h"
    pub bucket_id: String,
    /// 窗口类型: "weekly" / "5h"
    pub window: String,
    /// 剩余比例 0.0-1.0
    pub remaining_fraction: f64,
    /// None: legacy cache; false: API omitted/invalid value. Used by read-only views.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining_fraction_known: Option<bool>,
    /// 重置时间 (RFC3339)
    pub reset_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// 一个模型组 (如 Gemini Models / Claude and GPT models)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaGroup {
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub buckets: Vec<QuotaBucket>,
}

/// 模型配额信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelQuota {
    pub name: String,
    pub percentage: i32, // Legacy policy value; read-only views must consult percentage_known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percentage_known: Option<bool>,
    /// Exact valid API observation for read-only views; never used by policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_remaining_fraction: Option<f64>,
    /// Bucket association inferred by the existing fusion rule, not an API pool ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota_bucket_id: Option<String>,
    pub reset_time: String,

    // -- 动态参数解析与持久化 --
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_images: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_thinking: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_mime_types: Option<std::collections::HashMap<String, bool>>,
}

/// 配额数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaData {
    pub models: Vec<ModelQuota>,
    pub last_updated: i64,
    #[serde(default)]
    pub is_forbidden: bool,
    /// 禁止访问的原因 (403 详细信息)
    #[serde(default)]
    pub forbidden_reason: Option<String>,
    /// 订阅等级 (FREE/PRO/ULTRA)
    #[serde(default)]
    pub subscription_tier: Option<String>,
    /// 模型淘汰重定向规则表 (old_model_id -> new_model_id)
    #[serde(default)]
    pub model_forwarding_rules: std::collections::HashMap<String, String>,
    /// 按模型组的配额摘要 (weekly + 5h 双窗口),来自 retrieveUserQuotaSummary
    #[serde(default)]
    pub quota_groups: Option<Vec<QuotaGroup>>,
}

impl QuotaData {
    pub fn new() -> Self {
        Self {
            models: Vec::new(),
            last_updated: chrono::Utc::now().timestamp(),
            is_forbidden: false,
            forbidden_reason: None,
            subscription_tier: None,
            model_forwarding_rules: std::collections::HashMap::new(),
            quota_groups: None,
        }
    }

    pub fn add_model(&mut self, model: ModelQuota) {
        self.models.push(model);
    }
}

impl Default for QuotaData {
    fn default() -> Self {
        Self::new()
    }
}

pub fn fraction_is_known(value: Option<f64>) -> bool {
    value.is_some_and(|v| v.is_finite() && (0.0..=1.0).contains(&v))
}

pub fn observed_quota_percentage(value: f64, known: Option<bool>) -> Option<i32> {
    (known != Some(false) && (known.is_some() || value != 0.0) && fraction_is_known(Some(value)))
        .then(|| (value * 100.0).round() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pool_identity_matches_the_shared_frontend_fixture() {
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!("../../../tests/fixtures/quota-pool-cases.json")).unwrap();
        for case in cases {
            let actual = quota_pool(case["id"].as_str().unwrap(), case["name"].as_str().unwrap());
            let actual = actual.map(|pool| match pool { QuotaPool::Gemini => "gemini", QuotaPool::ThirdParty => "other", QuotaPool::Argon => "argon" });
            assert_eq!(actual, case["pool"].as_str(), "{case}");
        }
        assert_eq!(quota_group_label("Gemini 4 Argon"), "Gemini 4 Argon");
        assert_eq!(model_quota_pool("gemini-4-argon-high", None), Some(QuotaPool::Argon));
        assert_eq!(model_quota_pool("gemini-test", Some("Gemini 4 Argon High")), Some(QuotaPool::Argon));
    }
}
