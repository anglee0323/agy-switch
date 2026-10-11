use serde_json::Value;

const LOAD_PROJECT_ENDPOINTS: [&str; 3] = [
    "https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal:loadCodeAssist",
    "https://daily-cloudcode-pa.googleapis.com/v1internal:loadCodeAssist",
    "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist",
];

/// Resolve the Antigravity project ID used by quota requests.
pub async fn fetch_project_id(access_token: &str) -> Result<String, String> {
    let request_body = serde_json::json!({
        "metadata": {
            "ideType": "ANTIGRAVITY"
        }
    });

    let client = crate::utils::http::get_client()?;
    let mut last_error = String::new();

    for url in LOAD_PROJECT_ENDPOINTS {
        let response = client
            .post(url)
            .bearer_auth(access_token)
            .header("User-Agent", crate::constants::USER_AGENT.as_str())
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                let data: Value = resp
                    .json()
                    .await
                    .map_err(|e| format!("解析响应失败: {}", e))?;
                return data
                    .get("cloudaicompanionProject")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| "账号无资格获取官方 cloudaicompanionProject".to_string());
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                last_error = format!("loadCodeAssist [{}] 返回错误 {}: {}", url, status, body);
            }
            Err(error) => {
                last_error = format!("loadCodeAssist [{}] 请求失败: {}", url, error);
            }
        }
    }

    Err(if last_error.is_empty() {
        "loadCodeAssist 所有端点均不可用".to_string()
    } else {
        last_error
    })
}
