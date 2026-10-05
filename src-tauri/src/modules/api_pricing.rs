use chrono::{NaiveDate, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

const GEMINI_PRICING_URL: &str = "https://ai.google.dev/gemini-api/docs/pricing?hl=en";
const AGENT_PLATFORM_PRICING_URL: &str =
    "https://cloud.google.com/gemini-enterprise-agent-platform/generative-ai/pricing?hl=en";
const CACHE_TTL_SECONDS: i64 = 24 * 60 * 60;
const PARSER_REVISION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiPricing {
    pub model: String,
    pub input: f64,
    pub output: f64,
    pub cached: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiPricingSnapshot {
    #[serde(default)]
    pub parser_revision: u32,
    pub prices: Vec<ApiPricing>,
    pub fetched_at: i64,
    pub stale: bool,
    pub source: String,
    pub warning: Option<String>,
}

/// Fetch public Google pricing pages once per day. Only pricing metadata is cached locally;
/// no account, prompt, or conversation data is sent.
pub async fn get_api_pricing() -> Result<ApiPricingSnapshot, String> {
    let now = Utc::now().timestamp();
    if let Some(mut cached) = read_cache() {
        if cached.parser_revision == PARSER_REVISION && now.saturating_sub(cached.fetched_at) < CACHE_TTL_SECONDS && !cached.prices.is_empty() {
            cached.stale = false;
            cached.warning = None;
            return Ok(cached);
        }
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .user_agent("agy-switch local usage dashboard")
        .build()
        .map_err(|error| error.to_string())?;

    let (gemini_result, agent_platform_result) = tokio::join!(
        fetch_page(&client, GEMINI_PRICING_URL),
        fetch_page(&client, AGENT_PLATFORM_PRICING_URL),
    );

    let today = Utc::now().date_naive();
    let mut prices = Vec::new();
    let mut warnings = Vec::new();

    match gemini_result {
        Ok(html) => prices.extend(parse_gemini_prices(&html, today)),
        Err(error) => warnings.push(format!("Gemini 价格页：{}", error)),
    }
    match agent_platform_result {
        Ok(html) => prices.extend(parse_agent_platform_prices(&html, today)),
        Err(error) => warnings.push(format!("Google Agent Platform 价格页：{}", error)),
    }

    let mut unique_prices = BTreeMap::new();
    for price in prices {
        unique_prices.insert(price.model.to_ascii_lowercase(), price);
    }
    let prices: Vec<ApiPricing> = unique_prices.into_values().collect();

    if !prices.is_empty() {
        let snapshot = ApiPricingSnapshot {
            parser_revision: PARSER_REVISION,
            prices,
            fetched_at: now,
            stale: false,
            source: "Google Gemini API + Google Agent Platform 官方价格".to_string(),
            warning: (!warnings.is_empty()).then(|| warnings.join("；")),
        };
        write_cache(&snapshot);
        return Ok(snapshot);
    }

    if let Some(mut cached) = read_cache() {
        cached.stale = true;
        cached.warning = Some(if warnings.is_empty() {
            "官方价格页暂时没有解析到可用价格，继续使用本地缓存".to_string()
        } else {
            format!("{}；继续使用本地缓存", warnings.join("；"))
        });
        return Ok(cached);
    }

    Ok(ApiPricingSnapshot {
        parser_revision: PARSER_REVISION,
        prices: Vec::new(),
        fetched_at: now,
        stale: true,
        source: "内置价格兜底".to_string(),
        warning: Some(if warnings.is_empty() {
            "官方价格页暂时没有解析到可用价格".to_string()
        } else {
            warnings.join("；")
        }),
    })
}

async fn fetch_page(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status));
    }
    response.text().await.map_err(|error| error.to_string())
}

fn cache_path() -> Option<PathBuf> {
    let root = std::env::var_os("ABV_DATA_DIR").filter(|path| !path.to_string_lossy().trim().is_empty()).map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".antigravity_tools")))?;
    Some(root.join("api_pricing.json"))
}

pub fn cached_pricing() -> Option<ApiPricingSnapshot> {
    let mut snapshot = read_cache()?;
    snapshot.stale = snapshot.parser_revision != PARSER_REVISION || Utc::now().timestamp().saturating_sub(snapshot.fetched_at) >= CACHE_TTL_SECONDS;
    Some(snapshot)
}

