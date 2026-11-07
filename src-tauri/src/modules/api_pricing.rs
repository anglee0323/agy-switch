use chrono::{NaiveDate, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

const GEMINI_PRICING_URL: &str = "https://ai.google.dev/gemini-api/docs/pricing.md.txt";
const AGENT_PLATFORM_PRICING_URL: &str =
    "https://platform.claude.com/docs/en/about-claude/pricing.md";
const EXCHANGE_RATE_API_URL: &str = "https://open.er-api.com/v6/latest/USD";
const CACHE_TTL_SECONDS: i64 = 24 * 60 * 60;
const PARSER_REVISION: u32 = 4;
const DEFAULT_USD_TO_CNY: f64 = 7.25;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeRateSnapshot {
    pub rate: f64,
    pub fetched_at: i64,
    pub stale: bool,
    pub source: String,
    pub warning: Option<String>,
}

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

    let client = crate::utils::proxy::configure_reqwest(reqwest::Client::builder())?
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
    for price in builtin_fallback_prices() {
        unique_prices.insert(price.model.to_ascii_lowercase(), price);
    }
    for price in prices {
        unique_prices.insert(price.model.to_ascii_lowercase(), price);
    }
    let combined_prices: Vec<ApiPricing> = unique_prices.into_values().collect();

    let snapshot = ApiPricingSnapshot {
        parser_revision: PARSER_REVISION,
        prices: combined_prices,
        fetched_at: now,
        stale: !warnings.is_empty(),
        source: if warnings.is_empty() {
            "Google Gemini API + Google Agent Platform 官方价格".to_string()
        } else {
            "Google 官方价格与内置标准价格融合".to_string()
        },
        warning: (!warnings.is_empty()).then(|| warnings.join("；")),
    };
    write_cache(&snapshot);
    Ok(snapshot)
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

fn exchange_rate_cache_path() -> Option<PathBuf> {
    let root = std::env::var_os("ABV_DATA_DIR").filter(|path| !path.to_string_lossy().trim().is_empty()).map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".antigravity_tools")))?;
    Some(root.join("exchange_rate.json"))
}

