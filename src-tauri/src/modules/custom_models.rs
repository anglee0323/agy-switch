use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_true() -> bool {
    true
}

fn default_api_format() -> String {
    "openai".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CustomModelEntry {
    /// Unique internal identifier, typically "models/<id>"
    pub name: String,
    /// Display label in UI and chat dropdown
    pub display_name: String,
    /// Provider ID: "deepseek" | "openrouter" | "custom"
    pub provider: String,
    /// Wire format: "openai" | "anthropic" | "google"
    #[serde(default = "default_api_format")]
    pub api_format: String,
    /// Endpoint URL
    pub api_url: String,
    /// API Key
    #[serde(default)]
    pub api_key: String,
    /// Actual upstream model name passed in request body
    pub external_model_name: String,
    /// Whether this model is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Optional context window limit
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    /// Optional max output tokens
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    /// Optional reasoning effort
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum CustomModelsFileFormat {
    Object { models: Vec<CustomModelEntry> },
    List(Vec<CustomModelEntry>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TestConnectionResult {
    pub success: bool,
    pub latency_ms: Option<u64>,
    pub message: String,
}

/// Returns the standard path to `~/.gemini/antigravity/custom_models.json`.
pub fn get_custom_models_file_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Could not determine home directory".to_string())?;
    Ok(home.join(".gemini").join("antigravity").join("custom_models.json"))
}

/// Loads all custom models from `custom_models.json`.
/// Returns an empty list if the file does not exist yet.
pub fn load_custom_models() -> Result<Vec<CustomModelEntry>, String> {
    let path = get_custom_models_file_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read custom_models.json: {}", e))?;

    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    let parsed: CustomModelsFileFormat = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse custom_models.json: {}", e))?;

    let models = match parsed {
        CustomModelsFileFormat::Object { models } => models,
        CustomModelsFileFormat::List(models) => models,
    };

    Ok(models)
}

/// Atomically saves custom models to `custom_models.json`.
pub fn save_custom_models(mut models: Vec<CustomModelEntry>) -> Result<(), String> {
    let path = get_custom_models_file_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {:?}: {}", parent, e))?;
    }

    // Normalization
    for model in &mut models {
        let trimmed_name = model.name.trim();
        if trimmed_name.is_empty() {
            let sanitized_external = model.external_model_name.trim().replace('/', "-");
            model.name = format!("models/{}", sanitized_external);
        } else if !trimmed_name.starts_with("models/") {
            model.name = format!("models/{}", trimmed_name);
        } else {
            model.name = trimmed_name.to_string();
        }

        if model.display_name.trim().is_empty() {
            model.display_name = model.external_model_name.clone();
        } else {
            model.display_name = model.display_name.trim().to_string();
        }

        model.api_url = model.api_url.trim().to_string();
        model.external_model_name = model.external_model_name.trim().to_string();
        model.api_key = model.api_key.trim().to_string();
    }

    let payload = serde_json::json!({
        "models": models,
    });

    let json_bytes = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("Failed to serialize custom models: {}", e))?;

    crate::utils::fs::write_atomic(&path, json_bytes.as_bytes())
}

/// Real network ping / completion test for a custom model configuration.
pub async fn test_connection(entry: &CustomModelEntry) -> Result<TestConnectionResult, String> {
    let url_str = entry.api_url.trim();
    if url_str.is_empty() {
        return Ok(TestConnectionResult {
            success: false,
            latency_ms: None,
            message: "API URL cannot be empty".to_string(),
        });
    }

    let url = reqwest::Url::parse(url_str).map_err(|e| format!("Invalid API URL: {}", e))?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let start_time = std::time::Instant::now();

    let mut req = client.post(url.clone());
    if !entry.api_key.trim().is_empty() {
        req = req.header("Authorization", format!("Bearer {}", entry.api_key.trim()));
    }
    req = req.header("Content-Type", "application/json");

    if entry.provider == "openrouter" || url_str.contains("openrouter.ai") {
        req = req.header("HTTP-Referer", "https://github.com/anglee0323/antigravity-tools-lite");
        req = req.header("X-Title", "Antigravity Tools Lite");
    }

    let model_id = if entry.external_model_name.trim().is_empty() {
        "default"
    } else {
        entry.external_model_name.trim()
    };

    let body = serde_json::json!({
        "model": model_id,
        "messages": [
            { "role": "user", "content": "ping" }
        ],
        "max_tokens": 1
    });

    let body_str = serde_json::to_string(&body).map_err(|e| format!("序列化请求失败: {}", e))?;
    match req.body(body_str).send().await {
        Ok(resp) => {
            let latency_ms = start_time.elapsed().as_millis() as u64;
            let status = resp.status();
            if status.is_success() {
                Ok(TestConnectionResult {
                    success: true,
                    latency_ms: Some(latency_ms),
                    message: format!("HTTP 200 OK ({}ms)", latency_ms),
                })
            } else {
                let status_code = status.as_u16();
                let text = resp.text().await.unwrap_or_default();
                let snippet = if text.len() > 150 {
                    format!("{}...", &text[..150])
                } else {
                    text
                };

                let err_msg = match status_code {
                    401 => "认证失败 (HTTP 401)：API Key 无效或未提供".to_string(),
                    403 => format!("权限拒绝 (HTTP 403)：{}", snippet),
                    404 => format!("未找到模型或端点 (HTTP 404)：{}", snippet),
                    429 => format!("额度超限或受限 (HTTP 429)：{}", snippet),
                    _ => format!("HTTP 响应码 {}：{}", status_code, snippet),
                };

                Ok(TestConnectionResult {
                    success: false,
                    latency_ms: Some(latency_ms),
                    message: err_msg,
                })
            }
        }
        Err(e) => {
            let msg = if e.is_timeout() {
                "连接超时 (12s)，请检查网络或代理设置".to_string()
            } else if e.is_connect() {
                "无法连接到服务器，请检查 API 地址是否可达".to_string()
            } else {
                format!("请求错误: {}", e)
            };
            Ok(TestConnectionResult {
                success: false,
                latency_ms: None,
                message: msg,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_model_format_serialization() {
        let entry = CustomModelEntry {
            name: "models/deepseek-chat".to_string(),
            display_name: "DeepSeek V3".to_string(),
            provider: "deepseek".to_string(),
            api_format: "openai".to_string(),
            api_url: "https://api.deepseek.com/chat/completions".to_string(),
            api_key: "sk-test".to_string(),
            external_model_name: "deepseek-chat".to_string(),
            enabled: true,
            context_window: Some(65536),
            max_output_tokens: Some(8192),
            reasoning_effort: None,
        };

        let json = serde_json::to_string_pretty(&serde_json::json!({
            "models": [entry.clone()]
        })).unwrap();

        let parsed: CustomModelsFileFormat = serde_json::from_str(&json).unwrap();
        match parsed {
            CustomModelsFileFormat::Object { models } => {
                assert_eq!(models.len(), 1);
                assert_eq!(models[0], entry);
            }
            _ => panic!("Expected Object format"),
        }
    }

    #[test]
    fn test_custom_model_array_format_compatibility() {
        let json = r#"[
            {
                "name": "models/openrouter-claude",
                "displayName": "Claude via OpenRouter",
                "provider": "openrouter",
                "apiFormat": "openai",
                "apiUrl": "https://openrouter.ai/api/v1/chat/completions",
                "apiKey": "sk-or-v1-xyz",
                "externalModelName": "anthropic/claude-3.7-sonnet",
                "enabled": true
            }
        ]"#;

        let parsed: CustomModelsFileFormat = serde_json::from_str(json).unwrap();
        match parsed {
            CustomModelsFileFormat::List(models) => {
                assert_eq!(models.len(), 1);
                assert_eq!(models[0].name, "models/openrouter-claude");
                assert_eq!(models[0].provider, "openrouter");
            }
            _ => panic!("Expected List format"),
        }
    }

    #[test]
    fn test_custom_model_normalization_and_save() {
        let temp_dir = tempfile::tempdir().unwrap();
        let target_path = temp_dir.path().join("custom_models.json");

        let entry = CustomModelEntry {
            name: "deepseek-chat".to_string(), // not starting with models/
            display_name: "".to_string(),      // empty display name
            provider: "deepseek".to_string(),
            api_format: "openai".to_string(),
            api_url: " https://api.deepseek.com/chat/completions ".to_string(), // with spaces
            api_key: " sk-test ".to_string(),
            external_model_name: " deepseek-chat ".to_string(),
            enabled: true,
            context_window: Some(65536),
            max_output_tokens: Some(8192),
            reasoning_effort: None,
        };

        let mut models = vec![entry];
        for model in &mut models {
            let trimmed_name = model.name.trim();
            if trimmed_name.is_empty() {
                let sanitized_external = model.external_model_name.trim().replace('/', "-");
                model.name = format!("models/{}", sanitized_external);
            } else if !trimmed_name.starts_with("models/") {
                model.name = format!("models/{}", trimmed_name);
            } else {
                model.name = trimmed_name.to_string();
            }

            if model.display_name.trim().is_empty() {
                model.display_name = model.external_model_name.trim().to_string();
            } else {
                model.display_name = model.display_name.trim().to_string();
            }

            model.api_url = model.api_url.trim().to_string();
            model.external_model_name = model.external_model_name.trim().to_string();
            model.api_key = model.api_key.trim().to_string();
        }

        assert_eq!(models[0].name, "models/deepseek-chat");
        assert_eq!(models[0].display_name, "deepseek-chat");
        assert_eq!(models[0].api_url, "https://api.deepseek.com/chat/completions");
        assert_eq!(models[0].api_key, "sk-test");

        let payload = serde_json::json!({ "models": models });
        let bytes = serde_json::to_string_pretty(&payload).unwrap();
        crate::utils::fs::write_atomic(&target_path, bytes.as_bytes()).unwrap();

        assert!(target_path.exists());
        let read_back: CustomModelsFileFormat = serde_json::from_str(&std::fs::read_to_string(&target_path).unwrap()).unwrap();
        match read_back {
            CustomModelsFileFormat::Object { models: m } => {
                assert_eq!(m.len(), 1);
                assert_eq!(m[0].name, "models/deepseek-chat");
            }
            _ => panic!("Expected Object format"),
        }
    }
}

