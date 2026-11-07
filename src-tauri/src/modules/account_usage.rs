use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::models::quota::{quota_pool, model_quota_pool, QuotaBucket, QuotaPool};
use crate::models::AppConfig;
use crate::modules::api_pricing::{self, ApiPricing};
use crate::modules::menu_bar_usage::{match_model_price, pricing_keys};
use crate::modules::native_token_stats::{self, GenerationEvent};

const SPANS_FILE: &str = "account_usage_spans.json";
const CALIBRATION_FILE: &str = "quota_capacity_calibration.json";
const CYCLE_HISTORY_FILE: &str = "quota_cycle_history.json";
const MAX_SPANS_RETENTION_DAYS: i64 = 35;
const MAX_CALIBRATION_SAMPLES: usize = 9;
const MAX_CYCLES_PER_BUCKET: usize = 24;
const MAX_CHECKPOINTS_PER_CYCLE: usize = 20;

static ACCOUNT_USAGE_LOCK: Mutex<()> = Mutex::new(());

/// 周期内记录的采样快照
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CycleCheckpoint {
    pub ts: i64,
    pub remaining_fraction: f64,
}

/// 周期额度变化与折算账本单条记录（绝不含 Token 或凭据）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuotaCycleRecord {
    pub cycle_id: String,
    pub account_id: String,
    pub pool: String,
    pub window: String,
    pub cycle_start_ts: i64,
    pub reset_ts: i64,
    pub reset_time: String,
    pub status: String, // "active" | "completed"
    pub first_observed_remaining: f64,
    pub latest_remaining: f64,
    pub min_remaining: f64,
    pub consumed_fraction: f64,
    pub last_delta_fraction: f64,
    pub sample_count: u32,
    pub used_usd: f64,
    pub used_cny: f64,
    pub used_tokens: u64,
    pub estimated_total_usd: Option<f64>,
    pub estimated_total_cny: Option<f64>,
    pub estimated_total_tokens: Option<u64>,
    pub provenance: String,
    pub prev_cycle_total_usd: Option<f64>,
    pub delta_total_usd: Option<f64>,
    pub delta_total_pct: Option<f64>,
    #[serde(default)]
    pub checkpoints: Vec<CycleCheckpoint>,
    pub updated_at: i64,
}

/// 周期历史持久化存储
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuotaCycleHistoryStore {
    #[serde(default)]
    pub buckets: BTreeMap<String, Vec<QuotaCycleRecord>>,
}

/// 返回给前端的单账号周期汇总（动态关联邮箱与等级）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaCycleAccountSummary {
    pub account_id: String,
    pub email: String,
    pub subscription_tier: Option<String>,
    pub records: Vec<QuotaCycleRecord>,
}

/// 返回给前端的完整周期响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaCycleHistoryResponse {
    pub accounts: Vec<QuotaCycleAccountSummary>,
    pub currency: String,
    pub usd_to_cny_rate: f64,
    pub fetched_at: i64,
}

/// 单个账号激活区间（不含任何凭证/Token）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountUsageSpan {
    pub account_id: String,
    pub start_ts: i64,
    pub end_ts: i64,
}

/// 样本校准存储
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CalibrationSample {
    pub reset_time: String,
    pub total_usd: f64,
    pub usd_per_m_tokens: f64,
    pub recorded_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuotaCapacityCalibrationStore {
    #[serde(default)]
    pub samples: BTreeMap<String, Vec<CalibrationSample>>,
}

/// 单个配额桶折算结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketValuation {
    pub bucket_id: String,
    pub window: String,
    pub pool: String, // "gemini" | "third_party" | "argon" | "unknown"
    pub remaining_fraction: f64,
    pub reset_time: String,
    /// 价值来源："observed" | "exhausted" | "used_only" | "manual_reference" | "calibrated" | "none"
    pub provenance: String,
    pub used_usd: f64,
    pub used_tokens: u64,
    pub estimated_total_usd: Option<f64>,
    pub estimated_remaining_usd: Option<f64>,
    pub estimated_total_tokens: Option<u64>,
    pub estimated_remaining_tokens: Option<u64>,
    pub used_cny: f64,
    pub estimated_total_cny: Option<f64>,
    pub estimated_remaining_cny: Option<f64>,
    pub warning: Option<String>,
    #[serde(default)]
    pub last_delta_fraction: Option<f64>,
    #[serde(default)]
    pub consumed_fraction: Option<f64>,
    #[serde(default)]
    pub delta_total_usd: Option<f64>,
    #[serde(default)]
    pub delta_total_pct: Option<f64>,
}

/// 单个账号的配额折算总览
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountQuotaValuation {
    pub account_id: String,
    pub email: String,
    pub subscription_tier: Option<String>,
    pub is_active: bool,
    pub buckets: Vec<BucketValuation>,
    pub unpriced_events_count: u64,
}

/// 返回给前端的完整响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaValuationResponse {
    pub accounts: Vec<AccountQuotaValuation>,
    pub currency: String,
    pub display_mode: String,
    pub usd_to_cny_rate: f64,
    pub rate_stale: bool,
    pub calculated_at: i64,
}

fn data_dir() -> Option<PathBuf> {
    std::env::var_os("ABV_DATA_DIR")
        .filter(|p| !p.to_string_lossy().trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".antigravity_tools")))
}

fn spans_path() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(SPANS_FILE))
}

fn calibration_path() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(CALIBRATION_FILE))
}

pub fn load_spans() -> Vec<AccountUsageSpan> {
    let Some(path) = spans_path() else { return Vec::new(); };
    let Ok(bytes) = fs::read(path) else { return Vec::new(); };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

pub fn save_spans(spans: &[AccountUsageSpan]) -> Result<(), String> {
    let Some(path) = spans_path() else { return Err("无法获取数据目录".into()); };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let data = serde_json::to_vec_pretty(spans).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())
}

pub fn load_calibration() -> QuotaCapacityCalibrationStore {
    let Some(path) = calibration_path() else { return QuotaCapacityCalibrationStore::default(); };
    let Ok(bytes) = fs::read(path) else { return QuotaCapacityCalibrationStore::default(); };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

pub fn save_calibration(store: &QuotaCapacityCalibrationStore) -> Result<(), String> {
    let Some(path) = calibration_path() else { return Err("无法获取数据目录".into()); };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let data = serde_json::to_vec_pretty(store).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())
}

fn cycle_history_path() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(CYCLE_HISTORY_FILE))
}

pub fn load_cycle_history() -> QuotaCycleHistoryStore {
    let Some(path) = cycle_history_path() else { return QuotaCycleHistoryStore::default(); };
    let Ok(bytes) = fs::read(path) else { return QuotaCycleHistoryStore::default(); };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

pub fn save_cycle_history(store: &QuotaCycleHistoryStore) -> Result<(), String> {
    let Some(path) = cycle_history_path() else { return Err("无法获取数据目录".into()); };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let data = serde_json::to_vec_pretty(store).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())
}

