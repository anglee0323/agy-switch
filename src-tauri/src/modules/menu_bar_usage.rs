//! Today's local usage and API-equivalent estimate. The menu only reads the
//! existing public pricing cache, so opening it never waits for a price fetch.
use super::{api_pricing::{ApiPricing, ApiPricingSnapshot}, native_token_stats::{LocalTokenModel, LocalTokenTotals, LocalTokenUsageSummary}};
use serde::Serialize;
use std::{sync::Mutex, time::{Duration, Instant}};

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
}

pub fn project(summary: &LocalTokenUsageSummary, pricing: Option<&ApiPricingSnapshot>) -> MenuBarUsage {
    let (mut estimated_usd, unpriced_models) = estimate(&summary.by_model_today, pricing.map(|pricing| pricing.prices.as_slice()).unwrap_or_default());
    if summary.today.total_tokens > 0 && summary.by_model_today.is_empty() { estimated_usd = None; }
    MenuBarUsage {
        today: summary.today.clone(), estimated_usd, unpriced_models,
        pricing_stale: pricing.is_none_or(|pricing| pricing.stale),
        incomplete: summary.unreadable_databases > 0 || summary.skipped_large_records > 0,
    }
}

fn normalized(model: &str) -> String {
    // Native -n alias and the explicit EXP-A estimate mapping requested by the owner.
    let model = model.to_ascii_lowercase();
    let model = model.strip_suffix("-n").unwrap_or(&model);
    let model = if model == "gemini-3.8-flash-exp-a" { "gemini-3.8-flash" } else { model };
    model.chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

pub(crate) fn estimate(models: &[LocalTokenModel], prices: &[ApiPricing]) -> (Option<f64>, usize) {
    let mut usd = 0.0;
    let mut priced = 0;
    let mut unpriced = 0;
    for model in models {
        if model.input_tokens == 0 && model.output_tokens == 0 && model.cached_tokens == 0 { continue; }
        let name = normalized(&model.model);
        let mut matching = prices.iter().filter(|price| normalized(&price.model) == name);
        let price = matching.next();
        // Do not guess prices from a substring or silently pick an ambiguous rate.
        if let Some(price) = price.filter(|price| matching.next().is_none() && [price.input, price.output, price.cached].iter().all(|rate| rate.is_finite() && *rate >= 0.0)) {
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
        let mut cache = CachedUsage { day, read_at: Instant::now(), value: MenuBarUsage { today: LocalTokenTotals::default(), estimated_usd: Some(0.0), unpriced_models: 0, pricing_stale: false, incomplete: false } };
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
    fn explicit_experimental_alias_preserves_version_and_variant_boundaries() {
        let (usd, missing) = estimate(&[model("GEMINI-3.8-FLASH-EXP-A")], &[price()]);
        assert!((usd.unwrap() - 1.275).abs() < 1e-9); assert_eq!(missing, 0);
        for name in ["gemini-3.8-flash-exp-b", "gemini-3.7-flash", "gemini-3.8-flash-lite"] {
            assert_eq!(estimate(&[model(name)], &[price()]), (None, 1));
        }
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