fn read_exchange_rate_cache() -> Option<ExchangeRateSnapshot> {
    let path = exchange_rate_cache_path()?;
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_exchange_rate_cache(snapshot: &ExchangeRateSnapshot) {
    let Some(path) = exchange_rate_cache_path() else { return; };
    let Some(parent) = path.parent() else { return; };
    if fs::create_dir_all(parent).is_err() { return; }
    if let Ok(bytes) = serde_json::to_vec_pretty(snapshot) {
        let _ = fs::write(path, bytes);
    }
}

/// 获取 USD -> CNY 汇率。支持 24 小时本地缓存与优雅回退，绝不阻塞应用运行。
pub async fn get_exchange_rate(config_rate: Option<f64>, auto_fetch: bool) -> ExchangeRateSnapshot {
    let fallback_rate = config_rate
        .filter(|r| r.is_finite() && (3.0..=20.0).contains(r))
        .unwrap_or(DEFAULT_USD_TO_CNY);
    let now = Utc::now().timestamp();

    if !auto_fetch {
        return ExchangeRateSnapshot {
            rate: fallback_rate,
            fetched_at: now,
            stale: false,
            source: "用户固定汇率配置".to_string(),
            warning: None,
        };
    }

    if let Some(mut cached) = read_exchange_rate_cache() {
        if now.saturating_sub(cached.fetched_at) < CACHE_TTL_SECONDS && (3.0..=20.0).contains(&cached.rate) {
            cached.stale = false;
            cached.warning = None;
            return cached;
        }
    }

    let client = match crate::utils::proxy::configure_reqwest(reqwest::Client::builder()) {
        Ok(builder) => builder
            .timeout(std::time::Duration::from_secs(8))
            .user_agent("agy-switch exchange rate fetcher")
            .build()
            .map_err(|e| e.to_string()),
        Err(e) => Err(e),
    };

    let fetch_res = match client {
        Ok(cli) => fetch_open_exchange_rate(&cli).await,
        Err(e) => Err(e),
    };

    match fetch_res {
        Ok(parsed_rate) if (3.0..=20.0).contains(&parsed_rate) => {
            let snapshot = ExchangeRateSnapshot {
                rate: parsed_rate,
                fetched_at: now,
                stale: false,
                source: "open.er-api.com 实时汇率".to_string(),
                warning: None,
            };
            write_exchange_rate_cache(&snapshot);
            snapshot
        }
        Err(err) => {
            if let Some(mut cached) = read_exchange_rate_cache() {
                if (3.0..=20.0).contains(&cached.rate) {
                    cached.stale = true;
                    cached.warning = Some(format!("获取最新汇率失败（{}），使用本地缓存汇率", err));
                    return cached;
                }
            }
            ExchangeRateSnapshot {
                rate: fallback_rate,
                fetched_at: now,
                stale: true,
                source: "配置默认汇率兜底".to_string(),
                warning: Some(format!("汇率接口不可用（{}），使用配置兜底", err)),
            }
        }
        _ => {
            if let Some(mut cached) = read_exchange_rate_cache() {
                if (3.0..=20.0).contains(&cached.rate) {
                    cached.stale = true;
                    cached.warning = Some("最新汇率数值超出正常范围，使用本地缓存汇率".to_string());
                    return cached;
                }
            }
            ExchangeRateSnapshot {
                rate: fallback_rate,
                fetched_at: now,
                stale: true,
                source: "配置默认汇率兜底".to_string(),
                warning: Some("最新汇率数值异常，使用配置兜底".to_string()),
            }
        }
    }
}

async fn fetch_open_exchange_rate(client: &reqwest::Client) -> Result<f64, String> {
    let response = client
        .get(EXCHANGE_RATE_API_URL)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let body = response.text().await.map_err(|e| e.to_string())?;
    parse_exchange_rate_json(&body)
}

pub fn parse_exchange_rate_json(json_str: &str) -> Result<f64, String> {
    let val: serde_json::Value = serde_json::from_str(json_str).map_err(|e| e.to_string())?;
    let cny = val.get("rates").and_then(|r| r.get("CNY")).and_then(|v| v.as_f64())
        .ok_or_else(|| "rates.CNY not found in response".to_string())?;
    Ok(cny)
}

pub fn builtin_fallback_prices() -> Vec<ApiPricing> {
    vec![
        ApiPricing { model: "gemini-4-argon".into(), input: 2.0, output: 10.0, cached: 0.10 },
        ApiPricing { model: "gemini-3.8-flash".into(), input: 0.75, output: 3.75, cached: 0.075 },
        ApiPricing { model: "gemini-3.6-flash".into(), input: 0.50, output: 2.50, cached: 0.05 },
        ApiPricing { model: "gemini-3.5-flash-lite".into(), input: 0.20, output: 1.00, cached: 0.02 },
        ApiPricing { model: "gemini-3.1-pro-preview".into(), input: 2.00, output: 12.00, cached: 0.20 },
        ApiPricing { model: "gemini-3.1-flash-lite".into(), input: 0.25, output: 1.50, cached: 0.025 },
        ApiPricing { model: "gemini-3-flash-preview".into(), input: 0.50, output: 3.00, cached: 0.05 },
        ApiPricing { model: "gemini-2.5-pro".into(), input: 1.25, output: 10.00, cached: 0.125 },
        ApiPricing { model: "gemini-2.5-flash".into(), input: 0.30, output: 2.50, cached: 0.03 },
        ApiPricing { model: "gemini-2.5-flash-lite".into(), input: 0.10, output: 0.40, cached: 0.01 },
        ApiPricing { model: "Claude Opus 5.5".into(), input: 4.0, output: 20.0, cached: 0.20 },
        ApiPricing { model: "Claude Opus 5".into(), input: 5.0, output: 25.0, cached: 0.50 },
        ApiPricing { model: "Claude Opus 4.8".into(), input: 5.0, output: 25.0, cached: 0.50 },
        ApiPricing { model: "Claude Opus 4.7".into(), input: 5.0, output: 25.0, cached: 0.50 },
        ApiPricing { model: "Claude Opus 4.6".into(), input: 5.0, output: 25.0, cached: 0.50 },
        ApiPricing { model: "Claude Opus 4.5".into(), input: 5.0, output: 25.0, cached: 0.50 },
        ApiPricing { model: "Claude Sonnet 5.5".into(), input: 2.0, output: 10.0, cached: 0.10 },
        ApiPricing { model: "Claude Sonnet 5".into(), input: 2.0, output: 10.0, cached: 0.20 },
        ApiPricing { model: "Claude Sonnet 4.6".into(), input: 3.0, output: 15.0, cached: 0.30 },
        ApiPricing { model: "Claude Sonnet 4.5".into(), input: 3.0, output: 15.0, cached: 0.30 },
        ApiPricing { model: "Claude Haiku 5.5".into(), input: 0.10, output: 0.50, cached: 0.01 },
        ApiPricing { model: "Claude Haiku 4.5".into(), input: 1.0, output: 5.0, cached: 0.10 },
    ]
}

pub fn cached_pricing() -> Option<ApiPricingSnapshot> {
    if let Some(mut snapshot) = read_cache() {
        snapshot.stale = snapshot.parser_revision != PARSER_REVISION || Utc::now().timestamp().saturating_sub(snapshot.fetched_at) >= CACHE_TTL_SECONDS;
        return Some(snapshot);
    }
    Some(ApiPricingSnapshot {
        parser_revision: PARSER_REVISION,
        prices: builtin_fallback_prices(),
        fetched_at: Utc::now().timestamp(),
        stale: true,
        source: "内置价格兜底".to_string(),
        warning: None,
    })
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

fn parse_gemini_prices(content: &str, today: NaiveDate) -> Vec<ApiPricing> {
    if content.contains("<h2") {
        return parse_gemini_html_prices(content, today);
    }
    parse_gemini_markdown_prices(content, today)
}

fn parse_gemini_html_prices(html: &str, today: NaiveDate) -> Vec<ApiPricing> {
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

fn parse_gemini_markdown_prices(content: &str, today: NaiveDate) -> Vec<ApiPricing> {
    let heading_regex = Regex::new(r#"(?m)^##\s+[^\n]*?\{%\s*id="(gemini-[^"]+)"\s*%\}"#)
        .expect("valid markdown heading regex");
    let mut headings = Vec::new();
    for capture in heading_regex.captures_iter(content) {
        let Some(full) = capture.get(0) else { continue; };
        let Some(model) = capture.get(1) else { continue; };
        headings.push((full.start(), full.end(), model.as_str().to_string()));
    }

    let mut results = Vec::new();
    for (index, (_, end, model)) in headings.iter().enumerate() {
        let section_end = headings.get(index + 1).map(|h| h.0).unwrap_or(content.len());
        let section = &content[*end..section_end];

        let tab_content = if let Some(std_start) = section.find(r#"{% tab title="Standard" %}"#) {
            let after_std = &section[std_start + r#"{% tab title="Standard" %}"#.len()..];
            if let Some(tab_end) = after_std.find("{% /tab %}") {
                &after_std[..tab_end]
            } else {
                after_std
            }
        } else {
            section
        };

        let table_content = if let Some(tbl_start) = tab_content.find("{% table %}") {
            let after_tbl = &tab_content[tbl_start + "{% table %}".len()..];
            if let Some(tbl_end) = after_tbl.find("{% /table %}") {
                &after_tbl[..tbl_end]
            } else {
                after_tbl
            }
        } else {
            tab_content
        };

        let rows = parse_markdown_table_rows(table_content);
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

        if let (Some(inp), Some(out)) = (input, output) {
            results.push(ApiPricing {
                model: model.clone(),
                input: inp,
                output: out,
                cached: cached.unwrap_or(0.0),
            });
        }
    }

    results
}

fn parse_markdown_table_rows(table_content: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let row_blocks = table_content.split("\n---");
    for block in row_blocks {
        let mut cells = Vec::new();
        let mut current_cell: Option<String> = None;
        for line in block.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("* ") {
                if let Some(c) = current_cell.take() {
                    cells.push(c);
                }
                current_cell = Some(trimmed[2..].trim().to_string());
            } else if let Some(ref mut c) = current_cell {
                if !trimmed.is_empty() {
                    c.push(' ');
                    c.push_str(trimmed);
                }
            }
        }
        if let Some(c) = current_cell {
            cells.push(c);
        }
        if !cells.is_empty() {
            rows.push(cells);
        }
    }
    rows
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

fn parse_agent_platform_prices(content: &str, today: NaiveDate) -> Vec<ApiPricing> {
    if content.contains("<table") || content.contains("<tr") {
        return parse_agent_platform_html_prices(content, today);
    }
    parse_agent_platform_markdown_prices(content, today)
}

fn parse_agent_platform_html_prices(html: &str, today: NaiveDate) -> Vec<ApiPricing> {
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

fn parse_agent_platform_markdown_prices(content: &str, today: NaiveDate) -> Vec<ApiPricing> {
    let mut records: BTreeMap<String, ApiPricing> = BTreeMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        let mut cells: Vec<String> = trimmed
            .split('|')
            .map(|s| s.trim().to_string())
            .collect();
        if cells.first().map(|s| s.is_empty()).unwrap_or(false) {
            cells.remove(0);
        }
        if cells.last().map(|s| s.is_empty()).unwrap_or(false) {
            cells.pop();
        }
        if cells.len() < 6 {
            continue;
        }

        let raw_name = &cells[0];
        let name_without_paren = match raw_name.find('(') {
            Some(idx) => &raw_name[..idx],
            None => raw_name.as_str(),
        };
        let clean_name = name_without_paren.replace("**", "").trim().to_string();
        let lower_name = clean_name.to_ascii_lowercase();

        if lower_name.starts_with("claude ")
            && (lower_name.contains("opus") || lower_name.contains("sonnet") || lower_name.contains("haiku"))
        {
            if !records.contains_key(&lower_name) {
                let input = parse_effective_price(&cells[1], today, false);
                let cached = parse_effective_price(&cells[4], today, false);
                let output = parse_effective_price(&cells[5], today, false);
                if let (Some(inp), Some(out)) = (input, output) {
                    if inp > 0.0 && out > 0.0 {
                        records.insert(lower_name, ApiPricing {
                            model: clean_name,
                            input: inp,
                            output: out,
                            cached: cached.unwrap_or(0.0),
                        });
                    }
                }
            }
        }
    }
    records.into_values().collect()
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
    fn parses_claude_global_rates_without_batch_or_regional_overrides() {
        let html = r#"<table>
            <tr><td>Claude Opus 4.6</td><td>Input</td><td>$5.00</td></tr>
            <tr><td></td><td>Output</td><td>$25.00</td></tr>
            <tr><td></td><td>Batch Input</td><td>$2.50</td></tr>
            <tr><td></td><td>5m Cache Write</td><td>$6.25</td></tr>
            <tr><td></td><td>Cache Hit</td><td>$0.50</td></tr>
            </table><table>
            <tr><td>Claude Opus 4.6</td><td>Input</td><td>$5.50</td></tr>
            <tr><td></td><td>Output</td><td>$27.50</td></tr>
            <tr><td></td><td>Cache Hit</td><td>$0.55</td></tr>
            </table>"#;
        let prices = parse_agent_platform_prices(html, NaiveDate::from_ymd_opt(2026, 10, 9).unwrap());
        assert_eq!(prices.len(), 1);
        assert_eq!((prices[0].input, prices[0].output, prices[0].cached), (5.0, 25.0, 0.5));
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

    #[test]
    fn parses_valid_and_invalid_exchange_rate_json() {
        let valid_json = r#"{"result":"success","rates":{"CNY":7.2345,"EUR":0.92}}"#;
        assert_eq!(parse_exchange_rate_json(valid_json).unwrap(), 7.2345);

        let missing_cny = r#"{"result":"success","rates":{"EUR":0.92}}"#;
        assert!(parse_exchange_rate_json(missing_cny).is_err());

        let invalid_json = "not a json";
        assert!(parse_exchange_rate_json(invalid_json).is_err());
    }

    #[test]
    fn parses_gemini_markdown_format() {
        let md = r#"
## Gemini 4 Argon  {% id="gemini-4-argon" %}

{% tab title="Standard" %}
{% table %}
* Input price
* Free
* $2.00
---
* Output price
* Free
* $10.00
---
* Context caching price
* Free
* $0.10
{% /table %}
{% /tab %}

## Gemini 3.8 Flash  {% id="gemini-3.8-flash" %}

{% tab title="Standard" %}
{% table %}
* Input price
* Free
* $0.75
---
* Output price
* Free
* $3.75
---
* Context caching price
* Free
* $0.075
{% /table %}
{% /tab %}
        "#;
        let prices = parse_gemini_prices(md, NaiveDate::from_ymd_opt(2026, 10, 5).unwrap());
        assert_eq!(prices.len(), 2);
        assert_eq!(prices[0].model, "gemini-4-argon");
        assert_eq!((prices[0].input, prices[0].output, prices[0].cached), (2.0, 10.0, 0.10));
        assert_eq!(prices[1].model, "gemini-3.8-flash");
        assert_eq!((prices[1].input, prices[1].output, prices[1].cached), (0.75, 3.75, 0.075));
    }

    #[test]
    fn parses_anthropic_markdown_pricing_table() {
        let md = r#"
| Model | Base input tokens | 5m cache writes | 1h cache writes | Cache hits and refreshes | Output tokens |
|---|---|---|---|---|---|
| **Claude Opus 4.6** | $5.00 | $6.25 | $10.00 | $0.50 | $25.00 |
| **Claude Sonnet 4.6** | $3.00 | $3.75 | $6.00 | $0.30 | $15.00 |
| **Claude Haiku 5.5** (for prompts up to 100,000 tokens) | $0.10 | $0.125 | $0.20 | $0.01 | $0.50 |
        "#;
        let prices = parse_agent_platform_prices(md, NaiveDate::from_ymd_opt(2026, 10, 5).unwrap());
        assert_eq!(prices.len(), 3);
        let opus = prices.iter().find(|p| p.model == "Claude Opus 4.6").unwrap();
        assert_eq!((opus.input, opus.output, opus.cached), (5.0, 25.0, 0.50));
        let sonnet = prices.iter().find(|p| p.model == "Claude Sonnet 4.6").unwrap();
        assert_eq!((sonnet.input, sonnet.output, sonnet.cached), (3.0, 15.0, 0.30));
        let haiku = prices.iter().find(|p| p.model == "Claude Haiku 5.5").unwrap();
        assert_eq!((haiku.input, haiku.output, haiku.cached), (0.10, 0.50, 0.01));
    }
}