fn read_cache() -> Option<ApiPricingSnapshot> {
    let path = cache_path()?;
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_cache(snapshot: &ApiPricingSnapshot) {
    let Some(path) = cache_path() else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(snapshot) {
        let _ = fs::write(path, bytes);
    }
}

fn parse_gemini_prices(html: &str, today: NaiveDate) -> Vec<ApiPricing> {
    let heading_regex = Regex::new(r#"(?is)<h2\b[^>]*\bid="(gemini-[^"]+)"[^>]*>.*?</h2>"#)
        .expect("valid Gemini heading regex");
    let mut headings = Vec::new();
    for capture in heading_regex.captures_iter(html) {
        let Some(full) = capture.get(0) else {
            continue;
        };
        let Some(model) = capture.get(1) else {
            continue;
        };
        headings.push((full.start(), full.end(), model.as_str().to_string()));
    }

    headings
        .iter()
        .enumerate()
        .filter_map(|(index, (_, end, model))| {
            let section_end = headings
                .get(index + 1)
                .map(|heading| heading.0)
                .unwrap_or(html.len());
            parse_gemini_section(&html[*end..section_end], model, today)
        })
        .collect()
}

fn parse_gemini_section(section: &str, model: &str, today: NaiveDate) -> Option<ApiPricing> {
    let standard_heading_regex = Regex::new(r#"(?is)<h3\b[^>]*\bid="standard(?:_\d+)?"[^>]*>.*?</h3>"#)
        .expect("valid Standard heading regex");
    let standard_heading = standard_heading_regex.find(section)?;
    let after_heading = &section[standard_heading.end()..];
    let table_start = after_heading.find("<table")?;
    let table = &after_heading[table_start..];
    let table_end = table.find("</table>")? + "</table>".len();
    let rows = parse_table_rows(&table[..table_end]);

    let mut input = None;
    let mut output = None;
    let mut cached = None;
    for cells in rows {
        if cells.len() < 3 {
            continue;
        }
        let label = cells[0].to_ascii_lowercase();
        if label.starts_with("input price") {
            input = parse_effective_price(&cells[2], today, false);
        } else if label.starts_with("output price") {
            output = parse_effective_price(&cells[2], today, model.contains("image"));
        } else if label.starts_with("context caching price") {
            cached = parse_effective_price(&cells[2], today, false);
        }
    }

    Some(ApiPricing {
        model: model.to_string(),
        input: input?,
        output: output?,
        cached: cached.unwrap_or(0.0),
    })
}

fn parse_agent_platform_prices(html: &str, today: NaiveDate) -> Vec<ApiPricing> {
    let mut records: BTreeMap<String, ApiPricing> = BTreeMap::new();
    let mut current_model = None;

    for cells in parse_table_rows(html) {
        if cells.len() < 3 {
            continue;
        }
        if !cells[0].is_empty() {
            current_model = Some(cells[0].clone());
        }
        let Some(model) = current_model.as_ref() else {
            continue;
        };
        if !model.to_ascii_lowercase().contains("claude") {
            continue;
        }

        let kind = cells[1].to_ascii_lowercase();
        if !matches!(kind.as_str(), "input" | "output" | "cache hit") {
            continue;
        }
        let Some(value) = parse_effective_price(&cells[2], today, false) else {
            continue;
        };
        let entry = records.entry(model.clone()).or_insert_with(|| ApiPricing {
            model: model.clone(),
            input: 0.0,
            output: 0.0,
            cached: 0.0,
        });
        match kind.as_str() {
            "input" if entry.input == 0.0 => entry.input = value,
            "output" if entry.output == 0.0 => entry.output = value,
            "cache hit" if entry.cached == 0.0 => entry.cached = value,
            _ => {}
        }
    }

    records
        .into_values()
        .filter(|price| price.input > 0.0 && price.output > 0.0)
        .collect()
}

fn parse_table_rows(table: &str) -> Vec<Vec<String>> {
    let row_regex = Regex::new(r"(?is)<tr\b[^>]*>(.*?)</tr>").expect("valid row regex");
    let cell_regex = Regex::new(r"(?is)<td\b[^>]*>(.*?)</td>").expect("valid cell regex");
    row_regex
        .captures_iter(table)
        .map(|row| {
            cell_regex
                .captures_iter(row.get(1).map(|value| value.as_str()).unwrap_or_default())
                .map(|cell| clean_text(cell.get(1).map(|value| value.as_str()).unwrap_or_default()))
                .collect()
        })
        .collect()
}

fn clean_text(value: &str) -> String {
    let tag_regex = Regex::new(r"(?is)<[^>]+>").expect("valid tag regex");
    let value = tag_regex.replace_all(value, " ");
    value
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_effective_price(value: &str, today: NaiveDate, prefer_image: bool) -> Option<f64> {
    let value = clean_text(value);
    let money_regex = Regex::new(r"\$(\d+(?:\.\d+)?)").expect("valid money regex");

    if prefer_image {
        let image_regex = Regex::new(r"(?i)\$(\d+(?:\.\d+)?)\s*\(\s*images?\s*\)")
            .expect("valid image price regex");
        if let Some(capture) = image_regex.captures(&value) {
            return capture.get(1)?.as_str().parse().ok();
        }
    }

    let matches: Vec<_> = money_regex.find_iter(&value).collect();
    let mut fallback = None;
    let date_regex = Regex::new(r"(?i)(through|starting)\s+([A-Za-z]+\s+\d{1,2},\s+\d{4})")
        .expect("valid price date regex");

    for (index, money) in matches.iter().enumerate() {
        let amount = money.as_str().trim_start_matches('$').parse::<f64>().ok()?;
        fallback.get_or_insert(amount);
        let next_start = matches
            .get(index + 1)
            .map(|next| next.start())
            .unwrap_or(value.len());
        let context = &value[money.end()..next_start];
        let Some(date_capture) = date_regex.captures(context) else {
            continue;
        };
        let Some(date_text) = date_capture.get(2).map(|date| date.as_str()) else {
            continue;
        };
        let Some(date) = parse_price_date(date_text) else {
            continue;
        };
        let kind = date_capture
            .get(1)
            .map(|kind| kind.as_str())
            .unwrap_or_default();
        if (kind.eq_ignore_ascii_case("through") && today <= date)
            || (kind.eq_ignore_ascii_case("starting") && today >= date)
        {
            return Some(amount);
        }
    }

    fallback
}

fn parse_price_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%B %d, %Y")
        .or_else(|_| NaiveDate::parse_from_str(value, "%B %-d, %Y"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_current_price_from_a_scheduled_change() {
        let value = "$0.75 through December 31, 2026. $1.50 starting January 1, 2027.";
        assert_eq!(
            parse_effective_price(value, NaiveDate::from_ymd_opt(2026, 9, 15).unwrap(), false),
            Some(0.75)
        );
        assert_eq!(
            parse_effective_price(value, NaiveDate::from_ymd_opt(2027, 1, 2).unwrap(), false),
            Some(1.5)
        );
    }

    #[test]
    fn picks_image_output_price_when_a_model_has_two_output_types() {
        let value = "$3 (text and thinking) $60.00 (images)";
        assert_eq!(
            parse_effective_price(value, NaiveDate::from_ymd_opt(2026, 9, 15).unwrap(), true),
            Some(60.0)
        );
    }

    #[test]
    fn parses_numbered_standard_sections_without_using_batch_rates() {
        let html = r#"
            <h2 id="gemini-3.8-flash">Flash</h2>
            <h3 id="standard">Standard</h3><table>
              <tr><td>Input price</td><td>Free</td><td>$0.75</td></tr>
              <tr><td>Output price</td><td>Free</td><td>$3.75</td></tr>
              <tr><td>Context caching price</td><td>Free</td><td>$0.075</td></tr>
            </table>
            <h2 id="gemini-3.7-flash">Previous Flash</h2>
            <h3 id="batch_1">Batch</h3><table>
              <tr><td>Input price</td><td>Free</td><td>$0.375</td></tr>
              <tr><td>Output price</td><td>Free</td><td>$1.875</td></tr>
            </table>
            <h3 id="standard_1">Standard</h3><table>
              <tr><td>Input price</td><td>Free</td><td>$0.75</td></tr>
              <tr><td>Output price</td><td>Free</td><td>$3.75</td></tr>
              <tr><td>Context caching price</td><td>Free</td><td>$0.075</td></tr>
            </table>
        "#;
        let prices = parse_gemini_prices(html, NaiveDate::from_ymd_opt(2026, 10, 5).unwrap());
        assert_eq!(prices.len(), 2);
        assert_eq!(prices[1].model, "gemini-3.7-flash");
        assert_eq!((prices[1].input, prices[1].output, prices[1].cached), (0.75, 3.75, 0.075));
    }

    #[test]
    fn parses_gemini_model_sections() {
        let html = r#"
            <h2 id="gemini-3.9-flash">Gemini 3.9 Flash</h2>
            <h3 id="standard">Standard</h3>
            <table>
              <tr><td>Input price</td><td>Free</td><td>$0.80</td></tr>
              <tr><td>Output price</td><td>Free</td><td>$4.00</td></tr>
              <tr><td>Context caching price</td><td>Free</td><td>$0.08</td></tr>
            </table>
        "#;
        let prices = parse_gemini_prices(html, NaiveDate::from_ymd_opt(2026, 9, 15).unwrap());
        assert_eq!(prices.len(), 1);
        assert_eq!(prices[0].model, "gemini-3.9-flash");
        assert_eq!(prices[0].output, 4.0);
    }
}