/// 统一刷新钩子：记录每次配额刷新时的周期状态与变化（锁安全：绝不反向调用 load_account）
pub fn record_quota_cycle_refresh(account_id: &str, quota: &crate::models::quota::QuotaData) {
    if account_id.trim().is_empty() {
        return;
    }
    let now_ts = Utc::now().timestamp();
    let effective_buckets = get_account_effective_buckets(quota, now_ts);
    if effective_buckets.is_empty() {
        return;
    }

    let _lock = ACCOUNT_USAGE_LOCK.lock();
    let mut store = load_cycle_history();

    for (bucket, _group_name, pool_opt) in effective_buckets {
        let pool_key = match pool_opt {
            Some(QuotaPool::Gemini) => "gemini",
            Some(QuotaPool::ThirdParty) => "3p",
            Some(QuotaPool::Argon) => "argon",
            None => continue,
        };
        let normalized_window = if bucket.window.to_ascii_lowercase().contains("week") {
            "weekly"
        } else {
            "5h"
        };

        if bucket.remaining_fraction_known == Some(false) || !bucket.remaining_fraction.is_finite() {
            continue;
        }

        let new_rem = bucket.remaining_fraction.clamp(0.0, 1.0);
        let window_secs = window_duration_seconds(&bucket.window);
        let new_reset_ts = parse_rfc3339_timestamp(&bucket.reset_time).unwrap_or(now_ts + window_secs);

        let composite_key = format!("{}:{}:{}", account_id, pool_key, normalized_window);
        let cycle_list = store.buckets.entry(composite_key).or_default();

        let mut needs_new_cycle = true;
        if let Some(existing) = cycle_list.last_mut() {
            if existing.status == "active" {
                // 1. 同周期判定：reset_ts 容差在 300 秒内
                if (new_reset_ts - existing.reset_ts).abs() <= 300 {
                    needs_new_cycle = false;
                    existing.sample_count = existing.sample_count.saturating_add(1);
                    let delta = new_rem - existing.latest_remaining;
                    // 防冲刷铁律：实质变化 >= 0.005 才记录 last_delta 并追加 checkpoint
                    if delta.abs() >= 0.005 {
                        existing.last_delta_fraction = delta;
                        existing.latest_remaining = new_rem;
                        if new_rem < existing.min_remaining {
                            existing.min_remaining = new_rem;
                        }
                        existing.consumed_fraction = (1.0 - existing.min_remaining).clamp(0.0, 1.0);
                        existing.checkpoints.push(CycleCheckpoint {
                            ts: now_ts,
                            remaining_fraction: new_rem,
                        });
                        if existing.checkpoints.len() > MAX_CHECKPOINTS_PER_CYCLE {
                            existing.checkpoints.remove(0);
                        }
                    } else {
                        existing.latest_remaining = new_rem;
                    }
                    existing.updated_at = now_ts;
                }
                // 2. 空转顺延滑动（防刷屏铁律）：上周期全程 100% 满额，reset_time 被 Google 往后推
                else if new_reset_ts > existing.reset_ts + 300 && existing.consumed_fraction < 0.005 && new_rem >= 0.999 {
                    needs_new_cycle = false;
                    existing.reset_ts = new_reset_ts;
                    existing.reset_time = bucket.reset_time.clone();
                    existing.cycle_start_ts = new_reset_ts - window_secs;
                    existing.updated_at = now_ts;
                }
                // 3. 真实周期重置归档：归档旧周期并开启新周期
                else {
                    existing.status = "completed".to_string();
                    existing.updated_at = now_ts;
                    // 保持 needs_new_cycle = true
                }
            }
        }

        if needs_new_cycle {
            let prev_cycle_total_usd = cycle_list
                .iter()
                .rev()
                .find(|c| c.status == "completed" && c.estimated_total_usd.is_some())
                .and_then(|c| c.estimated_total_usd);

            let new_record = QuotaCycleRecord {
                cycle_id: format!("{}:{}", normalized_window, new_reset_ts),
                account_id: account_id.to_string(),
                pool: match pool_key {
                    "gemini" => "gemini".to_string(),
                    "3p" => "third_party".to_string(),
                    "argon" => "argon".to_string(),
                    _ => pool_key.to_string(),
                },
                window: normalized_window.to_string(),
                cycle_start_ts: new_reset_ts - window_secs,
                reset_ts: new_reset_ts,
                reset_time: bucket.reset_time.clone(),
                status: "active".to_string(),
                first_observed_remaining: new_rem,
                latest_remaining: new_rem,
                min_remaining: new_rem,
                consumed_fraction: (1.0 - new_rem).clamp(0.0, 1.0),
                last_delta_fraction: 0.0,
                sample_count: 1,
                used_usd: 0.0,
                used_cny: 0.0,
                used_tokens: 0,
                estimated_total_usd: None,
                estimated_total_cny: None,
                estimated_total_tokens: None,
                provenance: "none".to_string(),
                prev_cycle_total_usd,
                delta_total_usd: None,
                delta_total_pct: None,
                checkpoints: vec![CycleCheckpoint {
                    ts: now_ts,
                    remaining_fraction: new_rem,
                }],
                updated_at: now_ts,
            };

            cycle_list.push(new_record);
            if cycle_list.len() > MAX_CYCLES_PER_BUCKET {
                cycle_list.remove(0);
            }
        }
    }

    let _ = save_cycle_history(&store);
}

/// 记录账号活跃状态。
/// 1. 同一账号持续活跃：直接延长 end_ts，不设断开限制；
/// 2. 切换至另一账号：先闭合前序账号活跃区间，再推入新账号区间。
pub fn record_active_account(account_id: &str, timestamp: i64) {
    if account_id.trim().is_empty() {
        return;
    }
    let _lock = ACCOUNT_USAGE_LOCK.lock();
    let mut spans = load_spans();
    let cutoff = timestamp - MAX_SPANS_RETENTION_DAYS * 86400;

    if let Some(last) = spans.last_mut() {
        if last.account_id == account_id {
            if timestamp >= last.start_ts {
                last.end_ts = last.end_ts.max(timestamp);
            }
        } else {
            // 切号：闭合前序账号的活跃区间
            if timestamp > last.end_ts {
                last.end_ts = timestamp;
            }
            spans.push(AccountUsageSpan {
                account_id: account_id.to_string(),
                start_ts: timestamp,
                end_ts: timestamp,
            });
        }
    } else {
        spans.push(AccountUsageSpan {
            account_id: account_id.to_string(),
            start_ts: timestamp,
            end_ts: timestamp,
        });
    }

    spans.retain(|s| s.end_ts >= cutoff);
    let _ = save_spans(&spans);
}

/// 计算某账号在 [window_start, window_end] 内的有效时间片交集
pub fn active_intervals_for_account(
    account_id: &str,
    window_start: i64,
    window_end: i64,
    spans: &[AccountUsageSpan],
    is_active: bool,
    account_last_used: Option<i64>,
    other_accounts_max_last_used: Option<i64>,
) -> Vec<(i64, i64)> {
    if window_start >= window_end {
        return Vec::new();
    }

    let mut intervals = Vec::new();
    let spans_len = spans.len();

    for (idx, span) in spans.iter().enumerate() {
        if span.account_id == account_id {
            // 若当前账号仍处于全局活跃状态，且本 span 是最后一条记录，则有效结束时间延续至 window_end
            let effective_end = if is_active && idx + 1 == spans_len {
                span.end_ts.max(window_end)
            } else {
                span.end_ts
            };

            let start = span.start_ts.max(window_start);
            let end = effective_end.min(window_end);
            if start <= end {
                intervals.push((start, end));
            }
        }
    }

    // 冷启动兼容：当 spans 为空，且为当前活跃账号，且其 last_used 严格晚于其他所有账号时保守启用
    if intervals.is_empty() && spans.is_empty() && is_active {
        if let Some(last_used) = account_last_used.filter(|&t| t > 0) {
            let strictly_later = other_accounts_max_last_used.filter(|&t| t > 0).map_or(true, |other| last_used > other);
            if strictly_later {
                let start = last_used.max(window_start);
                let end = window_end;
                if start <= end {
                    intervals.push((start, end));
                }
            }
        }
    }

    intervals
}

fn is_timestamp_in_intervals(ts: i64, intervals: &[(i64, i64)]) -> bool {
    intervals.iter().any(|&(s, e)| ts >= s && ts <= e)
}

pub fn parse_rfc3339_timestamp(rfc3339_str: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(rfc3339_str)
        .ok()
        .map(|dt| dt.timestamp())
}

pub fn window_duration_seconds(window: &str) -> i64 {
    let lower = window.trim().to_ascii_lowercase();
    if lower.contains("week") {
        7 * 86400
    } else if lower.contains("5h") {
        5 * 3600
    } else if lower.contains("1h") {
        3600
    } else if lower.contains("24h") || lower.contains("day") {
        86400
    } else {
        18000
    }
}

pub fn is_placeholder_window(remaining_fraction: f64, reset_ts: i64, now_ts: i64, window_secs: i64) -> bool {
    if remaining_fraction < 1.0 {
        return false;
    }
    (reset_ts - now_ts - window_secs).abs() <= 90
}

/// 计算单个事件的费用。严禁捏造未定价模型单价！未匹配返回 None。
fn evaluate_event_cost(event: &GenerationEvent, prices: &[ApiPricing]) -> Option<f64> {
    let matched_price = match_model_price(&event.model, prices)?;
    let input_cost = (event.input_tokens as f64 * matched_price.input) / 1_000_000.0;
    let output_cost = (event.output_tokens as f64 * matched_price.output) / 1_000_000.0;
    let cached_cost = (event.cached_tokens as f64 * matched_price.cached) / 1_000_000.0;
    Some(input_cost + output_cost + cached_cost)
}

/// 代表模型典型 4:1 (0.8*input + 0.2*output) 混合单价 ($/1M tokens)
fn blended_reference_price_per_m_tokens(pool: QuotaPool, prices: &[ApiPricing]) -> f64 {
    let matched = match pool {
        QuotaPool::Gemini => prices
            .iter()
            .find(|p| {
                let (exact, _) = pricing_keys(&p.model);
                exact == "gemini-3-8-flash"
            })
            .or_else(|| {
                prices.iter().find(|p| {
                    let (exact, _) = pricing_keys(&p.model);
                    exact == "gemini-2-5-pro"
                })
            })
            .or_else(|| {
                prices.iter().find(|p| {
                    let (exact, base) = pricing_keys(&p.model);
                    exact.starts_with("gemini-")
                        && (exact.contains("-pro") || exact.contains("-flash") || base.is_some())
                })
            }),
        QuotaPool::ThirdParty => prices.iter().find(|p| {
            let (exact, _) = pricing_keys(&p.model);
            exact.contains("claudesonnet46") || exact.contains("claude-sonnet-4-6")
        }).or_else(|| prices.iter().find(|p| {
            let (exact, _) = pricing_keys(&p.model);
            exact.contains("claude") || exact.contains("gpt")
        })),
        QuotaPool::Argon => prices.iter().find(|p| {
            let (exact, _) = pricing_keys(&p.model);
            exact.contains("argon")
        }),
    };

    if let Some(p) = matched {
        0.8 * p.input + 0.2 * p.output
    } else {
        match pool {
            QuotaPool::Gemini => 2.0,      // 0.8 * 0.75 + 0.2 * 3.75 ~ 1.35 或 Pro 4:1 混合
            QuotaPool::ThirdParty => 5.4,  // 0.8 * 3.0 + 0.2 * 15.0
            QuotaPool::Argon => 3.6,       // 0.8 * 2.0 + 0.2 * 10.0
        }
    }
}

