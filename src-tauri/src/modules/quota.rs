use crate::models::QuotaData;
use rquest;
use serde::{Deserialize, Serialize};
use serde_json::json;

// Quota API endpoints (fallback order: Sandbox → Daily → Prod)
const QUOTA_API_ENDPOINTS: [&str; 3] = [
    "https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal:fetchAvailableModels",
    "https://daily-cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels",
    "https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels",
];

// Production serves the complete shared quota; daily can return HTTP 200 with only 3p.
const QUOTA_SUMMARY_ENDPOINTS: [&str; 3] = [
    "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary",
    "https://daily-cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary",
    "https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal:retrieveUserQuotaSummary",
];

/// Critical retry threshold: considered near recovery when quota reaches 95%

#[derive(Debug, Serialize, Deserialize)]
struct QuotaResponse {
    models: std::collections::HashMap<String, ModelInfo>,
    #[serde(rename = "deprecatedModelIds")]
    deprecated_model_ids: Option<std::collections::HashMap<String, DeprecatedModelInfo>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DeprecatedModelInfo {
    #[serde(rename = "newModelId")]
    new_model_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ModelInfo {
    #[serde(rename = "quotaInfo")]
    quota_info: Option<QuotaInfo>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "supportsImages")]
    supports_images: Option<bool>,
    #[serde(rename = "supportsThinking")]
    supports_thinking: Option<bool>,
    #[serde(rename = "thinkingBudget")]
    thinking_budget: Option<i32>,
    recommended: Option<bool>,
    #[serde(rename = "maxTokens")]
    max_tokens: Option<i32>,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: Option<i32>,
    #[serde(rename = "supportedMimeTypes")]
    supported_mime_types: Option<std::collections::HashMap<String, bool>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct QuotaInfo {
    #[serde(rename = "remainingFraction")]
    remaining_fraction: Option<f64>,
    #[serde(rename = "resetTime")]
    reset_time: Option<String>,
}

// ---- retrieveUserQuotaSummary 响应反序列化结构 ----

#[derive(Debug, Deserialize)]
struct QuotaSummaryResponse {
    groups: Vec<QuotaSummaryGroup>,
}

#[derive(Debug, Deserialize)]
struct QuotaSummaryGroup {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    description: Option<String>,
    buckets: Vec<QuotaSummaryBucket>,
}

#[derive(Debug, Deserialize)]
struct QuotaSummaryBucket {
    #[serde(rename = "bucketId")]
    bucket_id: Option<String>,
    window: Option<String>,
    #[serde(rename = "remainingFraction")]
    remaining_fraction: Option<f64>,
    #[serde(rename = "resetTime")]
    reset_time: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LoadProjectResponse {
    #[serde(rename = "cloudaicompanionProject")]
    project_id: Option<String>,
    #[serde(rename = "currentTier")]
    current_tier: Option<Tier>,
    #[serde(rename = "paidTier")]
    paid_tier: Option<Tier>,
    #[serde(rename = "allowedTiers")]
    allowed_tiers: Option<Vec<Tier>>,
    #[serde(rename = "ineligibleTiers")]
    ineligible_tiers: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
struct Tier {
    is_default: Option<bool>,
    id: Option<String>,
    name: Option<String>,
}

/// Get shared HTTP Client (15s timeout) for quota and project requests.
async fn create_standard_client() -> Result<rquest::Client, String> {
    crate::utils::http::get_standard_client()
}

const CLOUD_CODE_LOAD_PROJECT_ENDPOINTS: [&str; 3] = [
    "https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal:loadCodeAssist",
    "https://daily-cloudcode-pa.googleapis.com/v1internal:loadCodeAssist",
    "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist",
];

/// Fetch project ID and subscription tier
async fn fetch_project_id(access_token: &str, email: &str) -> (Option<String>, Option<String>) {
    let Ok(client) = create_standard_client().await else { return (None, None); };
    let meta = json!({"metadata": {"ideType": "ANTIGRAVITY"}});

    for (ep_idx, ep_url) in CLOUD_CODE_LOAD_PROJECT_ENDPOINTS.iter().enumerate() {
        let res = client
            .post(*ep_url)
            .header(
                rquest::header::AUTHORIZATION,
                format!("Bearer {}", access_token),
            )
            .header(rquest::header::CONTENT_TYPE, "application/json")
            .header(
                rquest::header::USER_AGENT,
                crate::constants::NATIVE_OAUTH_USER_AGENT.as_str(),
            )
            .json(&meta)
            .send()
            .await;

        match res {
            Ok(res) => {
                if res.status().is_success() {
                    if let Ok(data) = res.json::<LoadProjectResponse>().await {
                        let project_id = data.project_id.clone();

                        // Core logic: Multi-level fallback for tier extraction
                        // 1. Paid Tier (Google One AI Premium etc.)
                        // 2. Current Tier (If not ineligible)
                        // 3. Allowed tiers (restricted/default account access)
                        let mut subscription_tier = data
                            .paid_tier
                            .as_ref()
                            .and_then(|t| t.name.clone())
                            .or_else(|| data.paid_tier.as_ref().and_then(|t| t.id.clone()));

                        let is_ineligible = data.ineligible_tiers.is_some()
                            && !data.ineligible_tiers.as_ref().unwrap().is_empty();

                        if subscription_tier.is_none() {
                            if !is_ineligible {
                                subscription_tier = data
                                    .current_tier
                                    .as_ref()
                                    .and_then(|t| t.name.clone())
                                    .or_else(|| {
                                        data.current_tier.as_ref().and_then(|t| t.id.clone())
                                    });
                            } else {
                                // If account is marked as INELIGIBLE, drop to allowedTiers and extract default
                                if let Some(mut allowed) = data.allowed_tiers {
                                    if let Some(default_tier) =
                                        allowed.iter_mut().find(|t| t.is_default == Some(true))
                                    {
                                        if let Some(name) = &default_tier.name {
                                            subscription_tier =
                                                Some(format!("{} (Restricted)", name));
                                        } else if let Some(id) = &default_tier.id {
                                            subscription_tier =
                                                Some(format!("{} (Restricted)", id));
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(ref tier) = subscription_tier {
                            crate::modules::logger::log_info(&format!(
                                "📊 [{}] Subscription identified successfully: {}",
                                email, tier
                            ));
                        }

                        if ep_idx > 0 {
                            crate::modules::logger::log_info(&format!(
                                "loadCodeAssist fallback succeeded at endpoint #{}",
                                ep_idx + 1
                            ));
                        }

                        return (project_id, subscription_tier);
                    }
                } else {
                    crate::modules::logger::log_warn(&format!(
                        "⚠️  [{}] loadCodeAssist failed at {}: Status: {}",
                        email,
                        ep_url,
                        res.status()
                    ));
                    continue;
                }
            }
            Err(e) => {
                crate::modules::logger::log_error(&format!(
                    "❌ [{}] loadCodeAssist network error at {}: {}",
                    email, ep_url, e
                ));
                continue;
            }
        }
    }

    (None, None)
}

/// Unified entry point for fetching account quota
pub async fn fetch_quota(
    access_token: &str,
    email: &str,
) -> crate::error::AppResult<(QuotaData, Option<String>)> {
    fetch_quota_with_cache(access_token, email, None).await
}

/// Fetch quota with cache support
pub async fn fetch_quota_with_cache(
    access_token: &str,
    email: &str,
    cached_project_id: Option<&str>,
) -> crate::error::AppResult<(QuotaData, Option<String>)> {
    use crate::error::AppError;

    // Optimization: Skip loadCodeAssist call if project_id is cached to save API quota
    let (project_id, subscription_tier) = if let Some(pid) = cached_project_id {
        (Some(pid.to_string()), None)
    } else {
        fetch_project_id(access_token, email).await
    };

    // We keep project_id to store in the DB, but we NO LONGER force inject it into payload if it's absent

    let client = create_standard_client().await.map_err(crate::error::AppError::Config)?;
    let payload = if let Some(ref pid) = project_id {
        json!({ "project": pid })
    } else {
        json!({}) // Empty payload fallback
    };

    let mut last_error: Option<AppError> = None;

    for (ep_idx, ep_url) in QUOTA_API_ENDPOINTS.iter().enumerate() {
        let has_next = ep_idx + 1 < QUOTA_API_ENDPOINTS.len();

        let mut current_payload = payload.clone();
        let mut retry_without_project = false;

        loop {
            match client
                .post(*ep_url)
                .bearer_auth(access_token)
                .header(
                    rquest::header::USER_AGENT,
                    crate::constants::NATIVE_OAUTH_USER_AGENT.as_str(),
                )
                .json(&current_payload)
                .send()
                .await
            {
                Ok(response) => {
                    // Convert HTTP error status to AppError
                    if let Err(_) = response.error_for_status_ref() {
                        let status = response.status();

                        // [FIX] 403 Forbidden 处理：如果是带有 project_id 的请求，尝试剥离后重试
                        if status == rquest::StatusCode::FORBIDDEN {
                            if current_payload.get("project").is_some() && !retry_without_project {
                                crate::modules::logger::log_warn(&format!(
                                    "Quota fetch got 403 with project ID, retrying without project ID..."
                                ));
                                current_payload = json!({});
                                retry_without_project = true;
                                continue;
                            }

                            crate::modules::logger::log_warn(&format!(
                                "Account unauthorized (403 Forbidden), marking as forbidden"
                            ));
                            let mut q = QuotaData::new();
                            q.is_forbidden = true;
                            q.subscription_tier = subscription_tier.clone();
                            return Ok((q, project_id.clone()));
                        }

                        let text = response.text().await.unwrap_or_default();

                        // 429/5xx: fallback to next endpoint
                        if has_next
                            && (status == rquest::StatusCode::TOO_MANY_REQUESTS
                                || status.is_server_error())
                        {
                            crate::modules::logger::log_warn(&format!(
                                "Quota API {} returned {}, falling back to next endpoint",
                                ep_url, status
                            ));
                            last_error =
                                Some(AppError::Unknown(format!("HTTP {} - {}", status, text)));
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            break; // Break the inner retry loop, continue to next endpoint
                        }

                        return Err(AppError::Unknown(format!(
                            "API Error: {} - {}",
                            status, text
                        )));
                    }

                    if ep_idx > 0 {
                        crate::modules::logger::log_info(&format!(
                            "Quota API fallback succeeded at endpoint #{}",
                            ep_idx + 1
                        ));
                    }

                    let quota_response: QuotaResponse =
                        response.json().await.map_err(AppError::from)?;

                    let mut quota_data = QuotaData::new();

                    // Use debug level for detailed info to avoid console noise
                    tracing::debug!("Quota API returned {} models", quota_response.models.len());

                    for (name, info) in quota_response.models {
                        if let Some(quota_info) = info.quota_info {
                            let percentage = quota_info
                                .remaining_fraction
                                .map(|f| (f * 100.0) as i32)
                                .unwrap_or(0);

                            let reset_time = quota_info.reset_time.clone().unwrap_or_default();

                            // Only keep models we care about (exclude internal chat models)
                            if name.starts_with("gemini")
                                || name.starts_with("claude")
                                || name.starts_with("gpt")
                                || name.starts_with("image")
                                || name.starts_with("imagen")
                            {
                                let model_quota = crate::models::quota::ModelQuota {
                                    name,
                                    percentage,
                                    percentage_known: Some(
                                        crate::models::quota::fraction_is_known(quota_info.remaining_fraction),
                                    ),
                                    observed_remaining_fraction: quota_info.remaining_fraction
                                        .filter(|f| crate::models::quota::fraction_is_known(Some(*f))),
                                    quota_bucket_id: None,
                                    reset_time,
                                    display_name: info.display_name,
                                    supports_images: info.supports_images,
                                    supports_thinking: info.supports_thinking,
                                    thinking_budget: info.thinking_budget,
                                    recommended: info.recommended,
                                    max_tokens: info.max_tokens,
                                    max_output_tokens: info.max_output_tokens,
                                    supported_mime_types: info.supported_mime_types,
                                };
                                quota_data.add_model(model_quota);
                            }
                        }
                    }

                    // Parse deprecated model routing rules
                    if let Some(deprecated) = quota_response.deprecated_model_ids {
                        for (old_id, info) in deprecated {
                            // Register forwarding rules (including those mapping to gemini-pro-agent)
                            quota_data
                                .model_forwarding_rules
                                .insert(old_id, info.new_model_id);
                        }
                    }

                    // Set subscription tier
                    quota_data.subscription_tier = subscription_tier.clone();

                    // Best-effort: fetch grouped quota summary (weekly + 5h windows).
                    // Failure here must not block the primary quota result.
                    let quota_groups =
                        fetch_quota_summary(access_token, email, project_id.as_deref()).await;

                    if let Some(ref groups) = quota_groups {
                        fuse_model_quotas(&mut quota_data.models, groups);
                    }

                    quota_data.quota_groups = quota_groups;

                    return Ok((quota_data, project_id.clone()));
                }
                Err(e) => {
                    crate::modules::logger::log_warn(&format!(
                        "Quota API request failed at {}: {}",
                        ep_url, e
                    ));
                    last_error = Some(AppError::from(e));
                    if has_next {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                    break; // Break the inner retry loop on network error, continue to next endpoint
                }
            }
        } // End of inner loop
    }

    Err(last_error.unwrap_or_else(|| {
        AppError::Unknown("Quota fetch failed: all endpoints exhausted".to_string())
    }))
}

/// Fetch grouped quota summary (weekly + 5h windows) via retrieveUserQuotaSummary.
///
/// Best-effort: returns `None` on any failure so that the primary 5h quota fetch
/// (fetchAvailableModels) is never blocked by this auxiliary endpoint.
async fn fetch_quota_summary(
    access_token: &str,
    email: &str,
    project_id: Option<&str>,
) -> Option<Vec<crate::models::quota::QuotaGroup>> {
    let client = create_standard_client().await.ok()?;
    let payload = if let Some(pid) = project_id {
        json!({ "project": pid })
    } else {
        json!({})
    };

    for ep_url in QUOTA_SUMMARY_ENDPOINTS.iter() {
        let res = client
            .post(*ep_url)
            .bearer_auth(access_token)
            .header(
                rquest::header::USER_AGENT,
                crate::constants::NATIVE_OAUTH_USER_AGENT.as_str(),
            )
            .json(&payload)
            .send()
            .await;

        match res {
            Ok(response) => {
                let status = response.status();
                if !status.is_success() {
                    crate::modules::logger::log_warn(&format!(
                        "QuotaSummary API {} returned {}, trying next endpoint",
                        ep_url, status
                    ));
                    continue;
                }

                let summary: QuotaSummaryResponse = match response.json().await {
                    Ok(s) => s,
                    Err(e) => {
                        crate::modules::logger::log_warn(&format!(
                            "QuotaSummary JSON parse failed for {}: {}",
                            email, e
                        ));
                        return None;
                    }
                };

                let groups: Vec<crate::models::quota::QuotaGroup> = summary
                    .groups
                    .into_iter()
                    .map(|g| crate::models::quota::QuotaGroup {
                        display_name: g.display_name.unwrap_or_default(),
                        description: g.description,
                        buckets: g
                            .buckets
                            .into_iter()
                            .map(|b| crate::models::quota::QuotaBucket {
                                bucket_id: b.bucket_id.unwrap_or_default(),
                                window: b.window.unwrap_or_default(),
                                remaining_fraction: b.remaining_fraction.unwrap_or(0.0),
                                remaining_fraction_known: Some(
                                    crate::models::quota::fraction_is_known(b.remaining_fraction),
                                ),
                                reset_time: b.reset_time.unwrap_or_default(),
                                display_name: b.display_name,
                                description: b.description,
                            })
                            .collect(),
                    })
                    .collect();

                tracing::debug!("[{}] QuotaSummary fetched {} groups", email, groups.len());
                return Some(groups);
            }
            Err(e) => {
                crate::modules::logger::log_warn(&format!(
                    "QuotaSummary API request failed at {}: {}",
                    ep_url, e
                ));
                continue;
            }
        }
    }

    None
}

/// Retain the primary model observation when no matching pool was reported.
/// Never borrow a shared pool for Argon, or a model pool for shared Gemini.
fn fuse_model_quotas(models: &mut [crate::models::quota::ModelQuota], groups: &[crate::models::quota::QuotaGroup]) {
    use crate::models::quota::{model_quota_pool, quota_pool};
    for model in models {
        let Some(pool) = model_quota_pool(&model.name, model.display_name.as_deref()) else { continue; };
        let buckets: Vec<_> = groups.iter().flat_map(|group| group.buckets.iter()
            .filter(move |bucket| quota_pool(&bucket.bucket_id, &group.display_name) == Some(pool))).collect();
        let resolve_window = |window: &str| {
            let matching: Vec<_> = buckets.iter().copied().filter(|b| b.window.eq_ignore_ascii_case(window)).collect();
            let first = *matching.first()?;
            let conflict = matching.iter().any(|b|
                b.bucket_id != first.bucket_id
                || b.remaining_fraction != first.remaining_fraction
                || b.remaining_fraction_known != first.remaining_fraction_known
                || b.reset_time != first.reset_time
            );
            let known = !conflict
                && first.remaining_fraction_known != Some(false)
                && crate::models::quota::fraction_is_known(Some(first.remaining_fraction));
            Some((first, conflict, known))
        };
        let selected = match (resolve_window("5h"), resolve_window("weekly")) {
            (Some(s), Some(w)) => {
                if !s.2 {
                    Some(s)
                } else if !w.2 {
                    Some(w)
                } else if w.0.remaining_fraction < s.0.remaining_fraction {
                    Some(w)
                } else {
                    Some(s)
                }
            }
            (Some(s), None) => Some(s),
            (None, Some(w)) => Some(w),
            (None, None) => None,
        };
        let Some((bucket, conflict, known)) = selected else { continue; };
        model.percentage_known = Some(known);
        model.observed_remaining_fraction = known.then_some(bucket.remaining_fraction);
        model.quota_bucket_id = (!conflict && !bucket.bucket_id.is_empty()).then(|| bucket.bucket_id.clone());
        if known { model.percentage = (bucket.remaining_fraction * 100.0).round() as i32; }
        if !bucket.reset_time.is_empty() { model.reset_time = bucket.reset_time.clone(); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn argon_quota_fusion_is_independent_of_group_order_and_shared_pool() {
        let fixture = |id: &str, fraction: f64| serde_json::json!({
            "bucket_id":id,"window":if id.ends_with("5h") {"5h"} else {"weekly"},
            "remaining_fraction":fraction,"remaining_fraction_known":true,"reset_time":"2030-01-01T00:00:00Z"
        });
        let groups: Vec<crate::models::quota::QuotaGroup> = serde_json::from_value(json!([
            {"display_name":"Gemini 4 Argon","buckets":[fixture("gemini-argon-wtu-weekly",0.1)]},
            {"display_name":"Gemini Models","buckets":[fixture("gemini-weekly",0.9),fixture("gemini-5h",0.8)]}
        ])).unwrap();
        for mut groups in [groups.clone(), groups.into_iter().rev().collect()] {
            let mut models = serde_json::from_value::<Vec<crate::models::quota::ModelQuota>>(json!([
                {"name":"gemini-4-argon-high","percentage":100,"reset_time":""},
                {"name":"gemini-3.8-flash","percentage":100,"reset_time":""}
            ])).unwrap();
            fuse_model_quotas(&mut models, &groups);
            assert_eq!(models[0].percentage, 10);
            assert_eq!(models[0].quota_bucket_id.as_deref(), Some("gemini-argon-wtu-weekly"));
            assert_eq!(models[1].percentage, 80);
            groups.retain(|g| g.display_name != "Gemini 4 Argon");
            models[0].quota_bucket_id = None;
            fuse_model_quotas(&mut models, &groups);
            assert!(models[0].quota_bucket_id.is_none());
            assert_eq!(models[0].percentage, 10);
        }
    }
    #[test]
    fn quota_fusion_caps_by_weekly_when_weekly_is_more_restrictive_than_5h() {
        let groups: Vec<crate::models::quota::QuotaGroup> = serde_json::from_value(json!([
            {"display_name":"Gemini Models","buckets":[
                {"bucket_id":"gemini-weekly","window":"weekly","remaining_fraction":0.4569,"remaining_fraction_known":true,"reset_time":"2030-01-05T00:00:00Z"},
                {"bucket_id":"gemini-5h","window":"5h","remaining_fraction":1.0,"remaining_fraction_known":true,"reset_time":"2030-01-01T05:00:00Z"}
            ]},
            {"display_name":"Claude and GPT models","buckets":[
                {"bucket_id":"3p-weekly","window":"weekly","remaining_fraction":0.0061,"remaining_fraction_known":true,"reset_time":"2030-01-05T12:00:00Z"},
                {"bucket_id":"3p-5h","window":"5h","remaining_fraction":1.0,"remaining_fraction_known":true,"reset_time":"2030-01-01T05:00:00Z"}
            ]}
        ])).unwrap();
        let mut models = serde_json::from_value::<Vec<crate::models::quota::ModelQuota>>(json!([
            {"name":"gemini-3.8-flash","percentage":100,"reset_time":""},
            {"name":"claude-sonnet-4-6","percentage":100,"reset_time":""}
        ])).unwrap();
        fuse_model_quotas(&mut models, &groups);
        assert_eq!(models[0].percentage, 46);
        assert_eq!(models[0].quota_bucket_id.as_deref(), Some("gemini-weekly"));
        assert_eq!(models[0].reset_time, "2030-01-05T00:00:00Z");
        assert_eq!(models[1].percentage, 1);
        assert_eq!(models[1].quota_bucket_id.as_deref(), Some("3p-weekly"));
        assert_eq!(models[1].reset_time, "2030-01-05T12:00:00Z");
    }
    #[test]
    fn production_summary_precedes_successful_but_incomplete_daily_endpoints() {
        assert_eq!(QUOTA_SUMMARY_ENDPOINTS[0], "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary");
    }
}
