//! Today's local usage and API-equivalent estimate. The menu only reads the
//! existing public pricing cache, so opening it never waits for a price fetch.
use super::{api_pricing::{ApiPricing, ApiPricingSnapshot}, native_token_stats::{LocalTokenModel, LocalTokenTotals, LocalTokenUsageSummary, RecentPerformance}};
use serde::Serialize;
use std::{sync::{Mutex, OnceLock}, time::{Duration, Instant}};

struct CachedUsage { day: chrono::NaiveDate, read_at: Instant, value: MenuBarUsage }
impl CachedUsage {
    fn current(&self, day: chrono::NaiveDate) -> bool { self.day == day }
    fn fresh(&self, day: chrono::NaiveDate) -> bool { self.current(day) && self.read_at.elapsed() < MAX_AGE }
}
static CACHE: Mutex<Option<CachedUsage>> = Mutex::new(None);
static READING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
const MAX_AGE: Duration = Duration::from_secs(30);

/// Opening the native menu reads memory only; midnight never reuses yesterday.
pub fn cached() -> Option<MenuBarUsage> {
    CACHE.lock().ok()?.as_ref().filter(|cache| cache.current(chrono::Local::now().date_naive())).map(|cache| cache.value.clone())
}
fn fresh() -> Option<MenuBarUsage> {
    CACHE.lock().ok()?.as_ref().filter(|cache| cache.fresh(chrono::Local::now().date_naive())).map(|cache| cache.value.clone())
}
pub fn remember(summary: &LocalTokenUsageSummary) -> MenuBarUsage {
    let value = project(summary, super::api_pricing::cached_pricing().as_ref());
    if let Some(time) = chrono::DateTime::from_timestamp(summary.generated_at, 0) {
        if let Ok(mut cache) = CACHE.lock() { *cache = Some(CachedUsage { day: time.with_timezone(&chrono::Local).date_naive(), read_at: Instant::now(), value: value.clone() }); }
    }
    value
}
/// Coalesce concurrent tray requests, while leaving the native stores untouched.
pub async fn load() -> Result<MenuBarUsage, String> {
    let _reading = READING.lock().await;
    if let Some(value) = fresh() { return Ok(value); }
    let summary = tokio::task::spawn_blocking(super::native_token_stats::get_local_token_usage).await.map_err(|_| "usage_task_failed".to_string())??;
    Ok(remember(&summary))
}
pub fn warm() { tauri::async_runtime::spawn(async { let _ = load().await; }); }

#[derive(Debug, Clone, Serialize)]
pub struct MenuBarUsage {
    pub today: LocalTokenTotals,
    pub estimated_usd: Option<f64>,
    pub unpriced_models: usize,
    pub pricing_stale: bool,
    pub incomplete: bool,
    /// Median of recent eligible responses, independent of the today totals.
    pub performance: Option<RecentPerformance>,
}

pub fn project(summary: &LocalTokenUsageSummary, pricing: Option<&ApiPricingSnapshot>) -> MenuBarUsage {
    let (mut estimated_usd, unpriced_models) = estimate(&summary.by_model_today, pricing.map(|pricing| pricing.prices.as_slice()).unwrap_or_default());
    if summary.today.total_tokens > 0 && summary.by_model_today.is_empty() { estimated_usd = None; }
    MenuBarUsage {
        today: summary.today.clone(), estimated_usd, unpriced_models,
        pricing_stale: pricing.is_none_or(|pricing| pricing.stale),
        incomplete: summary.unreadable_databases > 0 || summary.skipped_large_records > 0,
        performance: summary.recent_performance.clone(),
    }
}