pub fn normalize_tier(tier_raw: &str) -> &'static str {
    let lower = tier_raw.trim().to_ascii_lowercase();
    if lower.contains("ultra") {
        "ultra"
    } else if lower.contains("pro") {
        "pro"
    } else if lower.contains("free") || lower.contains("starter") {
        "free"
    } else {
        "unknown"
    }
}

pub fn builtin_tier_reference_cap_usd(tier_key: &str, pool_key: &str, window_key: &str) -> Option<f64> {
    let tier = normalize_tier(tier_key);
    let pool = pool_key.trim().to_ascii_lowercase();
    let is_weekly = window_key.contains("week");
    let is_5h = window_key.contains("5h") || window_key.contains("hour") || (!is_weekly && !window_key.contains("day"));

    match tier {
        "pro" => match pool.as_str() {
            "gemini" => if is_weekly { Some(35.0) } else if is_5h { Some(5.0) } else { Some(35.0) },
            "3p" | "third_party" => if is_weekly { Some(25.0) } else if is_5h { Some(5.0) } else { Some(25.0) },
            "argon" => if is_weekly { Some(25.0) } else if is_5h { Some(5.0) } else { Some(25.0) },
            _ => None,
        },
        "ultra" => match pool.as_str() {
            "gemini" => if is_weekly { Some(100.0) } else if is_5h { Some(15.0) } else { Some(100.0) },
            "3p" | "third_party" => if is_weekly { Some(75.0) } else if is_5h { Some(15.0) } else { Some(75.0) },
            "argon" => if is_weekly { Some(75.0) } else if is_5h { Some(15.0) } else { Some(75.0) },
            _ => None,
        },
        "free" => match pool.as_str() {
            "gemini" => if is_weekly { Some(5.0) } else if is_5h { Some(1.0) } else { Some(5.0) },
            "3p" | "third_party" => if is_weekly { Some(5.0) } else if is_5h { Some(1.0) } else { Some(5.0) },
            "argon" => if is_weekly { Some(5.0) } else { Some(1.0) },
            _ => None,
        },
        _ => None,
    }
}

fn tier_weight(tier_key: &str) -> f64 {
    match normalize_tier(tier_key) {
        "ultra" => 3.0,
        "pro" => 1.0,
        "free" => 0.2,
        _ => 1.0,
    }
}

fn calculate_median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        Some((values[mid - 1] + values[mid]) / 2.0)
    } else {
        Some(values[mid])
    }
}

/// 折算单个配额桶的主流程（对外兼容接口，单测或单桶计算时直接跑完 Pass 1 与 Pass 2）
pub fn valuate_bucket(
    bucket: &QuotaBucket,
    group_name: &str,
    account_id: &str,
    tier_key: &str,
    is_active: bool,
    account_last_used: Option<i64>,
    other_accounts_max_last_used: Option<i64>,
    now_ts: i64,
    spans: &[AccountUsageSpan],
    events: &[GenerationEvent],
    prices: &[ApiPricing],
    calibration_store: &mut QuotaCapacityCalibrationStore,
    calibration_dirty: &mut bool,
    config: &AppConfig,
    rate: f64,
) -> BucketValuation {
    let mut bv = valuate_bucket_with_cold_share(
        bucket,
        group_name,
        account_id,
        tier_key,
        is_active,
        account_last_used,
        other_accounts_max_last_used,
        now_ts,
        spans,
        events,
        prices,
        calibration_store,
        calibration_dirty,
        config,
        rate,
        1.0,
    );

    let pool_opt = quota_pool(&bucket.bucket_id, group_name);
    let pool_key = match pool_opt {
        Some(QuotaPool::Gemini) => "gemini",
        Some(QuotaPool::ThirdParty) => "3p",
        Some(QuotaPool::Argon) => "argon",
        None => "unknown",
    };
    let fraction_known = bucket.remaining_fraction_known != Some(false) && bucket.remaining_fraction.is_finite();
    apply_reference_cap_pass2(
        &mut bv,
        tier_key,
        pool_key,
        &bucket.window.to_ascii_lowercase(),
        pool_opt,
        fraction_known,
        prices,
        calibration_store,
        config,
        rate,
    );
    bv
}

pub fn valuate_bucket_with_cold_share(
    bucket: &QuotaBucket,
    group_name: &str,
    account_id: &str,
    tier_key: &str,
    is_active: bool,
    account_last_used: Option<i64>,
    other_accounts_max_last_used: Option<i64>,
    now_ts: i64,
    spans: &[AccountUsageSpan],
    events: &[GenerationEvent],
    prices: &[ApiPricing],
    calibration_store: &mut QuotaCapacityCalibrationStore,
    calibration_dirty: &mut bool,
    _config: &AppConfig,
    rate: f64,
    cold_start_share: f64,
) -> BucketValuation {
    let pool_opt = quota_pool(&bucket.bucket_id, group_name);

    let pool_str = match pool_opt {
        Some(QuotaPool::Gemini) => "gemini",
        Some(QuotaPool::ThirdParty) => "third_party",
        Some(QuotaPool::Argon) => "argon",
        None => "unknown",
    };

    let fraction_known = bucket.remaining_fraction_known != Some(false) && bucket.remaining_fraction.is_finite();
    let remaining_fraction = if fraction_known {
        bucket.remaining_fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };

    let window_secs = window_duration_seconds(&bucket.window);
    let reset_ts = parse_rfc3339_timestamp(&bucket.reset_time).unwrap_or(now_ts + window_secs);

    let is_placeholder = fraction_known && is_placeholder_window(remaining_fraction, reset_ts, now_ts, window_secs);
    let window_start = reset_ts - window_secs;
    let window_end = now_ts.min(reset_ts);

    let intervals = if is_placeholder || !fraction_known {
        Vec::new()
    } else {
        active_intervals_for_account(
            account_id,
            window_start,
            window_end,
            spans,
            is_active,
            account_last_used,
            other_accounts_max_last_used,
        )
    };

    let first_span_ts = spans.first().map(|s| s.start_ts);

    let mut used_tokens = 0_u64;
    let mut used_usd = 0.0_f64;
    let mut priced_events = 0_u64;
    let mut unpriced_events = 0_u64;

    if let Some(expected_pool) = pool_opt {
        for event in events {
            if event.timestamp_seconds < window_start || event.timestamp_seconds > window_end {
                continue;
            }
            if model_quota_pool(&event.model, None) != Some(expected_pool) {
                continue;
            }

            let effective_share = if let Some(first_ts) = first_span_ts {
                if event.timestamp_seconds >= first_ts {
                    if is_timestamp_in_intervals(event.timestamp_seconds, &intervals) {
                        1.0
                    } else {
                        0.0
                    }
                } else if cold_start_share > 0.0 {
                    cold_start_share
                } else {
                    0.0
                }
            } else if cold_start_share > 0.0 {
                if is_timestamp_in_intervals(event.timestamp_seconds, &intervals) {
                    cold_start_share
                } else {
                    0.0
                }
            } else {
                0.0
            };

            if effective_share <= 0.0 {
                continue;
            }

            let matched_price = match_model_price(&event.model, prices);
            let event_equiv_tokens = if let Some(p) = matched_price {
                if p.input > 0.0 {
                    let equiv_cached = ((event.cached_tokens as f64) * (p.cached / p.input)).round() as u64;
                    event.input_tokens.saturating_add(event.output_tokens).saturating_add(equiv_cached)
                } else {
                    event.input_tokens.saturating_add(event.output_tokens).saturating_add(event.cached_tokens / 10)
                }
            } else {
                event.input_tokens.saturating_add(event.output_tokens).saturating_add(event.cached_tokens / 10)
            };

            let tok = (event_equiv_tokens as f64 * effective_share).round() as u64;
            used_tokens = used_tokens.saturating_add(tok);
            if let Some(cost) = evaluate_event_cost(event, prices) {
                used_usd += cost * effective_share;
                priced_events += 1;
            } else {
                unpriced_events += 1;
            }
        }
    }

    let mut provenance = if used_tokens > 0 { "used_only".to_string() } else { "none".to_string() };
    let mut estimated_total_usd = None;
    let mut estimated_remaining_usd = None;
    let mut estimated_total_tokens = None;
    let mut estimated_remaining_tokens = None;
    let mut warning = None;

    if unpriced_events > 0 {
        warning = Some(format!("包含 {} 条未在价格表中精确匹配的模型记录，未计入费用", unpriced_events));
    }

    if fraction_known {
        let used_fraction = (1.0 - remaining_fraction).clamp(0.0, 1.0);
        let pool_key = match pool_opt {
            Some(QuotaPool::Gemini) => "gemini",
            Some(QuotaPool::ThirdParty) => "3p",
            Some(QuotaPool::Argon) => "argon",
            None => "unknown",
        };
        let window_key = bucket.window.to_ascii_lowercase();
        let composite_key = format!("{}:{}:{}", tier_key.to_ascii_lowercase(), pool_key, window_key);

        if remaining_fraction <= 0.0 && used_tokens > 0 {
            provenance = "exhausted".to_string();
            estimated_total_tokens = Some(used_tokens);
            estimated_remaining_tokens = Some(0);
            if priced_events > 0 {
                estimated_total_usd = Some(used_usd);
                estimated_remaining_usd = Some(0.0);
            }
        } else if used_fraction >= 0.03 && used_tokens > 0 {
            provenance = "observed".to_string();
            let total_tok = (used_tokens as f64 / used_fraction).round() as u64;
            let rem_tok = (total_tok as f64 * remaining_fraction).round() as u64;
            estimated_total_tokens = Some(total_tok);
            estimated_remaining_tokens = Some(rem_tok);

            if priced_events > 0 {
                let total_usd = used_usd / used_fraction;
                let rem_usd = total_usd * remaining_fraction;
                estimated_total_usd = Some(total_usd);
                estimated_remaining_usd = Some(rem_usd);

                // 高置信度校准样本沉淀
                if used_fraction >= 0.15 && unpriced_events == 0 && total_tok > 0 && tier_key != "unknown" {
                    let usd_per_m = total_usd / (total_tok as f64 / 1_000_000.0);
                    let samples = calibration_store.samples.entry(composite_key).or_default();
                    if !samples.iter().any(|s| s.reset_time == bucket.reset_time) {
                        samples.push(CalibrationSample {
                            reset_time: bucket.reset_time.clone(),
                            total_usd,
                            usd_per_m_tokens: usd_per_m,
                            recorded_at: now_ts,
                        });
                        if samples.len() > MAX_CALIBRATION_SAMPLES {
                            samples.remove(0);
                        }
                        *calibration_dirty = true;
                    }
                }
            }
        } else if used_fraction > 0.0 && used_tokens > 0 {
            provenance = "used_only".to_string();
        }
    }

    let used_cny = used_usd * rate;
    let estimated_total_cny = estimated_total_usd.map(|v| v * rate);
    let estimated_remaining_cny = estimated_remaining_usd.map(|v| v * rate);

    BucketValuation {
        bucket_id: bucket.bucket_id.clone(),
        window: bucket.window.clone(),
        pool: pool_str.to_string(),
        remaining_fraction,
        reset_time: bucket.reset_time.clone(),
        provenance,
        used_usd,
        used_tokens,
        estimated_total_usd,
        estimated_remaining_usd,
        estimated_total_tokens,
        estimated_remaining_tokens,
        used_cny,
        estimated_total_cny,
        estimated_remaining_cny,
        warning,
        last_delta_fraction: None,
        consumed_fraction: None,
        delta_total_usd: None,
        delta_total_pct: None,
    }
}

