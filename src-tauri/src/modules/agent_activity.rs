//! Live task observations. Missing/unsupported observations never mean idle.
use serde_json::Value;
use std::io::Read;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Activity { Idle, Busy, Unknown }

const LIMIT: usize = 4 * 1024 * 1024;
const SERVICE: &str = "/exa.language_server_pb.LanguageServerService/";

fn classify(update: &Value) -> Activity {
    let idle = "CASCADE_RUN_STATUS_IDLE";
    if ["status", "executableStatus", "executorLoopStatus"].iter()
        .any(|key| matches!(update[*key].as_str(), Some("CASCADE_RUN_STATUS_RUNNING" | "CASCADE_RUN_STATUS_BUSY" | "CASCADE_RUN_STATUS_CANCELING")))
        || update["fullyIdle"].as_bool() == Some(false)
        || update["hasActiveChildren"].as_bool() == Some(true)
    { return Activity::Busy; }
    if update["fullyIdle"].as_bool() == Some(true)
        && ["status", "executableStatus", "executorLoopStatus"].iter().all(|key| update[*key].as_str() == Some(idle))
    { Activity::Idle } else { Activity::Unknown }
}

fn read_update(mut response: impl Read) -> Result<Activity, String> {
    for _ in 0..8 {
        let mut header = [0_u8; 5];
        response.read_exact(&mut header).map_err(|_| "task_state_unknown")?;
        let length = u32::from_be_bytes(header[1..5].try_into().unwrap()) as usize;
        if header[0] != 0 || length > LIMIT { return Err("task_state_unknown".into()); }
        let mut bytes = vec![0; length];
        response.read_exact(&mut bytes).map_err(|_| "task_state_unknown")?;
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| "task_state_unknown")?;
        if value.get("update").is_some() { return Ok(classify(&value["update"])); }
    }
    Err("task_state_unknown".into())
}

fn observe(port: u16, csrf: &str) -> Result<Activity, String> {
    let client = reqwest::blocking::Client::builder().no_proxy()
        .redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(3))
        .build().map_err(|_| "task_state_unknown")?;
    let base = format!("http://127.0.0.1:{port}{SERVICE}");
    let response = client.post(format!("{base}GetAllCascadeTrajectories"))
        .header("Content-Type", "application/json").header("x-codeium-csrf-token", csrf)
        .body("{}").send().map_err(|_| "task_state_unknown")?;
    if !response.status().is_success() { return Err("task_state_unknown".into()); }
    let mut bytes = Vec::new();
    response.take(LIMIT as u64 + 1).read_to_end(&mut bytes).map_err(|_| "task_state_unknown")?;
    if bytes.len() > LIMIT { return Err("task_state_unknown".into()); }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| "task_state_unknown")?;
    // An empty protobuf map is omitted. Reject any other unexpected shape.
    let empty = serde_json::Map::new();
    let summaries = match value.get("trajectorySummaries") {
        None if value.as_object().is_some_and(|v| v.is_empty()) => &empty,
        Some(Value::Object(map)) => map,
        _ => return Err("task_state_unknown".into()),
    };
    if summaries.len() > 128 { return Ok(Activity::Unknown); }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    for (id, summary) in summaries {
        if summary["status"].as_str() != Some("CASCADE_RUN_STATUS_IDLE") { return Ok(Activity::Busy); }
        if std::time::Instant::now() > deadline { return Ok(Activity::Unknown); }
        let payload = serde_json::to_vec(&serde_json::json!({
            "conversationId":id, "subscriberId":uuid::Uuid::new_v4().to_string(),
            "initialStepsPageBounds":{"startIndex":0,"endIndexExclusive":0},
            "initialGeneratorMetadatasPageBounds":{"startIndex":0,"endIndexExclusive":0},
            "initialExecutorMetadatasPageBounds":{"startIndex":0,"endIndexExclusive":0},
            "disableRehydration":true
        })).map_err(|_| "task_state_unknown")?;
        let mut frame = vec![0]; frame.extend_from_slice(&(payload.len() as u32).to_be_bytes()); frame.extend(payload);
        let response = client.post(format!("{base}StreamAgentStateUpdates"))
            .header("Content-Type", "application/connect+json").header("connect-protocol-version", "1")
            .header("connect-timeout-ms", "2500").header("x-codeium-csrf-token", csrf)
            .body(frame).send().map_err(|_| "task_state_unknown")?;
        if !response.status().is_success() { return Err("task_state_unknown".into()); }
        match read_update(response)? { Activity::Idle => {}, state => return Ok(state) }
    }
    Ok(Activity::Idle)
}

pub(crate) fn running() -> Activity {
    #[cfg(target_os = "macos")]
    {
        let Ok(config) = super::config::load_app_config() else { return Activity::Unknown; };
        let mut system = sysinfo::System::new();
        system.refresh_processes(sysinfo::ProcessesToUpdate::All);
        // Additional IDE/CLI executors cannot be declared idle using the App's
        // state. Until their adapter is verified, pause instead of closing them.
        let servers = system.processes().values().filter(|p| p.name().to_string_lossy().to_ascii_lowercase().contains("language_server")).count();
        if servers != 1 || system.processes().values().any(|p| matches!(p.name().to_str(), Some("agy" | "agy.exe"))) { return Activity::Unknown; }
        super::app_identity::with_running_connection(config.antigravity_executable.as_deref(), observe)
            .ok().flatten().unwrap_or(Activity::Unknown)
    }
    #[cfg(not(target_os = "macos"))]
    { Activity::Unknown }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_requires_live_executor_and_fully_idle_evidence() {
        let idle = serde_json::json!({"status":"CASCADE_RUN_STATUS_IDLE","executableStatus":"CASCADE_RUN_STATUS_IDLE","executorLoopStatus":"CASCADE_RUN_STATUS_IDLE","fullyIdle":true});
        assert_eq!(classify(&idle), Activity::Idle);
        for field in ["status", "executableStatus", "executorLoopStatus", "fullyIdle"] {
            let mut update = idle.clone(); update.as_object_mut().unwrap().remove(field);
            assert_eq!(classify(&update), Activity::Unknown);
        }
        let mut background = idle.clone(); background["fullyIdle"] = Value::Bool(false);
        assert_eq!(classify(&background), Activity::Busy);
        background["fullyIdle"] = Value::Bool(true); background["executorLoopStatus"] = Value::String("CASCADE_RUN_STATUS_RUNNING".into());
        assert_eq!(classify(&background), Activity::Busy);
        assert_eq!(classify(&serde_json::json!({})), Activity::Unknown);
    }
    #[test]
    fn connect_frames_accept_fragmented_reads_and_reject_invalid_frames() {
        let payload = br#"{"update":{"status":"CASCADE_RUN_STATUS_RUNNING"}}"#;
        let mut frame = vec![0]; frame.extend_from_slice(&(payload.len() as u32).to_be_bytes()); frame.extend_from_slice(payload);
        assert_eq!(read_update(frame.as_slice()).unwrap(), Activity::Busy);
        for bytes in [vec![], vec![2,0,0,0,2,b'{',b'}'], vec![1,0,0,0,0], vec![0,255,255,255,255]] {
            assert!(read_update(bytes.as_slice()).is_err());
        }
    }
}