fn pricing_keys(model: &str) -> (String, Option<String>) {
    // Keep this policy aligned with src/utils/modelPricing.ts.
    let model = model.to_ascii_lowercase();
    let model = model.strip_suffix("-n").unwrap_or(&model);
    static TOKENS: OnceLock<regex::Regex> = OnceLock::new();
    static GEMINI: OnceLock<regex::Regex> = OnceLock::new();
    let exact = TOKENS.get_or_init(|| regex::Regex::new(r"[a-z]+|[0-9]+").unwrap())
        .find_iter(model).map(|token| token.as_str()).collect::<Vec<_>>().join("-");
    let pattern = GEMINI.get_or_init(|| regex::Regex::new(r"^gemini-([0-9]+(?:-[0-9]+)*)-(flash(?:-lite)?|pro)(?:-(.*))?$").unwrap());
    if let Some(gemini) = pattern.captures(&exact) {
        let suffix: Vec<_> = gemini.get(3).map(|value| value.as_str().split('-').collect()).unwrap_or_default();
        let distinct_variant = suffix.iter().any(|token| ["lite", "image", "vision", "audio", "tts", "live", "embedding", "embeddings", "robotics", "native"].contains(token));
        let mut experimental = false;
        let alias = !distinct_variant && suffix.iter().all(|token| {
            if ["exp", "experimental", "preview", "test"].contains(token) { experimental = true; }
            experimental || (token.len() == 1 && token.as_bytes()[0].is_ascii_lowercase())
                || ["high", "medium", "low", "minimal", "thinking"].contains(token)
        });
        let base = alias.then(|| format!("gemini-{}-{}", &gemini[1], &gemini[2]));
        return (exact, base);
    }
    let name: String = model.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    // App thinking aliases use the same model's token rates. Do not absorb
    // fast, experimental or neighbouring model versions into this mapping.
    if let Some(base) = name.strip_suffix("thinking") {
        if ["claudeopus", "claudesonnet", "claudehaiku"].iter().any(|prefix|
            base.strip_prefix(prefix).is_some_and(|version| !version.is_empty() && version.chars().all(|c| c.is_ascii_digit()))) {
            return (base.to_string(), None);
        }
    }
    (name, None)
}