fn apply_reference_cap_pass2(
    bv: &mut BucketValuation,
    tier_key: &str,
    pool_key: &str,
    window_key: &str,
    pool_opt: Option<QuotaPool>,
    fraction_known: bool,
    prices: &[ApiPricing],
    calibration_store: &QuotaCapacityCalibrationStore,
    config: &AppConfig,
    rate: f64,
) {
    let can_use_reference = fraction_known
        && (bv.provenance == "none" || bv.provenance == "used_only")
        && config.quota_valuation.use_reference_cap;

    if !can_use_reference {
        return;
    }

    let composite_key = format!("{}:{}:{}", tier_key.to_ascii_lowercase(), pool_key, window_key);

    let manual_cap = config.quota_valuation.reference_caps_usd.get(&composite_key)
        .or_else(|| config.quota_valuation.reference_caps_usd.get(&format!("{}:{}", pool_key, window_key)))
        .or_else(|| if pool_key == "3p" {
            config.quota_valuation.reference_caps_usd.get(&format!("{}:third_party:{}", tier_key, window_key))
                .or_else(|| config.quota_valuation.reference_caps_usd.get(&format!("third_party:{}", window_key)))
        } else {
            None
        })
        .or_else(|| config.quota_valuation.reference_caps_usd.get(&bv.bucket_id))
        .or_else(|| config.quota_valuation.reference_caps_usd.get(pool_key))
        .copied()
        .filter(|c| c.is_finite() && *c > 0.0);

    let calibrated_cap = calibration_store.samples.get(&composite_key).and_then(|samples| {
        let mut totals: Vec<f64> = samples.iter().map(|s| s.total_usd).collect();
        calculate_median(&mut totals)
    });

    let calibrated_usd_per_m = calibration_store.samples.get(&composite_key).and_then(|samples| {
        let mut per_m: Vec<f64> = samples.iter().map(|s| s.usd_per_m_tokens).collect();
        calculate_median(&mut per_m)
    });

    let estimated_ref_cap = if manual_cap.is_none() && calibrated_cap.is_none() {
        builtin_tier_reference_cap_usd(tier_key, pool_key, window_key)
    } else {
        None
    };

    let chosen_cap = manual_cap.or(calibrated_cap).or(estimated_ref_cap);

    if let Some(cap) = chosen_cap {
        bv.provenance = if manual_cap.is_some() {
            "manual_reference".to_string()
        } else if calibrated_cap.is_some() {
            "calibrated".to_string()
        } else {
            "estimated_reference".to_string()
        };

        bv.estimated_total_usd = Some(cap);
        bv.estimated_remaining_usd = Some(cap * bv.remaining_fraction);

        let per_m = calibrated_usd_per_m.unwrap_or_else(|| {
            blended_reference_price_per_m_tokens(pool_opt.unwrap_or(QuotaPool::Gemini), prices)
        });
        if per_m > 0.0 {
            bv.estimated_total_tokens = Some((cap / per_m * 1_000_000.0).round() as u64);
            bv.estimated_remaining_tokens = Some(((cap * bv.remaining_fraction) / per_m * 1_000_000.0).round() as u64);
        }

        bv.used_cny = bv.used_usd * rate;
        bv.estimated_total_cny = bv.estimated_total_usd.map(|v| v * rate);
        bv.estimated_remaining_cny = bv.estimated_remaining_usd.map(|v| v * rate);
    }
}

fn extract_synthetic_5h_bucket(
    models: &[crate::models::quota::ModelQuota],
    pool: QuotaPool,
    now_ts: i64,
) -> Option<QuotaBucket> {
    let mut rem_fractions = Vec::new();
    let mut reset_timestamps = Vec::new();

    for m in models {
        if model_quota_pool(&m.name, m.display_name.as_deref()) != Some(pool) {
            continue;
        }
        if let Some(ts) = parse_rfc3339_timestamp(&m.reset_time) {
            if ts > now_ts {
                reset_timestamps.push(ts);
            }
        }
        if m.percentage_known == Some(false) {
            continue;
        }
        let fraction = m.observed_remaining_fraction
            .or_else(|| (m.percentage >= 0).then(|| m.percentage as f64 / 100.0));
        if let Some(f) = fraction {
            rem_fractions.push(f);
        }
    }

    if rem_fractions.is_empty() {
        return None;
    }

    let avg_f = calculate_median(&mut rem_fractions)?;
    let reset_ts = reset_timestamps.into_iter().min().unwrap_or(now_ts + 18000);
    let (bucket_id, display_name) = match pool {
        QuotaPool::Gemini => ("gemini-5h", "Gemini 5h"),
        QuotaPool::ThirdParty => ("3p-5h", "Claude/GPT 5h"),
        QuotaPool::Argon => ("gemini-argon-wtu-5h", "Gemini 4 Argon 5h"),
    };

    Some(QuotaBucket {
        bucket_id: bucket_id.to_string(),
        window: "5h".to_string(),
        remaining_fraction: avg_f,
        remaining_fraction_known: Some(true),
        reset_time: chrono::DateTime::from_timestamp(reset_ts, 0).unwrap().to_rfc3339(),
        display_name: Some(display_name.to_string()),
        description: None,
    })
}

fn get_account_effective_buckets(
    quota: &crate::models::quota::QuotaData,
    now_ts: i64,
) -> Vec<(QuotaBucket, String, Option<QuotaPool>)> {
    let mut result = Vec::new();
    let mut has_gemini_5h = false;
    let mut has_tp_5h = false;

    if let Some(groups) = &quota.quota_groups {
        for group in groups {
            for bucket in &group.buckets {
                let pool_opt = quota_pool(&bucket.bucket_id, &group.display_name);
                let is_5h = bucket.window.to_ascii_lowercase().contains("5h") || (!bucket.window.to_ascii_lowercase().contains("week") && !bucket.bucket_id.to_ascii_lowercase().contains("week"));
                if is_5h {
                    if pool_opt == Some(QuotaPool::Gemini) { has_gemini_5h = true; }
                    if pool_opt == Some(QuotaPool::ThirdParty) { has_tp_5h = true; }
                }
                result.push((bucket.clone(), group.display_name.clone(), pool_opt));
            }
        }
    }

    // 若缺少 5h 桶，由 models 自动合成
    if !quota.models.is_empty() {
        if !has_gemini_5h {
            if let Some(b) = extract_synthetic_5h_bucket(&quota.models, QuotaPool::Gemini, now_ts) {
                result.push((b, "Gemini Models".to_string(), Some(QuotaPool::Gemini)));
            }
        }
        if !has_tp_5h {
            if let Some(b) = extract_synthetic_5h_bucket(&quota.models, QuotaPool::ThirdParty, now_ts) {
                result.push((b, "Claude and GPT models".to_string(), Some(QuotaPool::ThirdParty)));
            }
        }
    }

    result
}