pub(crate) fn estimate(models: &[LocalTokenModel], prices: &[ApiPricing]) -> (Option<f64>, usize) {
    let mut usd = 0.0;
    let mut priced = 0;
    let mut unpriced = 0;
    let entries: Vec<_> = prices.iter().map(|price| (price, pricing_keys(&price.model))).collect();
    for model in models {
        if model.input_tokens == 0 && model.output_tokens == 0 && model.cached_tokens == 0 { continue; }
        let (exact, base) = pricing_keys(&model.model);
        let mut matching: Vec<_> = entries.iter().filter(|(_, keys)| keys.0 == exact).map(|(price, _)| *price).collect();
        if matching.is_empty() {
            if let Some(base) = base {
                let aliases: Vec<_> = entries.iter().filter(|(_, keys)| keys.1.as_deref() == Some(base.as_str())).collect();
                matching = aliases.iter().filter(|(_, keys)| keys.0 == base).map(|(price, _)| *price).collect();
                if matching.is_empty() { matching = aliases.iter().map(|(price, _)| *price).collect(); }
            }
        }
        let price = matching.first().copied();
        // Do not guess prices from a substring or silently pick an ambiguous rate.
        if let Some(price) = price.filter(|price| matching.len() == 1 && [price.input, price.output, price.cached].iter().all(|rate| rate.is_finite() && *rate >= 0.0)) {
            usd += (model.input_tokens as f64 * price.input + model.output_tokens as f64 * price.output + model.cached_tokens as f64 * price.cached) / 1_000_000.0;
            priced += 1;
        } else { unpriced += 1; }
    }
    (if priced > 0 || unpriced == 0 { Some(usd) } else { None }, unpriced)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model(name: &str) -> LocalTokenModel { LocalTokenModel { model: name.into(), input_tokens: 1_000_000, output_tokens: 100_000, cached_tokens: 2_000_000, total_tokens: 3_100_000, request_count: 3 } }
    fn price() -> ApiPricing { ApiPricing { model: "gemini-3.8-flash".into(), input: 0.75, output: 3.75, cached: 0.075 } }
    #[test]
    fn cached_usage_stays_visible_during_refresh_but_never_crosses_midnight() {
        let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 5).unwrap();
        let mut cache = CachedUsage { day, read_at: Instant::now(), value: MenuBarUsage { today: LocalTokenTotals::default(), estimated_usd: Some(0.0), unpriced_models: 0, pricing_stale: false, incomplete: false, performance: None } };
        assert!(cache.fresh(day));
        cache.read_at -= MAX_AGE;
        assert!(!cache.fresh(day)); assert!(cache.current(day));
        assert!(!cache.current(day.succ_opt().unwrap())); assert!(!cache.fresh(day.succ_opt().unwrap()));
    }
    #[test]
    fn native_alias_and_three_token_components_use_the_same_rate() {
        let (usd, missing) = estimate(&[model("gemini-3.8-flash-n")], &[price()]);
        assert!((usd.unwrap() - 1.275).abs() < 1e-9); assert_eq!(missing, 0);
    }
    #[test]
    fn shared_gemini_aliases_preserve_versions_variants_and_catalog_rates() {
        #[derive(serde::Deserialize)]
        struct Fixture { model: String, prices: Vec<String>, expected: Option<String> }
        let fixtures: Vec<Fixture> = serde_json::from_str(include_str!("../../../tests/fixtures/model-pricing-aliases.json")).unwrap();
        for fixture in fixtures {
            let prices: Vec<_> = fixture.prices.iter().enumerate().map(|(index, name)| {
                let multiplier = (index + 1) as f64;
                ApiPricing { model: name.clone(), input: 0.75 * multiplier, output: 3.75 * multiplier, cached: 0.075 * multiplier }
            }).collect();
            let (usd, missing) = estimate(&[model(&fixture.model)], &prices);
            if let Some(expected) = fixture.expected {
                let index = fixture.prices.iter().position(|name| name == &expected).unwrap();
                assert!((usd.unwrap() - 1.275 * (index + 1) as f64).abs() < 1e-9, "{}", fixture.model);
                assert_eq!(missing, 0, "{}", fixture.model);
            } else { assert_eq!((usd, missing), (None, 1), "{}", fixture.model); }
        }
    }
    #[test]
    fn claude_thinking_alias_uses_exact_input_output_and_cache_rates() {
        let price = ApiPricing { model: "Claude Opus 4.6".into(), input: 5.0, output: 25.0, cached: 0.5 };
        for name in ["claude-opus-4-6-thinking", "Claude Opus 4.6 Thinking-n", "claude-opus-4.6"] {
            assert_eq!(estimate(&[model(name)], &[price.clone()]), (Some(8.5), 0));
        }
        for name in ["claude-opus-4-5-thinking", "claude-opus-4-6-fast", "claude-opus-4-6-thinking-exp", "claude-sonnet-4-6-thinking"] {
            assert_eq!(estimate(&[model(name)], &[price.clone()]), (None, 1));
        }
        assert_eq!(estimate(&[model("claude-opus-4-6-thinking")], &[price.clone(), price]), (None, 1));
    }
    #[test]
    fn unknown_and_ambiguous_models_are_not_free() {
        assert_eq!(estimate(&[model("gemini-3.8-flash-image")], &[price()]), (None, 1));
        assert_eq!(estimate(&[model("gemini-3.8-flash")], &[price(), price()]), (None, 1));
        assert_eq!(estimate(&[model("unknown")], &[]), (None, 1));
        assert_eq!(estimate(&[], &[]), (Some(0.0), 0));
    }
    #[test]
    fn partial_estimate_retains_the_unpriced_count() {
        let (usd, missing) = estimate(&[model("gemini-3.8-flash"), model("unknown")], &[price()]);
        assert!((usd.unwrap() - 1.275).abs() < 1e-9); assert_eq!(missing, 1);
    }
}