/// 计算所有已保存账号的配额价值
pub async fn calculate_all_account_valuations() -> Result<QuotaValuationResponse, String> {
    let config = crate::modules::config::load_app_config()?;
    let rate_snapshot = api_pricing::get_exchange_rate(
        Some(config.quota_valuation.usd_to_cny_rate),
        config.quota_valuation.auto_exchange_rate,
    ).await;
    let rate = rate_snapshot.rate;

    let prices_snapshot = api_pricing::get_api_pricing().await.unwrap_or_else(|_| {
        api_pricing::ApiPricingSnapshot {
            parser_revision: 4,
            prices: api_pricing::builtin_fallback_prices(),
            fetched_at: Utc::now().timestamp(),
            stale: true,
            source: "兜底".to_string(),
            warning: None,
        }
    });

    let index = crate::modules::account::load_account_index()?;
    let current_acc_id = index.current_account_id.as_deref();

    let now_ts = Utc::now().timestamp();

    // 自动初始化当前激活账号时间片
    let mut spans = load_spans();
    if let Some(active_id) = current_acc_id {
        if spans.last().map(|s| s.account_id.as_str()) != Some(active_id) {
            let _ = record_active_account(active_id, now_ts);
            spans = load_spans();
        }
    }

    let since_ts = now_ts - MAX_SPANS_RETENTION_DAYS * 86400;

    let events = tokio::task::spawn_blocking(move || {
        native_token_stats::read_recent_generation_events(since_ts)
    })
    .await
    .map_err(|e| e.to_string())?
    .unwrap_or_default();

    let mut calibration_store = load_calibration();
    let mut calibration_dirty = false;

    let mut accounts_data = Vec::new();
    for entry in &index.accounts {
        if let Ok(acc) = crate::modules::account::load_account(&entry.id) {
            accounts_data.push(acc);
        }
    }

    // 统计各 (pool, normalized_window) 的冷启动消耗权重和
    let mut pool_window_total_weights: BTreeMap<(String, String), f64> = BTreeMap::new();
    for account in &accounts_data {
        let raw_tier = account
            .quota
            .as_ref()
            .and_then(|q| q.subscription_tier.as_deref())
            .unwrap_or("unknown");
        let tier_key = normalize_tier(raw_tier);
        let w = tier_weight(tier_key);

        if let Some(quota_data) = &account.quota {
            let effective_buckets = get_account_effective_buckets(quota_data, now_ts);
            for (bucket, _group_name, pool_opt) in effective_buckets {
                let pool_key = match pool_opt {
                    Some(QuotaPool::Gemini) => "gemini",
                    Some(QuotaPool::ThirdParty) => "3p",
                    Some(QuotaPool::Argon) => "argon",
                    None => continue,
                };
                let normalized_window = if bucket.window.to_ascii_lowercase().contains("week") {
                    "weekly"
                } else {
                    "5h"
                };
                let fraction_known = bucket.remaining_fraction_known != Some(false) && bucket.remaining_fraction.is_finite();
                if fraction_known {
                    let rem = bucket.remaining_fraction.clamp(0.0, 1.0);
                    let window_secs = window_duration_seconds(&bucket.window);
                    let reset_ts = parse_rfc3339_timestamp(&bucket.reset_time).unwrap_or(now_ts + window_secs);
                    let is_placeholder = is_placeholder_window(rem, reset_ts, now_ts, window_secs);
                    let used_fraction = (1.0 - rem).clamp(0.0, 1.0);
                    if !is_placeholder && used_fraction > 0.0 {
                        let entry = pool_window_total_weights.entry((pool_key.to_string(), normalized_window.to_string())).or_insert(0.0);
                        *entry += used_fraction * w;
                    }
                }
            }
        }
    }

    struct RawBucketCalc {
        account_idx: usize,
        tier_key: String,
        pool_key: String,
        window_key: String,
        pool_opt: Option<QuotaPool>,
        fraction_known: bool,
        valuation: BucketValuation,
    }

    let mut raw_calcs: Vec<RawBucketCalc> = Vec::new();

    // Pass 1: 实测与分摊计算，沉淀样本
    for (acc_idx, account) in accounts_data.iter().enumerate() {
        let is_active = current_acc_id == Some(account.id.as_str());
        let account_last_used = (account.last_used > 0).then_some(account.last_used);
        let other_max_last_used = accounts_data
            .iter()
            .filter(|a| a.id != account.id && a.last_used > 0)
            .map(|a| a.last_used)
            .max();

        let raw_tier = account
            .quota
            .as_ref()
            .and_then(|q| q.subscription_tier.as_deref())
            .unwrap_or("unknown");
        let tier_key = normalize_tier(raw_tier);

        if let Some(quota_data) = &account.quota {
            let effective_buckets = get_account_effective_buckets(quota_data, now_ts);
            for (bucket, group_name, pool_opt) in effective_buckets {
                let pool_key = match pool_opt {
                    Some(QuotaPool::Gemini) => "gemini",
                    Some(QuotaPool::ThirdParty) => "3p",
                    Some(QuotaPool::Argon) => "argon",
                    None => "unknown",
                };
                let normalized_window = if bucket.window.to_ascii_lowercase().contains("week") {
                    "weekly"
                } else {
                    "5h"
                };

                let fraction_known = bucket.remaining_fraction_known != Some(false) && bucket.remaining_fraction.is_finite();
                let used_fraction = if fraction_known {
                    (1.0 - bucket.remaining_fraction.clamp(0.0, 1.0)).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                let total_weight = pool_window_total_weights.get(&(pool_key.to_string(), normalized_window.to_string())).copied().unwrap_or(0.0);
                let cold_start_share = if used_fraction > 0.0 && total_weight > 0.0 {
                    (used_fraction * tier_weight(tier_key)) / total_weight
                } else {
                    0.0
                };

                let bv = valuate_bucket_with_cold_share(
                    &bucket,
                    &group_name,
                    &account.id,
                    tier_key,
                    is_active,
                    account_last_used,
                    other_max_last_used,
                    now_ts,
                    &spans,
                    &events,
                    &prices_snapshot.prices,
                    &mut calibration_store,
                    &mut calibration_dirty,
                    &config,
                    rate,
                    cold_start_share,
                );

                raw_calcs.push(RawBucketCalc {
                    account_idx: acc_idx,
                    tier_key: tier_key.to_string(),
                    pool_key: pool_key.to_string(),
                    window_key: bucket.window.to_ascii_lowercase(),
                    pool_opt,
                    fraction_known,
                    valuation: bv,
                });
            }
        }
    }

    // Pass 2: 对仍处于 none 或 used_only 的桶赋予参考基准
    for item in &mut raw_calcs {
        apply_reference_cap_pass2(
            &mut item.valuation,
            &item.tier_key,
            &item.pool_key,
            &item.window_key,
            item.pool_opt,
            item.fraction_known,
            &prices_snapshot.prices,
            &calibration_store,
            &config,
            rate,
        );
    }

    if calibration_dirty {
        let _ = save_calibration(&calibration_store);
    }

    // 重新按账号聚合
    let mut account_valuations = Vec::new();
    for (acc_idx, account) in accounts_data.iter().enumerate() {
        let is_active = current_acc_id == Some(account.id.as_str());
        let tier_key = account
            .quota
            .as_ref()
            .and_then(|q| q.subscription_tier.as_deref())
            .unwrap_or("unknown");

        let mut bucket_valuations = Vec::new();
        let mut unpriced_count = 0_u64;

        for item in &raw_calcs {
            if item.account_idx == acc_idx {
                if item.valuation.warning.is_some() {
                    unpriced_count += 1;
                }
                bucket_valuations.push(item.valuation.clone());
            }
        }

        account_valuations.push(AccountQuotaValuation {
            account_id: account.id.clone(),
            email: account.email.clone(),
            subscription_tier: if tier_key == "unknown" { None } else { Some(tier_key.to_string()) },
            is_active,
            buckets: bucket_valuations,
            unpriced_events_count: unpriced_count,
        });
    }

    // 将计算好的折算数据与周期账本同步，并回填内联比较信息
    sync_cycle_valuations(&mut account_valuations, now_ts);

    Ok(QuotaValuationResponse {
        accounts: account_valuations,
        currency: config.quota_valuation.currency,
        display_mode: config.quota_valuation.display_mode,
        usd_to_cny_rate: rate,
        rate_stale: rate_snapshot.stale,
        calculated_at: now_ts,
    })
}

/// 将计算好的估值数据同步到当前活跃周期记录中，并将周期环比/净消耗回填到 BucketValuation 中供前端内联展示
fn sync_cycle_valuations(
    accounts: &mut [AccountQuotaValuation],
    now_ts: i64,
) {
    let _lock = ACCOUNT_USAGE_LOCK.lock();
    let mut store = load_cycle_history();
    let mut store_dirty = false;

    for account in accounts.iter_mut() {
        for bv in account.buckets.iter_mut() {
            let normalized_window = if bv.window.to_ascii_lowercase().contains("week") {
                "weekly"
            } else {
                "5h"
            };
            let pool_key = match bv.pool.as_str() {
                "gemini" => "gemini",
                "third_party" | "3p" => "3p",
                "argon" => "argon",
                _ => "unknown",
            };
            if pool_key == "unknown" {
                continue;
            }

            let composite_key = format!("{}:{}:{}", account.account_id, pool_key, normalized_window);
            let cycle_list = store.buckets.entry(composite_key).or_default();

            // 若尚无 active 周期记录（例如用户升级后第一次打开软件），立即用当前配额自动补种首条记录
            if cycle_list.is_empty() || cycle_list.last().map(|c| c.status.as_str()) != Some("active") {
                let window_secs = window_duration_seconds(&bv.window);
                let reset_ts = parse_rfc3339_timestamp(&bv.reset_time).unwrap_or(now_ts + window_secs);
                let new_rem = bv.remaining_fraction.clamp(0.0, 1.0);
                let new_record = QuotaCycleRecord {
                    cycle_id: format!("{}:{}", normalized_window, reset_ts),
                    account_id: account.account_id.clone(),
                    pool: bv.pool.clone(),
                    window: normalized_window.to_string(),
                    cycle_start_ts: reset_ts - window_secs,
                    reset_ts,
                    reset_time: bv.reset_time.clone(),
                    status: "active".to_string(),
                    first_observed_remaining: new_rem,
                    latest_remaining: new_rem,
                    min_remaining: new_rem,
                    consumed_fraction: (1.0 - new_rem).clamp(0.0, 1.0),
                    last_delta_fraction: 0.0,
                    sample_count: 1,
                    used_usd: bv.used_usd,
                    used_cny: bv.used_cny,
                    used_tokens: bv.used_tokens,
                    estimated_total_usd: bv.estimated_total_usd,
                    estimated_total_cny: bv.estimated_total_cny,
                    estimated_total_tokens: bv.estimated_total_tokens,
                    provenance: bv.provenance.clone(),
                    prev_cycle_total_usd: None,
                    delta_total_usd: None,
                    delta_total_pct: None,
                    checkpoints: vec![CycleCheckpoint { ts: now_ts, remaining_fraction: new_rem }],
                    updated_at: now_ts,
                };
                cycle_list.push(new_record);
                store_dirty = true;
            }

            if let Some(active_cycle) = cycle_list.last_mut() {
                if active_cycle.status == "active" {
                    // 同步最新的折算数据
                    active_cycle.used_usd = bv.used_usd;
                    active_cycle.used_cny = bv.used_cny;
                    active_cycle.used_tokens = bv.used_tokens;
                    active_cycle.estimated_total_usd = bv.estimated_total_usd;
                    active_cycle.estimated_total_cny = bv.estimated_total_cny;
                    active_cycle.estimated_total_tokens = bv.estimated_total_tokens;
                    active_cycle.provenance = bv.provenance.clone();
                    active_cycle.updated_at = now_ts;

                    // 计算跨周期环比
                    if let (Some(cur), Some(prev)) = (active_cycle.estimated_total_usd, active_cycle.prev_cycle_total_usd) {
                        if prev > 1e-6 {
                            let diff = cur - prev;
                            active_cycle.delta_total_usd = Some(diff);
                            active_cycle.delta_total_pct = Some((diff / prev) * 100.0);
                        }
                    }

                    // 回填到 BucketValuation 供前端内联展示
                    bv.last_delta_fraction = Some(active_cycle.last_delta_fraction);
                    bv.consumed_fraction = Some(active_cycle.consumed_fraction);
                    bv.delta_total_usd = active_cycle.delta_total_usd;
                    bv.delta_total_pct = active_cycle.delta_total_pct;
                    store_dirty = true;
                }
            }
        }
    }

    if store_dirty {
        let _ = save_cycle_history(&store);
    }
}

/// 查询周期额度变化历史
pub async fn get_quota_cycle_history(
    target_account_id: Option<String>,
) -> Result<QuotaCycleHistoryResponse, String> {
    // 若尚未有任何周期账本记录（例如新安装或新升级首开），先触发一次全量账号估值计算以完成补种与折算回填
    let needs_seed = {
        let _lock = ACCOUNT_USAGE_LOCK.lock();
        let s = load_cycle_history();
        s.buckets.is_empty()
    };
    if needs_seed {
        let _ = calculate_all_account_valuations().await;
    }

    let config = crate::modules::config::load_app_config()?;
    let rate_snapshot = api_pricing::get_exchange_rate(
        Some(config.quota_valuation.usd_to_cny_rate),
        config.quota_valuation.auto_exchange_rate,
    ).await;
    let rate = rate_snapshot.rate;
    let now_ts = Utc::now().timestamp();

    let index = crate::modules::account::load_account_index()?;

    // 载入历史周期数据
    let store = {
        let _lock = ACCOUNT_USAGE_LOCK.lock();
        load_cycle_history()
    };

    let mut accounts_summary = Vec::new();
    for entry in &index.accounts {
        if let Some(target) = &target_account_id {
            if entry.id != *target {
                continue;
            }
        }

        let acc_opt = crate::modules::account::load_account(&entry.id).ok();
        let raw_tier = acc_opt.as_ref()
            .and_then(|a| a.quota.as_ref())
            .and_then(|q| q.subscription_tier.as_deref())
            .unwrap_or("unknown");
        let tier_key = normalize_tier(raw_tier);

        // 收集该账号的所有周期的记录
        let mut account_records = Vec::new();
        for (key, records) in &store.buckets {
            if key.starts_with(&format!("{}:", entry.id)) {
                for r in records {
                    account_records.push(r.clone());
                }
            }
        }

        // 按 reset_ts 倒序排列（最新的周期在最前）
        account_records.sort_by(|a, b| b.reset_ts.cmp(&a.reset_ts));

        accounts_summary.push(QuotaCycleAccountSummary {
            account_id: entry.id.clone(),
            email: entry.email.clone(),
            subscription_tier: if tier_key == "unknown" { None } else { Some(tier_key.to_string()) },
            records: account_records,
        });
    }

    Ok(QuotaCycleHistoryResponse {
        accounts: accounts_summary,
        currency: config.quota_valuation.currency,
        usd_to_cny_rate: rate,
        fetched_at: now_ts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuous_active_account_does_not_drop_events_in_open_interval() {
        // 场景验证：10:00 (ts=36000) 切入账号 A，10:05~11:30 (ts=36300..41400) 连续对话，11:35 (ts=41700) 查询估值
        let switch_ts = 36000;
        let query_ts = 41700;
        let window_start = 30000;
        let window_end = query_ts;

        let spans = vec![AccountUsageSpan {
            account_id: "acc_active".into(),
            start_ts: switch_ts,
            end_ts: switch_ts, // 模拟切号瞬间 записанный span
        }];

        // 查询时账号仍处于当前激活态 is_active = true
        let intervals = active_intervals_for_account(
            "acc_active",
            window_start,
            window_end,
            &spans,
            true, // is_active
            Some(switch_ts),
            None,
        );

        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0].0, switch_ts);
        // 关键断言：有效结束时间必须自然延续至 window_end (41700)，绝不截断在 36000！
        assert_eq!(intervals[0].1, window_end);

        // 验证 11:00 (ts=39600) 的对话事件能够被判定在区间内
        assert!(is_timestamp_in_intervals(39600, &intervals));
    }

    #[test]
    fn strict_pool_isolation_prevents_cross_pool_counting() {
        let bucket = QuotaBucket {
            bucket_id: "gemini-5h".into(),
            window: "5h".into(),
            remaining_fraction: 0.5,
            remaining_fraction_known: Some(true),
            reset_time: "2026-11-07T12:00:00Z".into(),
            display_name: None,
            description: None,
        };
        let reset_ts = parse_rfc3339_timestamp(&bucket.reset_time).unwrap();
        let now_ts = reset_ts - 3600;

        let spans = vec![AccountUsageSpan {
            account_id: "acc_1".into(),
            start_ts: reset_ts - 10000,
            end_ts: now_ts,
        }];

        let events = vec![
            GenerationEvent {
                model: "gemini-2.5-pro".into(),
                input_tokens: 10_000,
                output_tokens: 2_000,
                cached_tokens: 0,
                total_tokens: 12_000,
                timestamp_seconds: reset_ts - 5000,
            },
            GenerationEvent {
                model: "claude-3-5-sonnet".into(),
                input_tokens: 100_000,
                output_tokens: 50_000,
                cached_tokens: 0,
                total_tokens: 150_000,
                timestamp_seconds: reset_ts - 5000,
            },
        ];

        let prices = vec![
            ApiPricing {
                model: "gemini-2.5-pro".into(),
                input: 1.25,
                output: 5.0,
                cached: 0.3,
            },
            ApiPricing {
                model: "claude-3-5-sonnet".into(),
                input: 3.0,
                output: 15.0,
                cached: 0.3,
            },
        ];

        let mut cal_store = QuotaCapacityCalibrationStore::default();
        let mut cal_dirty = false;
        let config = AppConfig::default();

        let valuation = valuate_bucket(
            &bucket,
            "Gemini Models",
            "acc_1",
            "pro",
            true,
            None,
            None,
            now_ts,
            &spans,
            &events,
            &prices,
            &mut cal_store,
            &mut cal_dirty,
            &config,
            7.2,
        );

        assert_eq!(valuation.pool, "gemini");
        assert_eq!(valuation.used_tokens, 12_000);
        assert!((valuation.used_usd - 0.0225).abs() < 1e-6);
    }

    #[test]
    fn small_consumption_prevents_wild_valuation_explosion() {
        let bucket = QuotaBucket {
            bucket_id: "gemini-5h".into(),
            window: "5h".into(),
            remaining_fraction: 0.9998,
            remaining_fraction_known: Some(true),
            reset_time: "2026-11-07T12:00:00Z".into(),
            display_name: None,
            description: None,
        };
        let reset_ts = parse_rfc3339_timestamp(&bucket.reset_time).unwrap();
        let now_ts = reset_ts - 3600;

        let spans = vec![AccountUsageSpan {
            account_id: "acc_1".into(),
            start_ts: reset_ts - 10000,
            end_ts: now_ts,
        }];

        let events = vec![GenerationEvent {
            model: "gemini-2.5-pro".into(),
            input_tokens: 1_000,
            output_tokens: 100,
            cached_tokens: 0,
            total_tokens: 1_100,
            timestamp_seconds: reset_ts - 5000,
        }];

        let prices = vec![ApiPricing {
            model: "gemini-2.5-pro".into(),
            input: 1.25,
            output: 5.0,
            cached: 0.3,
        }];

        let mut cal_store = QuotaCapacityCalibrationStore::default();
        let mut cal_dirty = false;
        let mut config = AppConfig::default();
        config.quota_valuation.use_reference_cap = false;

        let valuation = valuate_bucket(
            &bucket,
            "Gemini Models",
            "acc_1",
            "pro",
            true,
            None,
            None,
            now_ts,
            &spans,
            &events,
            &prices,
            &mut cal_store,
            &mut cal_dirty,
            &config,
            7.2,
        );

        assert_eq!(valuation.provenance, "used_only");
        assert!(valuation.estimated_total_usd.is_none());
        assert!(valuation.used_usd > 0.0);
    }

    #[test]
    fn unknown_tier_does_not_pollute_calibration_pool() {
        let bucket = QuotaBucket {
            bucket_id: "gemini-5h".into(),
            window: "5h".into(),
            remaining_fraction: 0.5,
            remaining_fraction_known: Some(true),
            reset_time: "2026-11-07T12:00:00Z".into(),
            display_name: None,
            description: None,
        };
        let reset_ts = parse_rfc3339_timestamp(&bucket.reset_time).unwrap();
        let now_ts = reset_ts - 3600;

        let spans = vec![AccountUsageSpan {
            account_id: "acc_unknown".into(),
            start_ts: reset_ts - 10000,
            end_ts: now_ts,
        }];

        let events = vec![GenerationEvent {
            model: "gemini-2.5-pro".into(),
            input_tokens: 1_000_000,
            output_tokens: 200_000,
            cached_tokens: 0,
            total_tokens: 1_200_000,
            timestamp_seconds: reset_ts - 5000,
        }];

        let prices = vec![ApiPricing {
            model: "gemini-2.5-pro".into(),
            input: 1.25,
            output: 5.0,
            cached: 0.3,
        }];

        let mut cal_store = QuotaCapacityCalibrationStore::default();
        let mut cal_dirty = false;
        let config = AppConfig::default();

        let val = valuate_bucket(
            &bucket,
            "Gemini Models",
            "acc_unknown",
            "unknown", // 未知订阅等级！
            true,
            None,
            None,
            now_ts,
            &spans,
            &events,
            &prices,
            &mut cal_store,
            &mut cal_dirty,
            &config,
            7.25,
        );

        assert_eq!(val.provenance, "observed");
        // 关键断言：校准池绝不能被 unknown 等级写入，cal_store 必须保持为空，cal_dirty 必须为 false！
        assert!(cal_store.samples.is_empty());
        assert!(!cal_dirty);
    }

    #[test]
    fn full_quota_uses_builtin_estimated_reference_when_no_calibration() {
        let bucket = QuotaBucket {
            bucket_id: "gemini-5h".into(),
            window: "5h".into(),
            remaining_fraction: 1.0,
            remaining_fraction_known: Some(true),
            reset_time: "2026-11-07T12:00:00Z".into(),
            display_name: None,
            description: None,
        };
        let reset_ts = parse_rfc3339_timestamp(&bucket.reset_time).unwrap();
        let now_ts = reset_ts - 3600;

        let spans = Vec::new();
        let events = Vec::new();
        let prices = vec![ApiPricing {
            model: "gemini-3.8-flash".into(),
            input: 0.75,
            output: 3.75,
            cached: 0.075,
        }];

        let mut cal_store = QuotaCapacityCalibrationStore::default();
        let mut cal_dirty = false;
        let mut config = AppConfig::default();
        config.quota_valuation.use_reference_cap = true;

        let val = valuate_bucket(
            &bucket,
            "Gemini Models",
            "acc_pro",
            "pro",
            true,
            None,
            None,
            now_ts,
            &spans,
            &events,
            &prices,
            &mut cal_store,
            &mut cal_dirty,
            &config,
            7.25,
        );

        assert_eq!(val.provenance, "estimated_reference");
        assert_eq!(val.estimated_total_usd, Some(5.0));
        assert_eq!(val.estimated_remaining_usd, Some(5.0));
        assert!(val.estimated_total_tokens.unwrap() > 0);
    }

    #[test]
    fn unknown_fraction_bucket_never_assigned_reference_cap() {
        let bucket = QuotaBucket {
            bucket_id: "gemini-5h".into(),
            window: "5h".into(),
            remaining_fraction: 0.0,
            remaining_fraction_known: Some(false),
            reset_time: "2026-11-07T12:00:00Z".into(),
            display_name: None,
            description: None,
        };
        let reset_ts = parse_rfc3339_timestamp(&bucket.reset_time).unwrap();
        let now_ts = reset_ts - 3600;

        let spans = Vec::new();
        let events = Vec::new();
        let prices = vec![ApiPricing {
            model: "gemini-3.8-flash".into(),
            input: 0.75,
            output: 3.75,
            cached: 0.075,
        }];

        let mut cal_store = QuotaCapacityCalibrationStore::default();
        let mut cal_dirty = false;
        let mut config = AppConfig::default();
        config.quota_valuation.use_reference_cap = true;

        let val = valuate_bucket(
            &bucket,
            "Gemini Models",
            "acc_pro",
            "pro",
            true,
            None,
            None,
            now_ts,
            &spans,
            &events,
            &prices,
            &mut cal_store,
            &mut cal_dirty,
            &config,
            7.25,
        );

        assert_eq!(val.provenance, "none");
        assert!(val.estimated_total_usd.is_none());
        assert!(val.estimated_remaining_usd.is_none());
        assert!(val.estimated_total_tokens.is_none());
        assert!(val.estimated_remaining_tokens.is_none());
    }

    #[test]
    fn normalize_tier_maps_google_ai_pro_and_starter() {
        assert_eq!(normalize_tier("Google AI Pro"), "pro");
        assert_eq!(normalize_tier("Antigravity Starter Quota"), "free");
        assert_eq!(normalize_tier("Ultra Plan"), "ultra");
        assert_eq!(normalize_tier("unknown"), "unknown");
        assert_eq!(normalize_tier(""), "unknown");

        assert_eq!(builtin_tier_reference_cap_usd("Google AI Pro", "gemini", "5h"), Some(5.0));
        assert_eq!(builtin_tier_reference_cap_usd("Google AI Pro", "3p", "weekly"), Some(25.0));
    }

    #[test]
    fn equivalent_cached_tokens_prevent_token_explosion() {
        let bucket = QuotaBucket {
            bucket_id: "gemini-weekly".into(),
            window: "weekly".into(),
            remaining_fraction: 0.93,
            remaining_fraction_known: Some(true),
            reset_time: "2026-11-14T12:00:00Z".into(),
            display_name: None,
            description: None,
        };
        let reset_ts = parse_rfc3339_timestamp(&bucket.reset_time).unwrap();
        let now_ts = reset_ts - 86400 * 2;

        let spans = vec![AccountUsageSpan {
            account_id: "acc_pro".into(),
            start_ts: reset_ts - 86400 * 7,
            end_ts: now_ts,
        }];

        // 模拟大量提示词缓存命中的事件
        let events = vec![GenerationEvent {
            model: "gemini-3.8-flash".into(),
            input_tokens: 10_000,
            output_tokens: 5_000,
            cached_tokens: 900_000, // 900K 缓存，单价 1/10，等效仅 90K
            total_tokens: 915_000,
            timestamp_seconds: reset_ts - 86400 * 3,
        }];

        let prices = vec![ApiPricing {
            model: "gemini-3.8-flash".into(),
            input: 0.75,
            output: 3.75,
            cached: 0.075,
        }];

        let mut cal_store = QuotaCapacityCalibrationStore::default();
        let mut cal_dirty = false;
        let config = AppConfig::default();

        let val = valuate_bucket(
            &bucket,
            "Gemini Models",
            "acc_pro",
            "Google AI Pro",
            true,
            None,
            None,
            now_ts,
            &spans,
            &events,
            &prices,
            &mut cal_store,
            &mut cal_dirty,
            &config,
            7.25,
        );

        assert_eq!(val.provenance, "observed");
        // 验证等效 tokens 不会爆炸到数亿
        let total_tok = val.estimated_total_tokens.unwrap();
        assert!(total_tok < 10_000_000, "总等效 Token 不应超过 10M，实际为 {}", total_tok);
        assert!(val.estimated_total_usd.is_some());
    }

    static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn intra_cycle_refresh_tracks_delta_and_ignores_noop_refresh() {
        let _env_guard = TEST_ENV_LOCK.lock().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        std::env::set_var("ABV_DATA_DIR", temp_dir.path());

        let account_id = "test_acc_delta";
        let reset_time = "2026-11-08T12:00:00Z";

        // 第一次刷新：100%
        let mut q1 = crate::models::quota::QuotaData::new();
        q1.quota_groups = Some(vec![crate::models::quota::QuotaGroup {
            display_name: "Gemini".into(),
            description: None,
            buckets: vec![QuotaBucket {
                bucket_id: "gemini-5h".into(),
                window: "5h".into(),
                remaining_fraction: 1.0,
                remaining_fraction_known: Some(true),
                reset_time: reset_time.into(),
                display_name: None,
                description: None,
            }],
        }]);
        record_quota_cycle_refresh(account_id, &q1);

        let store1 = load_cycle_history();
        let rec1 = store1.buckets.get("test_acc_delta:gemini:5h").unwrap().last().unwrap();
        assert_eq!(rec1.latest_remaining, 1.0);
        assert_eq!(rec1.consumed_fraction, 0.0);
        assert_eq!(rec1.last_delta_fraction, 0.0);

        // 第二次刷新：降至 85%（实质变化 -0.15）
        let mut q2 = q1.clone();
        q2.quota_groups.as_mut().unwrap()[0].buckets[0].remaining_fraction = 0.85;
        record_quota_cycle_refresh(account_id, &q2);

        let store2 = load_cycle_history();
        let rec2 = store2.buckets.get("test_acc_delta:gemini:5h").unwrap().last().unwrap();
        assert_eq!(rec2.latest_remaining, 0.85);
        assert!((rec2.consumed_fraction - 0.15).abs() < 1e-6);
        assert!((rec2.last_delta_fraction - (-0.15)).abs() < 1e-6);

        // 第三次刷新：依然为 85%（无变化定时刷新，不得把 last_delta 冲为 0）
        record_quota_cycle_refresh(account_id, &q2);
        let store3 = load_cycle_history();
        let rec3 = store3.buckets.get("test_acc_delta:gemini:5h").unwrap().last().unwrap();
        assert_eq!(rec3.latest_remaining, 0.85);
        assert!((rec3.consumed_fraction - 0.15).abs() < 1e-6);
        assert!((rec3.last_delta_fraction - (-0.15)).abs() < 1e-6, "无变化刷新不得冲掉上次的实质 delta");
        assert_eq!(rec3.sample_count, 3);
    }

    #[test]
    fn idle_100_percent_rolling_window_does_not_spam_completed_cycles() {
        let _env_guard = TEST_ENV_LOCK.lock().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        std::env::set_var("ABV_DATA_DIR", temp_dir.path());

        let account_id = "test_acc_idle";

        // 第一次刷新：100% 满额，重置时间为 T0
        let mut q = crate::models::quota::QuotaData::new();
        q.quota_groups = Some(vec![crate::models::quota::QuotaGroup {
            display_name: "Gemini".into(),
            description: None,
            buckets: vec![QuotaBucket {
                bucket_id: "gemini-5h".into(),
                window: "5h".into(),
                remaining_fraction: 1.0,
                remaining_fraction_known: Some(true),
                reset_time: "2026-11-08T12:00:00Z".into(),
                display_name: None,
                description: None,
            }],
        }]);
        record_quota_cycle_refresh(account_id, &q);

        // 第二次刷新：100% 满额，但 Google 滚动窗口顺延了 10 分钟 (+600s)
        q.quota_groups.as_mut().unwrap()[0].buckets[0].reset_time = "2026-11-08T12:10:00Z".into();
        record_quota_cycle_refresh(account_id, &q);

        // 第三次刷新：100% 满额，再次顺延 30 分钟 (+1800s)
        q.quota_groups.as_mut().unwrap()[0].buckets[0].reset_time = "2026-11-08T12:40:00Z".into();
        record_quota_cycle_refresh(account_id, &q);

        let store = load_cycle_history();
        let list = store.buckets.get("test_acc_idle:gemini:5h").unwrap();
        assert_eq!(list.len(), 1, "空转 100% 滚动窗口顺延绝不能刷屏生成 completed 垃圾周期！");
        assert_eq!(list[0].status, "active");
        assert_eq!(list[0].reset_time, "2026-11-08T12:40:00Z");
    }

    #[test]
    fn cycle_reset_archives_previous_cycle_and_computes_coc_delta() {
        let _env_guard = TEST_ENV_LOCK.lock().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        std::env::set_var("ABV_DATA_DIR", temp_dir.path());

        let account_id = "test_acc_coc";

        // 周期 1：消耗至 70%
        let mut q = crate::models::quota::QuotaData::new();
        q.quota_groups = Some(vec![crate::models::quota::QuotaGroup {
            display_name: "Gemini".into(),
            description: None,
            buckets: vec![QuotaBucket {
                bucket_id: "gemini-5h".into(),
                window: "5h".into(),
                remaining_fraction: 0.70,
                remaining_fraction_known: Some(true),
                reset_time: "2026-11-08T12:00:00Z".into(),
                display_name: None,
                description: None,
            }],
        }]);
        record_quota_cycle_refresh(account_id, &q);

        // 模拟周期 1 产生估值：$5.00
        {
            let mut store = load_cycle_history();
            let rec = store.buckets.get_mut("test_acc_coc:gemini:5h").unwrap().last_mut().unwrap();
            rec.estimated_total_usd = Some(5.00);
            save_cycle_history(&store).unwrap();
        }

        // 周期 2：时间跃迁到下一个 5 小时周期，且重置回 100%
        q.quota_groups.as_mut().unwrap()[0].buckets[0].reset_time = "2026-11-08T17:00:00Z".into();
        q.quota_groups.as_mut().unwrap()[0].buckets[0].remaining_fraction = 1.0;
        record_quota_cycle_refresh(account_id, &q);

        let store = load_cycle_history();
        let list = store.buckets.get("test_acc_coc:gemini:5h").unwrap();
        assert_eq!(list.len(), 2, "真实跨周期重置应归档旧周期并生成新活跃周期");
        assert_eq!(list[0].status, "completed");
        assert_eq!(list[0].estimated_total_usd, Some(5.00));
        assert_eq!(list[1].status, "active");
        assert_eq!(list[1].prev_cycle_total_usd, Some(5.00), "新周期应继承上一周期的总额基准");
    }
}
