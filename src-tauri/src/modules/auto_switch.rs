//! Opt-in low-quota scheduling. This module never stops, kills, or starts a client.
//! A process scan is a conservative observation, not an atomic global idle barrier.
use crate::models::{Account, QuotaData};
use crate::modules::{account, cli_credentials, db, device, integration, version};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

const CONFIG_FILE: &str = "auto_switch.json";
const QUOTA_MAX_AGE: i64 = 180;
const REFRESH_SECONDS: i64 = 60;
const COOLDOWN_SECONDS: i64 = 300;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Wait,
    Stop,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    RoundRobin,
    #[default]
    Priority,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    #[default]
    #[serde(alias = "all")]
    App,
    #[serde(alias = "desktop")]
    AppCli,
    Ide,
    Vscode,
}
impl Target {
    pub fn argument(self) -> Option<&'static str> {
        match self {
            Self::App => None,
            Self::AppCli => Some("app"),
            Self::Ide => Some("ide"),
            Self::Vscode => Some("vscode"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub enabled: bool,
    pub mode: Mode,
    pub strategy: Strategy,
    pub reserve_percentage: u8,
    pub candidate_min_percentage: u8,
    pub monitored_model: String,
    pub candidate_account_ids: Vec<String>,
    pub target: Target,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: Mode::Wait,
            strategy: Strategy::Priority,
            reserve_percentage: 10,
            candidate_min_percentage: 30,
            monitored_model: "all".into(),
            candidate_account_ids: vec![],
            target: Target::App,
        }
    }
}
impl Config {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if !(1..=98).contains(&self.reserve_percentage)
            || self.candidate_min_percentage <= self.reserve_percentage
            || self.candidate_min_percentage > 100
        {
            return Err("Choose a reserve from 1–98% and a candidate minimum above the reserve, up to 100%.".into());
        }
        if self.candidate_account_ids.len() > 100 || self.monitored_model.len() > 200 {
            return Err("Select no more than 100 candidate accounts.".into());
        }
        let mut ids = HashSet::new();
        if self
            .candidate_account_ids
            .iter()
            .any(|id| id.is_empty() || !ids.insert(id))
        {
            return Err("Candidate accounts must be unique.".into());
        }
        if self.enabled && self.candidate_account_ids.is_empty() {
            return Err(
                "Choose at least one allowed backup account before enabling.".into(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessState {
    Closed,
    Running,
    #[default]
    Unknown,
}
#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub phase: String,
    pub reason: Option<String>,
    pub source_account_id: Option<String>,
    pub source_email: Option<String>,
    pub target_account_id: Option<String>,
    pub target_email: Option<String>,
    pub remaining_percentage: Option<f64>,
    pub pending_id: Option<String>,
    pub mode: Mode,
    pub process_state: ProcessState,
    pub last_checked: Option<i64>,
}
impl Default for Status {
    fn default() -> Self {
        Self {
            phase: "disabled".into(),
            reason: None,
            source_account_id: None,
            source_email: None,
            target_account_id: None,
            target_email: None,
            remaining_percentage: None,
            pending_id: None,
            mode: Mode::Wait,
            process_state: ProcessState::Unknown,
            last_checked: None,
        }
    }
}
impl Status {
    fn set(&mut self, phase: &str, reason: &str) {
        self.phase = phase.into();
        self.reason = Some(reason.into());
    }
}
#[derive(Clone, Debug)]
struct Pending {
    id: String,
    source_id: String,
    target_id: String,
    revision: u64,
    closed_since: Option<i64>,
}
#[derive(Default)]
struct RuntimeData {
    config: Config,
    status: Status,
    revision: u64,
    pending: Option<Pending>,
    canceled_source: Option<String>,
    cooldown_until: i64,
    refresh_after: HashMap<String, i64>,
    refresh_failed: HashSet<String>,
    failed: bool,
    commit_started: bool,
}
#[derive(Default)]
pub struct Runtime {
    data: Arc<Mutex<RuntimeData>>,
    tick: tokio::sync::Mutex<()>,
}

#[derive(Default, Serialize, Deserialize)]
struct PauseRecord {
    source_id: Option<String>,
    failed: bool,
}
fn write_pause(source_id: Option<String>, failed: bool) -> Result<(), String> {
    let bytes = serde_json::to_vec(&PauseRecord { source_id, failed })
        .map_err(|_| "Cannot encode switch state.")?;
    crate::utils::fs::write_atomic(
        &account::get_data_dir()?.join("auto_switch_state.json"),
        &bytes,
    )
    .map_err(|_| "Cannot save switch state.".into())
}
fn read_pause() -> Result<PauseRecord, String> {
    read_pause_at(&account::get_data_dir()?)
}
fn read_pause_at(root: &std::path::Path) -> Result<PauseRecord, String> {
    match std::fs::read(root.join("auto_switch_state.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| "Cannot read switch state.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(PauseRecord::default()),
        Err(_) => Err("Cannot read switch state.".into()),
    }
}

#[cfg(test)]
fn config_path() -> Result<std::path::PathBuf, String> {
    Ok(account::get_data_dir()?.join(CONFIG_FILE))
}
fn read_config() -> Result<Config, String> {
    read_config_at(&account::get_data_dir()?)
}
/// CLI reads must not create the data directory or initialize the desktop runtime.
pub(crate) fn read_config_at(root: &std::path::Path) -> Result<Config, String> {
    match std::fs::read(root.join(CONFIG_FILE)) {
        Ok(bytes) => {
            let c: Config = serde_json::from_slice(&bytes)
                .map_err(|_| "Cannot read auto-switch settings.".to_string())?;
            c.validate()?;
            Ok(c)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(_) => Err("Cannot read auto-switch settings.".into()),
    }
}

/// Compare and replace under the same OS lock as credential commits. A stale editor
/// must reload instead of silently overwriting settings saved by another client.
pub(crate) fn save_config_at(root: &std::path::Path, expected: &Config, config: &Config) -> Result<(), String> {
    config.validate()?;
    let _switch = crate::cli::SwitchLock::acquire(root)?;
    if read_config_at(root)? != *expected {
        return Err("auto_switch_settings_changed".into());
    }
    let bytes = serde_json::to_vec_pretty(config).map_err(|_| "Cannot encode settings.")?;
    crate::utils::fs::write_atomic(root.join(CONFIG_FILE), &bytes)
        .map_err(|_| "Cannot save auto-switch settings.")?;
    crate::utils::fs::write_atomic(root.join("auto_switch_state.json"),
        &serde_json::to_vec(&PauseRecord::default()).map_err(|_| "Cannot encode switch state.")?)
        .map_err(|_| "Cannot save switch state.".into())
}

fn apply_config(d: &mut RuntimeData, config: Config) {
    d.config = config.clone();
    d.revision += 1;
    d.pending = None;
    d.canceled_source = None;
    d.refresh_after.clear();
    d.refresh_failed.clear();
    d.failed = false;
    d.status = Status {
        mode: config.mode,
        phase: if config.enabled { "monitoring" } else { "disabled" }.into(),
        ..Status::default()
    };
}

fn sync_external_config(runtime: &Runtime) -> Result<(), String> {
    sync_external_config_at(runtime, &account::get_data_dir()?)
}
fn sync_external_config_at(runtime: &Runtime, root: &std::path::Path) -> Result<(), String> {
    let mut d = runtime.data.lock().map_err(|_| "Auto-switch state is unavailable.")?;
    if d.commit_started { return Ok(()); }
    let config = read_config_at(root)?;
    if config != d.config {
        let pause = read_pause_at(root)?;
        apply_config(&mut d, config);
        d.canceled_source = pause.source_id;
        d.failed = pause.failed;
        if d.failed { d.status.set("blocked", "switch_failed"); }
    }
    Ok(())
}

/// Known bucket IDs, rather than translated display labels, identify provider pools.
/// Missing/ambiguous windows are unknown. The UI's single percentage is insufficient.
fn remaining(q: &QuotaData, model: &str, now: i64) -> Result<f64, &'static str> {
    if q.is_forbidden {
        return Err("account_unavailable");
    }
    if q.last_updated > now + 30 || now - q.last_updated > QUOTA_MAX_AGE {
        return Err("stale_quota");
    }
    let trimmed = model.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("all") {
        let mut min_val: Option<f64> = None;
        for m in &q.models {
            if let Ok(val) = remaining_single_model(q, &m.name, now) {
                min_val = Some(min_val.map_or(val, |old| old.min(val)));
            }
        }
        return min_val.ok_or("unknown_pool");
    }
    if trimmed.eq_ignore_ascii_case("gemini") {
        let mut min_val: Option<f64> = None;
        for m in &q.models {
            if m.name.to_ascii_lowercase().starts_with("gemini") {
                if let Ok(val) = remaining_single_model(q, &m.name, now) {
                    min_val = Some(min_val.map_or(val, |old| old.min(val)));
                }
            }
        }
        return min_val.ok_or("unknown_pool");
    }
    if trimmed.eq_ignore_ascii_case("claude")
        || trimmed.eq_ignore_ascii_case("3p")
        || trimmed.eq_ignore_ascii_case("non-gemini")
    {
        let mut min_val: Option<f64> = None;
        for m in &q.models {
            let lower = m.name.to_ascii_lowercase();
            if lower.starts_with("claude") || lower.starts_with("gpt") {
                if let Ok(val) = remaining_single_model(q, &m.name, now) {
                    min_val = Some(min_val.map_or(val, |old| old.min(val)));
                }
            }
        }
        return min_val.ok_or("unknown_pool");
    }
    remaining_single_model(q, trimmed, now)
}

fn remaining_single_model(q: &QuotaData, model: &str, now: i64) -> Result<f64, &'static str> {
    let mut resolved = model;
    for _ in 0..8 {
        if let Some(next) = q.model_forwarding_rules.get(resolved) {
            resolved = next;
        } else {
            break;
        }
    }
    let m = q
        .models
        .iter()
        .find(|m| m.name == resolved)
        .ok_or("unknown_pool")?;
    if !(0..=100).contains(&m.percentage) {
        return Err("unknown_pool");
    }
    let lower = resolved.to_ascii_lowercase();
    let pool = if lower.starts_with("gemini") {
        "gemini"
    } else if lower.starts_with("claude") || lower.starts_with("gpt") {
        "3p"
    } else {
        return Err("unknown_pool");
    };
    let groups = q.quota_groups.as_ref().ok_or("unknown_pool")?;
    let mut weekly = None::<f64>;
    let mut short = None::<f64>;
    for bucket in groups.iter().flat_map(|g| &g.buckets) {
        let id = bucket.bucket_id.to_ascii_lowercase();
        if !id.starts_with(&format!("{pool}-")) {
            continue;
        }
        if !bucket.remaining_fraction.is_finite()
            || !(0.0..=1.0).contains(&bucket.remaining_fraction)
        {
            return Err("unknown_pool");
        }
        let reset = chrono::DateTime::parse_from_rfc3339(&bucket.reset_time)
            .map_err(|_| "unknown_pool")?
            .timestamp();
        // A reset deadline is not evidence that quota has actually recovered.
        if reset <= now {
            return Err("stale_quota");
        }
        let window = bucket.window.to_ascii_lowercase();
        let value = bucket.remaining_fraction * 100.0;
        if id.ends_with("weekly") || window == "weekly" {
            weekly = Some(weekly.map_or(value, |old| old.min(value)));
        } else if id.ends_with("5h") || window == "5h" {
            short = Some(short.map_or(value, |old| old.min(value)));
        } else {
            return Err("unknown_pool");
        }
    }
    let weekly = weekly.ok_or("unknown_pool")?;
    let free = q
        .subscription_tier
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase()
        .contains("free");

    // 仅以 5 小时短周期配额为限制，不限制周线；仅在无 5h 桶的 Free 账号下回退使用周配额
    if let Some(s) = short {
        Ok(s.min(m.percentage as f64))
    } else if free {
        Ok(weekly.min(m.percentage as f64))
    } else {
        Err("unknown_pool")
    }
}
fn usable(a: &Account) -> bool {
    !a.disabled && !a.validation_blocked && !a.quota.as_ref().is_some_and(|q| q.is_forbidden)
}
fn account_remaining(a: &Account, model: &str, now: i64) -> Result<f64, &'static str> {
    if !usable(a) {
        return Err("account_unavailable");
    }
    remaining(a.quota.as_ref().ok_or("no_quota")?, model, now)
}

/// Scan all known native APP/IDE/agy processes even for CLI-only changes. An open
/// client may cache credentials or later write them back. A running client is NOT
/// evidence of a running task; it simply prevents this closed-client-only mode.
fn clients() -> ProcessState {
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All);
    if system.processes().is_empty() {
        return ProcessState::Unknown;
    }
    let own = std::process::id();
    let mut paths = vec![];
    if let Ok(config) = crate::modules::config::load_app_config() {
        for p in [
            config.antigravity_executable,
            config.antigravity_ide_executable,
        ]
        .into_iter()
        .flatten()
        {
            match std::fs::canonicalize(p) {
                Ok(p) => paths.push(p),
                Err(_) => return ProcessState::Unknown,
            }
        }
    } else {
        return ProcessState::Unknown;
    }
    for (pid, proc) in system.processes() {
        if pid.as_u32() == own {
            continue;
        }
        if is_client_process(&proc.name().to_string_lossy(), proc.exe(), &paths) {
            return ProcessState::Running;
        }
    }
    ProcessState::Closed
}

fn is_client_process(
    name: &str,
    executable: Option<&std::path::Path>,
    configured_paths: &[std::path::PathBuf],
) -> bool {
    let name = name.to_ascii_lowercase();
    if name.starts_with("antigravity-tools") || name.starts_with("antigravity_tools") {
        return false;
    }
    if name.contains("antigravity") || name == "agy" || name == "agy.exe" {
        return true;
    }
    executable.is_some_and(|path| {
        let normalized = path
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        // Some background executors have generic process names but still live in
        // the client's installation/data tree after its visible window closes.
        normalized.contains("/antigravity.app/")
            || normalized.contains("/antigravity ide.app/")
            || normalized.contains("/antigravity-ide/")
            || normalized.contains("/antigravity/")
            || normalized.contains("/antigravity-cli/")
            || configured_paths
                .iter()
                .any(|configured| configured_client_path(path, configured))
    })
}

fn configured_client_path(path: &std::path::Path, configured: &std::path::Path) -> bool {
    if path == configured || path.starts_with(configured) {
        return true;
    }
    let normalized = configured.to_string_lossy().replace('\\', "/");
    if let Some(end) = normalized.to_ascii_lowercase().find(".app/") {
        return path
            .to_string_lossy()
            .replace('\\', "/")
            .starts_with(&normalized[..end + 5]);
    }
    let Some(root) = configured.parent() else {
        return false;
    };
    if matches!(
        root.to_str(),
        Some("/" | "/bin" | "/usr/bin" | "/usr/local/bin")
    ) {
        return false;
    }
    // Only the configured executable's resource/framework children, not arbitrary
    // sibling executables, belong to this installation.
    path.starts_with(root.join("resources"))
        || path.starts_with(root.join("Resources"))
        || path.starts_with(root.join("Frameworks"))
}

/// Compare credentials privately. No token or raw authentication error is exposed
/// through the status DTO. Any external sign-in mismatch invalidates the request.
fn verify_source(source: &Account, target: Target) -> Result<(), &'static str> {
    if target == Target::App || target == Target::AppCli {
        let config =
            crate::modules::config::load_app_config().map_err(|_| "credentials_changed")?;
        if crate::modules::app_identity::running_email(config.antigravity_executable.as_deref())
            .map_err(|_| "credentials_changed")?
            .is_some_and(|email| !email.eq_ignore_ascii_case(&source.email))
        {
            return Err("credentials_changed");
        }
    }
    let actual_refresh_token = match target {
        Target::App | Target::AppCli => {
            let v = installed_app_version()?;
            if version::compare_version(&v.short_version, "2.0.0") == std::cmp::Ordering::Less {
                return Err("unsupported_client");
            }
            integration::read_from_system_keyring()
                .map(|s| s.refresh_token)
                .map_err(|_| "credentials_changed")?
        }
        Target::Ide => {
            crate::modules::migration::get_refresh_token_from_db(Some("ide"))
                .map_err(|_| "credentials_changed")?
        }
        Target::Vscode => {
            if let Some(home) = dirs::home_dir() {
                let token_file = home.join(".gemini/jetski-standalone-oauth-token");
                if token_file.is_file() {
                    if let Ok(bytes) = std::fs::read(&token_file) {
                        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                            if let Some(rt) = json.get("refresh_token").and_then(|v| v.as_str()) {
                                if !rt.is_empty() && rt != source.token.refresh_token {
                                    return Err("credentials_changed");
                                }
                            }
                        }
                    }
                }
            }
            source.token.refresh_token.clone()
        }
    };
    if actual_refresh_token.is_empty() || actual_refresh_token != source.token.refresh_token {
        return Err("credentials_changed");
    }
    if target == Target::App || target == Target::AppCli {
        let home = dirs::home_dir().ok_or("credentials_changed")?;
        if let Some(path) =
            cli_credentials::session_path(&home).map_err(|_| "credentials_changed")?
        {
            if !native_session_exists(&path)? {
                return Ok(());
            }
            let cli = integration::read_cli_credentials().map_err(|_| "credentials_changed")?;
            if cli.refresh_token != source.token.refresh_token {
                return Err("credentials_changed");
            }
        }
    }
    Ok(())
}

/// Never execute the APP just to discover its version on Linux. Some bundles
/// interpret --version as a normal launch, violating closed-client-only behavior.
fn installed_app_version() -> Result<version::AntigravityVersion, &'static str> {
    #[cfg(target_os = "linux")]
    {
        let path = crate::modules::process::get_antigravity_executable_path(None)
            .ok_or("unsupported_client")?;
        let path = std::fs::canonicalize(path).map_err(|_| "unsupported_client")?;
        let package = path
            .parent()
            .ok_or("unsupported_client")?
            .join("resources/app/package.json");
        let content = std::fs::read(package).map_err(|_| "unsupported_client")?;
        let json: serde_json::Value =
            serde_json::from_slice(&content).map_err(|_| "unsupported_client")?;
        let v = json
            .get("version")
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
            .ok_or("unsupported_client")?;
        Ok(version::AntigravityVersion {
            short_version: v.into(),
            bundle_version: v.into(),
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        version::get_antigravity_version(None).map_err(|_| "unsupported_client")
    }
}

fn native_session_exists(path: &std::path::Path) -> Result<bool, &'static str> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(true),
        Ok(_) => Err("credentials_changed"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err("credentials_changed"),
    }
}

fn advance_pending(
    d: &mut RuntimeData,
    source_id: &str,
    target_id: &str,
    now: i64,
    process_state: ProcessState,
) -> Pending {
    if !d
        .pending
        .as_ref()
        .is_some_and(|p| p.source_id == source_id && p.target_id == target_id)
    {
        d.pending = Some(Pending {
            id: uuid::Uuid::new_v4().to_string(),
            source_id: source_id.into(),
            target_id: target_id.into(),
            revision: d.revision,
            closed_since: None,
        });
    }
    let p = d.pending.as_mut().unwrap();
    match process_state {
        ProcessState::Closed => {
            if p.closed_since.is_none() {
                p.closed_since = Some(now);
            }
        }
        _ => p.closed_since = None,
    }
    p.clone()
}

#[cfg_attr(not(test), allow(dead_code))]
fn commit_guard(
    d: &RuntimeData,
    pending: &Pending,
    config: &Config,
    current: Option<&str>,
    source: &Account,
    target: &Account,
    now: i64,
    process: ProcessState,
) -> Result<(), &'static str> {
    if d.revision != pending.revision
        || &d.config != config
        || !config.enabled
        || !d.pending.as_ref().is_some_and(|p| p.id == pending.id)
    {
        return Err("request_changed");
    }
    if current != Some(pending.source_id.as_str()) || source.id != pending.source_id {
        return Err("source_changed");
    }
    if target.id != pending.target_id || !config.candidate_account_ids.contains(&target.id) {
        return Err("no_candidate");
    }
    if !account_remaining(source, &config.monitored_model, now)
        .is_ok_and(|v| v <= config.reserve_percentage as f64)
    {
        return Err("no_quota");
    }
    if !account_remaining(target, &config.monitored_model, now)
        .is_ok_and(|v| v >= config.candidate_min_percentage as f64)
    {
        return Err("no_candidate");
    }
    if config.target == Target::Vscode {
        return Ok(());
    }
    match process {
        ProcessState::Running => Err("clients_running"),
        ProcessState::Unknown => Err("process_unknown"),
        ProcessState::Closed => Ok(()),
    }
}

pub fn is_any_agent_actively_working() -> bool {
    #[cfg(test)]
    {
        return false;
    }
    #[cfg(not(test))]
    {
        let Some(home) = dirs::home_dir() else { return false; };
        let candidates = [
            home.join(".gemini/antigravity/brain"),
            home.join(".gemini/antigravity-cli/brain"),
        ];
        let now = std::time::SystemTime::now();

        for brain_dir in &candidates {
            if !brain_dir.is_dir() {
                continue;
            }
            let Ok(entries) = std::fs::read_dir(brain_dir) else { continue; };
            for entry in entries.flatten() {
                let p = entry.path().join(".system_generated/logs/transcript.jsonl");
                if !p.is_file() {
                    continue;
                }
                let Ok(meta) = p.metadata() else { continue; };
                let Ok(mtime) = meta.modified() else { continue; };
                let Ok(elapsed) = now.duration_since(mtime) else { continue; };

                if elapsed.as_secs() < 8 {
                    if let Ok(file) = std::fs::File::open(&p) {
                        use std::io::{BufRead, BufReader, Seek, SeekFrom};
                        let mut reader = BufReader::new(file);
                        if let Ok(len) = reader.seek(SeekFrom::End(0)) {
                            let offset = if len > 4096 { len - 4096 } else { 0 };
                            let _ = reader.seek(SeekFrom::Start(offset));
                            let lines: Vec<String> = reader.lines().flatten().collect();
                            if let Some(last_line) = lines.iter().rev().find(|l| !l.trim().is_empty()) {
                                if !last_line.contains("\"status\":\"DONE\"") {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }
        false
    }
}

pub fn interrupt_vscode_agent() {
    #[cfg(all(target_os = "macos", not(test)))]
    {
        let script = r#"
            tell application "System Events"
                if exists (processes whose name is "Code") then
                    tell process "Code"
                        key code 53
                    end tell
                end if
            end tell
        "#;
        let _ = std::process::Command::new("osascript").args(["-e", script]).output();
    }
}

/// Only the external environment is injectable. Production and isolated tests
/// run the same coordinator, account switch locks, journal and index updates.
trait Environment: Clone + Send + Sync + 'static {
    fn now(&self) -> i64;
    fn clients(&self) -> ProcessState;
    fn is_agent_working(&self) -> bool {
        false
    }
    fn interrupt_vscode(&self) {}
    fn is_client_running(&self, _target_ide: Option<&str>) -> bool {
        false
    }
    fn close_client(&self, _timeout: u64, _target_ide: Option<&str>) -> Result<(), String> {
        Ok(())
    }
    fn start_client(&self, _target_ide: Option<&str>) -> Result<(), String> {
        Ok(())
    }
    fn fetch_quota<'a>(
        &'a self,
        account: &'a mut Account,
    ) -> impl std::future::Future<Output = Result<QuotaData, String>> + Send + 'a;
    fn save_quota(&self, id: &str, quota: QuotaData) -> Result<(), String> {
        account::update_account_quota(id, quota)
    }
    fn verify_source(&self, source: &Account, target: Target) -> Result<(), &'static str>;
    fn write_credentials(&self, target: &Account, target_ide: Option<&str>) -> Result<(), String>;
}
#[derive(Clone)]
struct NativeEnvironment;
impl Environment for NativeEnvironment {
    fn now(&self) -> i64 {
        chrono::Utc::now().timestamp()
    }
    fn clients(&self) -> ProcessState {
        clients()
    }
    fn is_agent_working(&self) -> bool {
        is_any_agent_actively_working()
    }
    fn interrupt_vscode(&self) {
        interrupt_vscode_agent();
    }
    fn is_client_running(&self, target_ide: Option<&str>) -> bool {
        crate::modules::process::is_antigravity_running(target_ide)
    }
    fn close_client(&self, timeout: u64, target_ide: Option<&str>) -> Result<(), String> {
        crate::modules::process::close_antigravity(timeout, target_ide)
    }
    fn start_client(&self, target_ide: Option<&str>) -> Result<(), String> {
        crate::modules::process::start_antigravity(target_ide)
    }
    async fn fetch_quota(&self, account: &mut Account) -> Result<QuotaData, String> {
        account::fetch_quota_with_retry(account)
            .await
            .map_err(|_| "no_quota".into())
    }
    fn verify_source(&self, source: &Account, target: Target) -> Result<(), &'static str> {
        verify_source(source, target)
    }
    fn write_credentials(&self, target: &Account, target_ide: Option<&str>) -> Result<(), String> {
        let is_ide_mode = target_ide == Some("ide");
        let is_app_cli_mode = target_ide == Some("app");
        let is_vscode_mode = target_ide == Some("vscode");
        if !is_ide_mode && !is_vscode_mode {
            integration::write_to_system_keyring(target, false)?;
        }

        if !is_app_cli_mode {
            if let Some(home) = dirs::home_dir() {
                let gemini_dir = home.join(".gemini");
                if gemini_dir.is_dir() {
                    let jetski_file = gemini_dir.join("jetski-standalone-oauth-token");
                    if let Ok(payload) = cli_credentials::payload(&target.token) {
                        let _ = cli_credentials::write_session(&jetski_file, &payload);
                    }
                }
            }
        }

        if let Ok(storage_path) = device::get_storage_path(target_ide) {
            if let Some(ref profile) = target.device_profile {
                let _ = device::write_profile(&storage_path, profile);
            }
        }

        let candidate_dbs = if is_app_cli_mode {
            let mut paths = Vec::new();
            #[cfg(target_os = "macos")]
            if let Some(home) = dirs::home_dir() {
                paths.push(home.join("Library/Application Support/Antigravity/User/globalStorage/state.vscdb"));
            }
            #[cfg(target_os = "windows")]
            if let Ok(appdata) = std::env::var("APPDATA") {
                paths.push(std::path::PathBuf::from(appdata).join("Antigravity\\User\\globalStorage\\state.vscdb"));
            }
            #[cfg(target_os = "linux")]
            if let Some(config_home) = crate::modules::linux_paths::config_home() {
                paths.push(config_home.join("Antigravity/User/globalStorage/state.vscdb"));
            }
            paths
        } else {
            db::get_all_candidate_db_paths(target_ide)
        };

        for db_path in candidate_dbs {
            if db_path.exists() {
                let backup_path = db_path.with_extension("vscdb.backup");
                let _ = std::fs::copy(&db_path, &backup_path);
                let _ = db::inject_token(
                    &db_path,
                    &target.token.access_token,
                    &target.token.refresh_token,
                    target.token.expiry_timestamp,
                    &target.email,
                    target.token.is_gcp_tos,
                    target.token.project_id.as_deref(),
                    target.token.id_token.as_deref(),
                    target.token.oauth_client_key.as_deref(),
                );
            }
        }

        Ok(())
    }
}

pub fn start(app: tauri::AppHandle) {
    let runtime = app.state::<Runtime>();
    if let Ok(mut d) = runtime.data.lock() {
        match (read_config(), read_pause()) {
            (Ok(c), Ok(pause)) => {
                d.config = c;
                d.canceled_source = pause.source_id;
                d.failed = pause.failed;
                if d.failed {
                    d.status.set("blocked", "switch_failed");
                }
            }
            _ => {
                d.failed = true;
                d.status.set("blocked", "configuration_required");
            }
        }
    }
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if evaluate(&app, false).await.is_err() {
                if let Ok(mut d) = app.state::<Runtime>().data.lock() {
                    if d.config.enabled && !d.commit_started {
                        d.status.set("blocked", "state_unavailable");
                    }
                }
            }
        }
    });
}
#[tauri::command]
pub fn get_auto_switch_config(app: tauri::AppHandle) -> Result<Config, String> {
    sync_external_config(&app.state::<Runtime>())?;
    Ok(app
        .state::<Runtime>()
        .data
        .lock()
        .map_err(|_| "Auto-switch state is unavailable.")?
        .config
        .clone())
}
#[tauri::command]
pub fn set_auto_switch_config(app: tauri::AppHandle, mut config: Config) -> Result<Config, String> {
    config.monitored_model = config.monitored_model.trim().to_owned();
    config.validate()?;
    let runtime = app.state::<Runtime>();
    let mut d = runtime
        .data
        .lock()
        .map_err(|_| "Auto-switch state is unavailable.")?;
    if d.commit_started {
        return Err(
            "Account credentials are being updated. Wait for the result before changing settings."
                .into(),
        );
    }
    if config.enabled {
        let accounts = account::list_accounts()?;
        if config
            .candidate_account_ids
            .iter()
            .any(|id| !accounts.iter().any(|a| &a.id == id))
        {
            return Err("A selected account no longer exists. Refresh the account list.".into());
        }
    }
    save_config_at(&account::get_data_dir()?, &d.config, &config)?;
    apply_config(&mut d, config.clone());
    Ok(config)
}
#[tauri::command]
pub fn get_auto_switch_status(app: tauri::AppHandle) -> Result<Status, String> {
    Ok(app
        .state::<Runtime>()
        .data
        .lock()
        .map_err(|_| "Auto-switch state is unavailable.")?
        .status
        .clone())
}
#[tauri::command]
pub fn cancel_auto_switch(app: tauri::AppHandle, pending_id: String) -> Result<Status, String> {
    let runtime = app.state::<Runtime>();
    cancel_pending(&runtime, &pending_id)
}
fn cancel_pending(runtime: &Runtime, pending_id: &str) -> Result<Status, String> {
    let mut d = runtime
        .data
        .lock()
        .map_err(|_| "Auto-switch state is unavailable.")?;
    if d.commit_started {
        return Err("Account credentials are being updated. Wait for the result.".into());
    }
    let pending = d
        .pending
        .as_ref()
        .filter(|p| p.id == pending_id)
        .ok_or("This switch request is no longer pending.")?
        .clone();
    d.canceled_source = Some(pending.source_id.clone());
    d.revision += 1;
    d.pending = None;
    d.status.pending_id = None;
    d.status.set("canceled", "canceled_until_recovery");
    // Cancellation takes effect immediately even if its durable record fails.
    write_pause(Some(pending.source_id), false)?;
    Ok(d.status.clone())
}
#[tauri::command]
pub async fn check_auto_switch_now(app: tauri::AppHandle) -> Result<Status, String> {
    evaluate(&app, true).await?;
    get_auto_switch_status(app)
}

async fn refresh<E: Environment>(
    runtime: &Runtime,
    environment: &E,
    id: &str,
    force: bool,
) -> Result<Account, &'static str> {
    let now = environment.now();
    let should_refresh = {
        let mut d = runtime.data.lock().map_err(|_| "no_quota")?;
        let after = d.refresh_after.get(id).copied().unwrap_or(0);
        // Even manual checks have a 10-second floor; a failed API is not hammered.
        if now < after && (!force || after - now > REFRESH_SECONDS - 10) {
            false
        } else {
            d.refresh_after.insert(id.into(), now + REFRESH_SECONDS);
            true
        }
    };
    let mut a = account::load_account(id).map_err(|_| "account_unavailable")?;
    if should_refresh {
        // Leave a failure marker through fetch, persistence AND reread. Otherwise
        // a throttled next tick could reuse an obsolete high-quota snapshot.
        runtime
            .data
            .lock()
            .map_err(|_| "no_quota")?
            .refresh_failed
            .insert(id.into());
        let quota = environment
            .fetch_quota(&mut a)
            .await
            .map_err(|_| "no_quota")?;
        environment.save_quota(id, quota).map_err(|_| "no_quota")?;
        a = account::load_account(id).map_err(|_| "account_unavailable")?;
        runtime
            .data
            .lock()
            .map_err(|_| "no_quota")?
            .refresh_failed
            .remove(id);
    }
    if runtime
        .data
        .lock()
        .map_err(|_| "no_quota")?
        .refresh_failed
        .contains(id)
    {
        return Err("no_quota");
    }
    Ok(a)
}
fn update_status(runtime: &Runtime, revision: u64, status: Status) {
    if let Ok(mut d) = runtime.data.lock() {
        if d.revision == revision {
            if status.phase == "blocked" {
                d.pending = None;
            }
            d.status = status;
        }
    }
}

async fn evaluate(app: &tauri::AppHandle, force: bool) -> Result<(), String> {
    sync_external_config(&app.state::<Runtime>())?;
    if evaluate_core(&app.state::<Runtime>(), NativeEnvironment, force).await? {
        let _ = app.emit("tray://account-switched", ());
        crate::modules::tray::update_tray_menus(app);
    }
    Ok(())
}

async fn evaluate_core<E: Environment>(
    runtime: &Runtime,
    environment: E,
    force: bool,
) -> Result<bool, String> {
    let Ok(_tick) = runtime.tick.try_lock() else {
        return Ok(false);
    };
    let (config, revision, previous, cooldown) = {
        let d = runtime.data.lock().map_err(|_| "State unavailable")?;
        (
            d.config.clone(),
            d.revision,
            d.status.clone(),
            d.cooldown_until,
        )
    };
    if !config.enabled || runtime.data.lock().map_err(|_| "State unavailable")?.failed {
        return Ok(false);
    }
    let now = environment.now();
    let mut status = Status {
        phase: "monitoring".into(),
        mode: config.mode,
        last_checked: Some(now),
        ..Status::default()
    };
    if now < cooldown {
        status = previous;
        status.last_checked = Some(now);
        update_status(runtime, revision, status);
        return Ok(false);
    }
    let source_id = match account::get_current_account_id()? {
        Some(id) => id,
        None => {
            status.set("blocked", "no_current_account");
            update_status(runtime, revision, status);
            return Ok(false);
        }
    };
    status.source_account_id = Some(source_id.clone());
    let source = match refresh(runtime, &environment, &source_id, force).await {
        Ok(a) => a,
        Err(reason) => {
            status.set("blocked", reason);
            update_status(runtime, revision, status);
            return Ok(false);
        }
    };
    status.source_email = Some(source.email.clone());
    let low = match account_remaining(&source, &config.monitored_model, now) {
        Ok(p) => p,
        Err(reason) => {
            status.set("blocked", reason);
            update_status(runtime, revision, status);
            return Ok(false);
        }
    };
    status.remaining_percentage = Some(low);
    {
        let mut d = runtime.data.lock().map_err(|_| "State unavailable")?;
        if d.revision != revision {
            return Ok(false);
        }
        if d.pending.as_ref().is_some_and(|p| p.source_id != source_id) {
            d.pending = None;
        }
        if low > config.reserve_percentage as f64 {
            d.pending = None;
            if low >= config.candidate_min_percentage as f64
                && d.canceled_source.as_deref() == Some(source_id.as_str())
            {
                write_pause(None, false)?;
                d.canceled_source = None;
            }
            d.status = status;
            return Ok(false);
        }
        if d.canceled_source.as_deref() == Some(&source_id) {
            status.set("canceled", "canceled_until_recovery");
            d.status = status;
            return Ok(false);
        }
    }
    let mut candidate = None;
    let candidate_ids = &config.candidate_account_ids;
    let len = candidate_ids.len();
    if len > 0 {
        let start_idx = match config.strategy {
            Strategy::RoundRobin => candidate_ids
                .iter()
                .position(|id| id == &source_id)
                .map(|idx| (idx + 1) % len)
                .unwrap_or(0),
            Strategy::Priority => 0,
        };

        for step in 0..len {
            let idx = (start_idx + step) % len;
            let id = &candidate_ids[idx];
            if id == &source_id {
                continue;
            }
            if let Ok(a) = refresh(runtime, &environment, id, force).await {
                if account_remaining(&a, &config.monitored_model, now)
                    .is_ok_and(|p| p >= config.candidate_min_percentage as f64)
                {
                    candidate = Some(a);
                    break;
                }
            }
            if runtime
                .data
                .lock()
                .map_err(|_| "State unavailable")?
                .revision
                != revision
            {
                return Ok(false);
            }
        }
    }
    let Some(candidate) = candidate else {
        status.set("blocked", "no_candidate");
        update_status(runtime, revision, status);
        return Ok(false);
    };
    status.target_account_id = Some(candidate.id.clone());
    status.target_email = Some(candidate.email.clone());
    let process_environment = environment.clone();
    let process_state = tokio::task::spawn_blocking(move || process_environment.clients())
        .await
        .unwrap_or(ProcessState::Unknown);
    status.process_state = process_state;
    let pending = {
        let mut d = runtime.data.lock().map_err(|_| "State unavailable")?;
        if d.revision != revision {
            return Ok(false);
        }
        let result = advance_pending(&mut d, &source_id, &candidate.id, now, process_state);
        status.pending_id = Some(result.id.clone());
        result
    };

    if process_state == ProcessState::Unknown {
        status.set("blocked", "process_unknown");
        update_status(runtime, revision, status);
        return Ok(false);
    }

    // In Mode::Wait, if agent is actively generating, wait for it to finish
    if config.mode == Mode::Wait && environment.is_agent_working() {
        let mut d = runtime.data.lock().map_err(|_| "State unavailable")?;
        if d.revision == revision {
            status.set("pending", "waiting_task_finish");
            d.status = status;
        }
        return Ok(false);
    }

    // In VS Code mode: no client exit needed, hot-swap in place
    let is_vscode = config.target == Target::Vscode;
    if !is_vscode {
        if process_state != ProcessState::Closed {
            status.set("pending", "clients_running");
            update_status(runtime, revision, status.clone());
            // In automated mode, trigger close on running clients when safe:
            if config.mode == Mode::Stop || (config.mode == Mode::Wait && !environment.is_agent_working()) {
                let close_environment = environment.clone();
                let close_data = runtime.data.clone();
                let close_source = source.clone();
                let close_config = config.clone();
                let checked_close = tokio::task::spawn_blocking(move || {
                    let root = account::get_data_dir().map_err(|_| "source_changed")?;
                    let _switch = crate::cli::SwitchLock::acquire(&root)
                        .map_err(|_| "another_account_switch_in_progress")?;
                    // A stale index must never cause the real client to exit.
                    // Recheck identity before closing, as well as before writing.
                    close_environment.verify_source(&close_source, close_config.target)?;
                    if account::get_current_account_id()
                        .map_err(|_| "source_changed")?
                        .as_deref()
                        != Some(close_source.id.as_str())
                    {
                        return Err("source_changed");
                    }
                    {
                        let d = close_data.lock().map_err(|_| "request_changed")?;
                        if d.revision != revision
                            || d.pending.is_none()
                            || d.canceled_source.as_deref() == Some(close_source.id.as_str())
                            || read_config().map_err(|_| "configuration_required")? != close_config
                        {
                            return Err("request_changed");
                        }
                    }
                    close_environment
                        .close_client(20, close_config.target.argument())
                        .map_err(|_| "clients_running")
                })
                .await
                .unwrap_or(Err("process_unknown"));
                if let Err(reason) = checked_close {
                    status.set("blocked", reason);
                    update_status(runtime, revision, status);
                }
            }
            return Ok(false);
        }

        if !pending.closed_since.is_some_and(|since| now - since >= 3) {
            status.set("pending", "ready");
            update_status(runtime, revision, status);
            return Ok(false);
        }
    } else {
        // VS Code mode: interrupt if in Mode::Stop
        if config.mode == Mode::Stop {
            environment.interrupt_vscode();
        }
    }

    let integration = AutoSwitchIntegration {
        data: runtime.data.clone(),
        pending: pending.clone(),
        config: config.clone(),
        environment: environment.clone(),
    };
    status.set("pending", "checking");
    update_status(runtime, revision, status);
    let result =
        account::switch_account(&candidate.id, config.target.argument(), &integration).await;
    let mut d = runtime.data.lock().map_err(|_| "State unavailable")?;
    d.commit_started = false;
    if d.revision != revision {
        return Ok(false);
    }
    let committed = result.is_ok();
    match result {
        Ok(()) => {
            d.pending = None;
            d.status.pending_id = None;
            let final_reason = if d.status.reason.as_deref() == Some("restarted") {
                "restarted"
            } else if config.mode == Mode::Stop && config.target == Target::Vscode {
                "paused_and_updated"
            } else {
                "credentials_updated"
            };
            d.status.set("completed", final_reason);
            d.cooldown_until = now + COOLDOWN_SECONDS;
            if write_pause(d.canceled_source.clone(), false).is_err() {
                d.failed = true;
            }
        }
        Err(_) => {
            if matches!(
                d.status.reason.as_deref(),
                Some("clients_running" | "process_unknown")
            ) {
                if let Some(p) = d.pending.as_mut() {
                    p.closed_since = None;
                }
            } else {
                d.pending = None;
                d.status.pending_id = None;
                d.canceled_source = Some(source_id.clone());
                d.failed = true;
                let _ = write_pause(Some(source_id), true);
                if matches!(d.status.reason.as_deref(), Some("checking") | None) {
                    d.status.set("blocked", "switch_failed");
                }
            }
        }
    }
    Ok(committed)
}

struct AutoSwitchIntegration<E: Environment> {
    data: Arc<Mutex<RuntimeData>>,
    pending: Pending,
    config: Config,
    environment: E,
}
impl<E: Environment> integration::SystemIntegration for AutoSwitchIntegration<E> {
    async fn on_account_switch(
        &self,
        target: &Account,
        target_ide: Option<&str>,
    ) -> Result<(), String> {
        let data = self.data.clone();
        let p = self.pending.clone();
        let config = self.config.clone();
        let target = target.clone();
        let environment = self.environment.clone();
        let target_ide = target_ide.map(str::to_owned);
        tokio::task::spawn_blocking(move || {
            let report = |reason: &str| -> Result<(), String> {
                if let Ok(mut d) = data.lock() {
                    if d.revision == p.revision {
                        d.status.set("blocked", reason);
                    }
                }
                Err(reason.into())
            };
            let source_before = account::load_account(&p.source_id)?;
            if let Err(reason) = environment.verify_source(&source_before, config.target) {
                return report(reason);
            }
            let _account_write = account::lock_account_file_updates()?;
            let source = account::load_account(&p.source_id)?;
            let latest_target = account::load_account(&target.id)?;
            if source.token.refresh_token != source_before.token.refresh_token {
                return report("credentials_changed");
            }
            let current = account::get_current_account_id()?;
            {
                let mut d = data.lock().map_err(|_| "state_unavailable")?;
                if d.revision != p.revision {
                    return Err("request_changed".into());
                }
                if read_config()? != config {
                    d.status.set("blocked", "configuration_required");
                    return Err("configuration_required".into());
                }
                if let Err(reason) = commit_guard(
                    &d,
                    &p,
                    &config,
                    current.as_deref(),
                    &source,
                    &latest_target,
                    environment.now(),
                    environment.clients(),
                ) {
                    d.status.set("blocked", reason);
                    return Err(reason.into());
                }
                write_pause(Some(p.source_id.clone()), true)?;
                d.commit_started = true;
                d.status.set("switching", "checking");
            }

            // 1. Process detection and graceful close
            let is_vscode = target_ide.as_deref() == Some("vscode");
            let is_ide = target_ide.as_deref() == Some("ide");
            let is_app = target_ide.as_deref() == Some("app");
            let is_all = target_ide.is_none();

            let app_running = if is_ide || is_vscode { false } else { environment.is_client_running(None) };
            let ide_running = if is_app || is_vscode { false } else { environment.is_client_running(Some("ide")) };

            if is_ide {
                if ide_running {
                    let _ = environment.close_client(20, Some("ide"));
                }
            } else if is_app {
                if app_running {
                    let _ = environment.close_client(20, None);
                }
            } else if is_all {
                if app_running {
                    let _ = environment.close_client(20, None);
                }
                if ide_running {
                    let _ = environment.close_client(20, Some("ide"));
                }
            }

            // 2. Write credentials
            environment.write_credentials(&latest_target, target_ide.as_deref())?;

            // 3. Smart relaunch
            if is_ide {
                if ide_running {
                    let _ = environment.start_client(Some("ide"));
                }
            } else if is_app {
                if app_running {
                    let _ = environment.start_client(None);
                }
            } else if is_all {
                if app_running {
                    let _ = environment.start_client(None);
                }
                if ide_running {
                    let _ = environment.start_client(Some("ide"));
                }
            }

            // 4. Update status in memory
            if let Ok(mut d) = data.lock() {
                if d.revision == p.revision {
                    let reason = if app_running || ide_running {
                        "restarted"
                    } else {
                        "credentials_updated"
                    };
                    d.status.set("completed", reason);
                }
            }

            Ok(())
        })
        .await
        .map_err(|_| "Auto switch worker failed.".to_string())?
    }
    fn update_tray(&self) {}
    fn show_notification(&self, _title: &str, _body: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    const NOW: i64 = 1_800_000_000;
    fn quota(weekly: f64, short: f64) -> QuotaData {
        serde_json::from_value(serde_json::json!({
            "models": [{"name":"gemini-test", "percentage":(short * 100.0) as i32, "reset_time":"2027-02-01T00:00:00Z"}],
            "last_updated":NOW, "subscription_tier":"PRO",
            "quota_groups": [{"display_name":"任意语言名称", "buckets":[
                {"bucket_id":"gemini-weekly","window":"weekly","remaining_fraction":weekly,"reset_time":"2027-02-01T00:00:00Z"},
                {"bucket_id":"gemini-5h","window":"5h","remaining_fraction":short,"reset_time":"2027-02-01T00:00:00Z"}
            ]}]
        })).unwrap()
    }
    fn account_fixture(id: &str, weekly: f64, short: f64) -> Account {
        let token = crate::models::TokenData::new(
            format!("fixture-{id}-access"),
            format!("fixture-{id}-refresh"),
            3600,
            None,
            None,
            None,
            true,
            None,
        );
        let mut a = Account::new(id.into(), format!("{id}@example.invalid"), token);
        a.quota = Some(quota(weekly, short));
        a
    }
    fn config(mode: Mode) -> Config {
        Config {
            enabled: true,
            mode,
            monitored_model: "gemini-test".into(),
            candidate_account_ids: vec!["B".into()],
            ..Config::default()
        }
    }
    fn runtime(mode: Mode) -> RuntimeData {
        RuntimeData {
            config: config(mode),
            ..RuntimeData::default()
        }
    }

    #[test]
    fn opt_in_and_config_validation() {
        assert!(!Config::default().enabled);
        assert_eq!(serde_json::from_str::<Config>("{}").unwrap().strategy, Strategy::Priority);
        assert!(!serde_json::from_str::<Config>("{}").unwrap().enabled);
        let mut c = Config::default();
        c.enabled = true;
        assert!(c.validate().is_err());
        c = config(Mode::Wait);
        assert!(c.validate().is_ok());
        c.monitored_model = "all".into();
        assert!(c.validate().is_ok());
        c.monitored_model = "".into();
        assert!(c.validate().is_ok());
        c.candidate_min_percentage = 10;
        assert!(c.validate().is_err());
        c.candidate_min_percentage = 30;
        c.candidate_account_ids = (0..101).map(|i| format!("acc_{i}")).collect();
        assert!(c.validate().is_err());
        c.candidate_account_ids = vec!["B".into(), "B".into()];
        assert!(c.validate().is_err());

        assert_eq!(serde_json::from_str::<Target>("\"all\"").unwrap(), Target::App);
        assert_eq!(serde_json::from_str::<Target>("\"app\"").unwrap(), Target::App);
        assert_eq!(serde_json::from_str::<Target>("\"app_cli\"").unwrap(), Target::AppCli);
        assert_eq!(serde_json::from_str::<Target>("\"desktop\"").unwrap(), Target::AppCli);
        assert_eq!(serde_json::from_str::<Target>("\"ide\"").unwrap(), Target::Ide);
        assert_eq!(serde_json::from_str::<Target>("\"vscode\"").unwrap(), Target::Vscode);
        assert_eq!(Target::App.argument(), None);
        assert_eq!(Target::AppCli.argument(), Some("app"));
        assert_eq!(Target::Ide.argument(), Some("ide"));
        assert_eq!(Target::Vscode.argument(), Some("vscode"));
    }
    #[test]
    fn prefers_5h_window_without_weekly_restriction() {
        assert_eq!(remaining(&quota(0.08, 0.8), "gemini-test", NOW), Ok(80.0));
        assert_eq!(remaining(&quota(0.8, 0.08), "gemini-test", NOW), Ok(8.0));
        assert_eq!(remaining(&quota(0.08, 0.8), "all", NOW), Ok(80.0));
        assert_eq!(remaining(&quota(0.8, 0.08), "", NOW), Ok(8.0));
        assert_eq!(remaining(&quota(0.08, 0.8), "gemini", NOW), Ok(80.0));
        assert_eq!(remaining(&quota(0.08, 0.8), "claude", NOW), Err("unknown_pool"));
        let mut q = quota(0.8, 0.8);
        q.quota_groups.as_mut().unwrap()[0].buckets[0].bucket_id = "3p-weekly".into();
        assert_eq!(remaining(&q, "gemini-test", NOW), Err("unknown_pool"));
    }
    #[test]
    fn unknown_stale_reset_and_invalid_numbers_fail_closed() {
        let mut q = quota(0.8, 0.8);
        q.quota_groups = None;
        assert_eq!(remaining(&q, "gemini-test", NOW), Err("unknown_pool"));
        q = quota(0.8, 0.8);
        q.last_updated = NOW - QUOTA_MAX_AGE - 1;
        assert_eq!(remaining(&q, "gemini-test", NOW), Err("stale_quota"));
        q = quota(0.8, 0.8);
        q.quota_groups.as_mut().unwrap()[0].buckets[0].reset_time = "2020-01-01T00:00:00Z".into();
        assert_eq!(remaining(&q, "gemini-test", NOW), Err("stale_quota"));
        q = quota(0.8, 0.8);
        q.quota_groups.as_mut().unwrap()[0].buckets[0].remaining_fraction = f64::NAN;
        assert_eq!(remaining(&q, "gemini-test", NOW), Err("unknown_pool"));
        assert_eq!(
            remaining(&quota(0.8, 0.8), "missing-model", NOW),
            Err("unknown_pool")
        );
    }
    #[test]
    fn free_weekly_only_is_supported_without_inventing_paid_capacity() {
        let mut q = quota(0.08, 0.8);
        q.quota_groups.as_mut().unwrap()[0].buckets.pop();
        assert_eq!(remaining(&q, "gemini-test", NOW), Err("unknown_pool"));
        q.subscription_tier = Some("FREE".into());
        assert_eq!(remaining(&q, "gemini-test", NOW), Ok(8.0));
    }
    #[test]
    fn queue_is_stable_and_loses_closed_evidence_on_activity() {
        let mut d = runtime(Mode::Wait);
        let first = advance_pending(&mut d, "A", "B", NOW, ProcessState::Running);
        assert!(first.closed_since.is_none());
        let closed = advance_pending(&mut d, "A", "B", NOW + 1, ProcessState::Closed);
        assert_eq!(first.id, closed.id);
        assert_eq!(closed.closed_since, Some(NOW + 1));
        let unknown = advance_pending(&mut d, "A", "B", NOW + 5, ProcessState::Unknown);
        assert!(unknown.closed_since.is_none());
        let changed = advance_pending(&mut d, "A", "C", NOW + 6, ProcessState::Closed);
        assert_ne!(changed.id, first.id);
        assert_eq!(changed.closed_since, Some(NOW + 6));
    }
    #[test]
    fn commit_revalidates_cancel_source_config_pool_and_process() {
        let mut d = runtime(Mode::Wait);
        let p = advance_pending(&mut d, "A", "B", NOW, ProcessState::Closed);
        let a = account_fixture("A", 0.08, 0.08);
        let mut b = account_fixture("B", 0.8, 0.8);
        let c = config(Mode::Wait);
        let validate = |d: &RuntimeData, b: &Account, current, process| {
            commit_guard(d, &p, &c, current, &a, b, NOW, process)
        };
        assert_eq!(validate(&d, &b, Some("A"), ProcessState::Closed), Ok(()));
        assert_eq!(
            validate(&d, &b, Some("C"), ProcessState::Closed),
            Err("source_changed")
        );
        assert_eq!(
            validate(&d, &b, Some("A"), ProcessState::Running),
            Err("clients_running")
        );
        assert_eq!(
            validate(&d, &b, Some("A"), ProcessState::Unknown),
            Err("process_unknown")
        );
        b.quota = Some(quota(0.9, 0.2));
        assert_eq!(
            validate(&d, &b, Some("A"), ProcessState::Closed),
            Err("no_candidate")
        );
        b.quota = Some(quota(0.8, 0.8));
        b.disabled = true;
        assert_eq!(
            validate(&d, &b, Some("A"), ProcessState::Closed),
            Err("no_candidate")
        );
        b.disabled = false;
        d.revision += 1;
        assert_eq!(
            validate(&d, &b, Some("A"), ProcessState::Closed),
            Err("request_changed")
        );
        d.revision -= 1;
        d.pending = None;
        assert_eq!(
            validate(&d, &b, Some("A"), ProcessState::Closed),
            Err("request_changed")
        );
    }
    #[test]
    fn expired_quota_or_recovered_source_cannot_commit() {
        let mut d = runtime(Mode::Wait);
        let p = advance_pending(&mut d, "A", "B", NOW, ProcessState::Closed);
        let mut a = account_fixture("A", 0.08, 0.08);
        let b = account_fixture("B", 0.8, 0.8);
        assert_eq!(
            commit_guard(
                &d,
                &p,
                &d.config,
                Some("A"),
                &a,
                &b,
                NOW + 181,
                ProcessState::Closed
            ),
            Err("no_quota")
        );
        a.quota = Some(quota(0.9, 0.9));
        assert_eq!(
            commit_guard(
                &d,
                &p,
                &d.config,
                Some("A"),
                &a,
                &b,
                NOW,
                ProcessState::Closed
            ),
            Err("no_quota")
        );
    }
    #[test]
    fn both_modes_guard_and_file_fixture_only_commit_after_client_exit() {
        for mode in [Mode::Wait, Mode::Stop] {
            let home = tempfile::tempdir().unwrap();
            let session = home.path().join("native-session");
            let a = account_fixture("A", 0.08, 0.08);
            let b = account_fixture("B", 0.8, 0.8);
            cli_credentials::write_session(&session, &cli_credentials::payload(&a.token).unwrap())
                .unwrap();
            let original = std::fs::read(&session).unwrap();
            let mut d = runtime(mode);
            let mut writes = 0;
            // Native Stop alone does not exit the process. Both modes stay pending.
            for state in [
                ProcessState::Running,
                ProcessState::Unknown,
                ProcessState::Running,
            ] {
                let p = advance_pending(&mut d, "A", "B", NOW, state);
                assert!(commit_guard(&d, &p, &d.config, Some("A"), &a, &b, NOW, state).is_err());
                assert_eq!(std::fs::read(&session).unwrap(), original);
            }
            let first = advance_pending(&mut d, "A", "B", NOW + 1, ProcessState::Closed);
            assert_eq!(first.closed_since, Some(NOW + 1));
            let pending = advance_pending(&mut d, "A", "B", NOW + 5, ProcessState::Closed);
            assert!(NOW + 5 - pending.closed_since.unwrap() >= 3);
            commit_guard(
                &d,
                &pending,
                &d.config,
                Some("A"),
                &a,
                &b,
                NOW + 5,
                ProcessState::Closed,
            )
            .unwrap();
            cli_credentials::write_session(&session, &cli_credentials::payload(&b.token).unwrap())
                .unwrap();
            writes += 1;
            d.pending = None;
            assert!(commit_guard(
                &d,
                &pending,
                &d.config,
                Some("B"),
                &a,
                &b,
                NOW + 6,
                ProcessState::Closed
            )
            .is_err());
            let written: serde_json::Value =
                serde_json::from_slice(&std::fs::read(session).unwrap()).unwrap();
            assert_eq!(written["token"]["refresh_token"], "fixture-B-refresh");
            assert_eq!(writes, 1);
            assert!(!home.path().join("oauth_creds.json").exists());
        }
    }
    #[test]
    fn native_file_absence_is_distinct_from_invalid_existing_path() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("native");
        assert_eq!(native_session_exists(&path), Ok(false));
        std::fs::create_dir(&path).unwrap();
        assert_eq!(native_session_exists(&path), Err("credentials_changed"));
    }
    #[test]
    fn background_executors_are_not_mistaken_for_closed_clients() {
        use std::path::Path;
        assert!(is_client_process("AGY.exe", None, &[]));
        assert!(is_client_process(
            "language_server_macos",
            Some(Path::new(
                "/Applications/Antigravity.app/Contents/Resources/server"
            )),
            &[]
        ));
        assert!(is_client_process(
            "language_server",
            Some(Path::new(r"C:\Users\test\Programs\Antigravity\server.exe")),
            &[]
        ));
        assert!(is_client_process(
            "crashpad_handler",
            Some(Path::new(
                "/Applications/Antigravity IDE.app/Contents/Frameworks/helper"
            )),
            &[]
        ));
        assert!(is_client_process(
            "language_server",
            Some(Path::new("/opt/antigravity-ide/resources/server")),
            &[]
        ));
        assert!(is_client_process(
            "language_server",
            Some(Path::new("/opt/custom/resources/server")),
            &[Path::new("/opt/custom/client").to_path_buf()]
        ));
        assert!(!is_client_process(
            "other_program",
            Some(Path::new("/usr/bin/resources/helper")),
            &[Path::new("/usr/bin/agy").to_path_buf()]
        ));
        assert!(!is_client_process(
            "antigravity-tools",
            Some(Path::new("/Applications/Antigravity.app/tools")),
            &[]
        ));
        assert!(!is_client_process(
            "bash",
            Some(Path::new("/bin/bash")),
            &[]
        ));
    }

    #[test]
    fn pause_record_roundtrip_contains_no_credentials() {
        let pause = PauseRecord {
            source_id: Some("A".into()),
            failed: true,
        };
        let encoded = serde_json::to_string(&pause).unwrap();
        let restored: PauseRecord = serde_json::from_str(&encoded).unwrap();
        assert_eq!(restored.source_id.as_deref(), Some("A"));
        assert!(restored.failed);
        assert!(!encoded.contains("token"));
        assert!(!encoded.contains("email"));
    }
    #[derive(Clone)]
    struct FixtureEnvironment {
        now: Arc<std::sync::atomic::AtomicI64>,
        process: Arc<std::sync::atomic::AtomicU8>,
        writes: Arc<std::sync::atomic::AtomicUsize>,
        source_checks: Arc<std::sync::atomic::AtomicUsize>,
        closes: Arc<std::sync::atomic::AtomicUsize>,
        fail_save: Arc<std::sync::atomic::AtomicBool>,
        low_backup: Arc<std::sync::atomic::AtomicBool>,
        fail_write: Arc<std::sync::atomic::AtomicBool>,
        on_verify: Arc<Mutex<Option<Box<dyn Fn() + Send + Sync>>>>,
    }
    impl Default for FixtureEnvironment {
        fn default() -> Self {
            Self {
                now: Arc::new(std::sync::atomic::AtomicI64::new(NOW)),
                process: Arc::new(std::sync::atomic::AtomicU8::new(1)),
                writes: Arc::default(),
                source_checks: Arc::default(),
                closes: Arc::default(),
                fail_save: Arc::default(),
                low_backup: Arc::default(),
                fail_write: Arc::default(),
                on_verify: Arc::default(),
            }
        }
    }
    impl Environment for FixtureEnvironment {
        fn now(&self) -> i64 {
            self.now.load(std::sync::atomic::Ordering::SeqCst)
        }
        fn clients(&self) -> ProcessState {
            match self.process.load(std::sync::atomic::Ordering::SeqCst) {
                0 => ProcessState::Closed,
                1 => ProcessState::Running,
                _ => ProcessState::Unknown,
            }
        }
        fn close_client(&self, _timeout: u64, _target_ide: Option<&str>) -> Result<(), String> {
            self.closes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
        async fn fetch_quota(&self, a: &mut Account) -> Result<QuotaData, String> {
            let mut q = if a.id == "B" && self.low_backup.load(std::sync::atomic::Ordering::SeqCst)
            {
                quota(0.9, 0.01)
            } else {
                a.quota.clone().unwrap()
            };
            q.last_updated = self.now();
            Ok(q)
        }
        fn save_quota(&self, id: &str, q: QuotaData) -> Result<(), String> {
            if id == "B" && self.fail_save.load(std::sync::atomic::Ordering::SeqCst) {
                return Err("injected quota persistence failure".into());
            }
            account::update_account_quota(id, q)
        }
        fn verify_source(&self, source: &Account, _target: Target) -> Result<(), &'static str> {
            self.source_checks
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if let Some(callback) = self.on_verify.lock().unwrap().take() {
                callback();
            }
            let path = account::get_data_dir().unwrap().join("fixture-native.json");
            let value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            if value["token"]["refresh_token"] == source.token.refresh_token {
                Ok(())
            } else {
                Err("credentials_changed")
            }
        }
        fn write_credentials(&self, target: &Account, _target_ide: Option<&str>) -> Result<(), String> {
            // Proves production account::switch_account took the cross-process
            // lock before entering the injected native boundary.
            let root = account::get_data_dir()?;
            assert!(
                matches!(crate::cli::SwitchLock::acquire(&root),Err(e) if e=="another_account_switch_in_progress")
            );
            self.writes
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            cli_credentials::write_session(
                &root.join("fixture-native.json"),
                &cli_credentials::payload(&target.token)?,
            )?;
            // Simulate metadata saved by another caller while native work finishes;
            // final last-used handling must retain it rather than saving an old clone.
            let mut latest = account::load_account(&target.id)?;
            latest.custom_label = Some("keep-concurrent-label".into());
            account::save_account(&latest)?;
            if self.fail_write.load(std::sync::atomic::Ordering::SeqCst) {
                Err("injected partial credential commit".into())
            } else {
                Ok(())
            }
        }
    }
    fn fixture_setup(mode: Mode) -> Runtime {
        let root = account::get_data_dir().unwrap();
        let a = account_fixture("A", 0.08, 0.08);
        let b = account_fixture("B", 0.8, 0.8);
        account::save_account(&a).unwrap();
        account::save_account(&b).unwrap();
        let index: crate::models::AccountIndex = serde_json::from_value(
            serde_json::json!({"version":"2.0","current_account_id":"A","accounts":[
                {"id":"A","email":a.email,"created_at":1,"last_used":1},
                {"id":"B","email":b.email,"created_at":1,"last_used":1}
            ]}),
        )
        .unwrap();
        account::save_account_index(&index).unwrap();
        cli_credentials::write_session(
            &root.join("fixture-native.json"),
            &cli_credentials::payload(&a.token).unwrap(),
        )
        .unwrap();
        let c = config(mode);
        crate::utils::fs::write_atomic(&config_path().unwrap(), &serde_json::to_vec(&c).unwrap())
            .unwrap();
        write_pause(None, false).unwrap();
        Runtime {
            data: Arc::new(Mutex::new(RuntimeData {
                config: c,
                ..RuntimeData::default()
            })),
            tick: tokio::sync::Mutex::new(()),
        }
    }
    #[test]
    fn production_coordinator_in_isolated_process() {
        let directory = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "modules::auto_switch::tests::coordinator_fixture_child",
                "--ignored",
                "--nocapture",
            ])
            .env("ABV_DATA_DIR", directory.path())
            .env("AGY_SAFE_SWITCH_FIXTURE", directory.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    #[test]
    #[ignore = "helper launched only with an isolated temporary account store"]
    fn coordinator_fixture_child() {
        use std::sync::atomic::Ordering::SeqCst;
        let Some(root) = std::env::var_os("AGY_SAFE_SWITCH_FIXTURE") else {
            return;
        };
        assert_eq!(std::env::var_os("ABV_DATA_DIR"), Some(root));
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            for mode in [Mode::Wait, Mode::Stop] {
                let r = fixture_setup(mode);
                let env = FixtureEnvironment::default();
                assert!(!evaluate_core(&r, env.clone(), false).await.unwrap());
                assert_eq!(r.data.lock().unwrap().status.phase, "pending");
                env.process.store(2, SeqCst);
                assert!(!evaluate_core(&r, env.clone(), false).await.unwrap());
                assert_eq!(
                    r.data.lock().unwrap().status.reason.as_deref(),
                    Some("process_unknown")
                );
                env.process.store(0, SeqCst);
                env.now.store(NOW + 5, SeqCst);
                assert!(!evaluate_core(&r, env.clone(), false).await.unwrap());
                env.now.store(NOW + 10, SeqCst);
                assert!(evaluate_core(&r, env.clone(), false).await.unwrap());
                assert_eq!(
                    account::get_current_account_id().unwrap().as_deref(),
                    Some("B")
                );
                assert_eq!(
                    account::load_account("B").unwrap().custom_label.as_deref(),
                    Some("keep-concurrent-label")
                );
                assert_eq!(env.source_checks.load(SeqCst), 2);
                assert_eq!(env.closes.load(SeqCst), 1);
                assert_eq!(env.writes.load(SeqCst), 1);
                assert_eq!(r.data.lock().unwrap().status.phase, "completed");
                assert!(!read_pause().unwrap().failed);
                assert!(!evaluate_core(&r, env.clone(), false).await.unwrap());
                assert_eq!(env.writes.load(SeqCst), 1);
            }
            // Exercise production selection, including wraparound, configured priority,
            // an absent source, disabled candidates, and forbidden candidates.
            for (strategy, source, unavailable, expected) in [
                (Strategy::RoundRobin, "A", None, "B"),
                (Strategy::RoundRobin, "B", None, "C"),
                (Strategy::RoundRobin, "C", None, "A"),
                (Strategy::Priority, "B", None, "A"),
                (Strategy::RoundRobin, "X", None, "A"),
                (Strategy::RoundRobin, "A", Some("disabled"), "C"),
                (Strategy::Priority, "C", Some("forbidden"), "B"),
            ] {
                let r = fixture_setup(Mode::Wait);
                for id in ["A", "B", "C", "X"] {
                    let mut a = account_fixture(id, 0.8, 0.8);
                    if id == source { a.quota = Some(quota(0.08, 0.08)); }
                    if id == "B" && unavailable == Some("disabled") { a.disabled = true; }
                    if id == "A" && unavailable == Some("forbidden") { a.quota.as_mut().unwrap().is_forbidden = true; }
                    account::save_account(&a).unwrap();
                }
                account::set_current_account_id(source).unwrap();
                { let mut d = r.data.lock().unwrap(); d.config.strategy = strategy; d.config.candidate_account_ids = vec!["A".into(), "B".into(), "C".into()]; }
                let env = FixtureEnvironment::default();
                env.process.store(2, SeqCst); // Inspect selection without mutating native credentials.
                assert!(!evaluate_core(&r, env.clone(), false).await.unwrap());
                assert_eq!(r.data.lock().unwrap().status.target_account_id.as_deref(), Some(expected));
                assert_eq!(env.writes.load(SeqCst), 0);
            }
            // The shared switch core preserves newer quota/disable metadata.
            fixture_setup(Mode::Wait);
            account::switch_merge_fixture_check();
            // Cancel A, visit healthy B, then return to still-low A: A remains canceled.
            let r = fixture_setup(Mode::Wait);
            let env = FixtureEnvironment::default();
            evaluate_core(&r, env.clone(), false).await.unwrap();
            let id = r.data.lock().unwrap().status.pending_id.clone().unwrap();
            cancel_pending(&r, &id).unwrap();
            account::set_current_account_id("B").unwrap();
            evaluate_core(&r, env.clone(), false).await.unwrap();
            assert_eq!(r.data.lock().unwrap().canceled_source.as_deref(), Some("A"));
            account::set_current_account_id("A").unwrap();
            env.process.store(0, SeqCst);
            env.now.store(NOW + 10, SeqCst);
            assert!(!evaluate_core(&r, env.clone(), false).await.unwrap());
            assert_eq!(r.data.lock().unwrap().status.phase, "canceled");
            assert_eq!(env.writes.load(SeqCst), 0);
            // A successful unrelated B -> C switch must persist A's cancellation.
            let c = account_fixture("C", 0.8, 0.8);
            account::save_account(&c).unwrap();
            let mut index = account::load_account_index().unwrap();
            index.accounts.push(
                serde_json::from_value(
                    serde_json::json!({"id":"C","email":c.email,"created_at":1,"last_used":1}),
                )
                .unwrap(),
            );
            account::save_account_index(&index).unwrap();
            {
                let mut data = r.data.lock().unwrap();
                data.config.candidate_account_ids.push("C".into());
                crate::utils::fs::write_atomic(
                    &config_path().unwrap(),
                    &serde_json::to_vec(&data.config).unwrap(),
                )
                .unwrap();
            }
            account::set_current_account_id("B").unwrap();
            account::update_account_quota("B", quota(0.8, 0.08)).unwrap();
            let b = account::load_account("B").unwrap();
            cli_credentials::write_session(
                &account::get_data_dir().unwrap().join("fixture-native.json"),
                &cli_credentials::payload(&b.token).unwrap(),
            )
            .unwrap();
            env.now.store(NOW + 15, SeqCst);
            assert!(!evaluate_core(&r, env.clone(), false).await.unwrap());
            env.now.store(NOW + 20, SeqCst);
            assert!(evaluate_core(&r, env.clone(), false).await.unwrap());
            assert_eq!(
                account::get_current_account_id().unwrap().as_deref(),
                Some("C")
            );
            assert_eq!(read_pause().unwrap().source_id.as_deref(), Some("A"));
            // Cancel during a slow native identity read must stay visibly canceled.
            let r = Arc::new(fixture_setup(Mode::Wait));
            let env = FixtureEnvironment::default();
            env.process.store(0, SeqCst);
            evaluate_core(&r, env.clone(), false).await.unwrap();
            let id = r.data.lock().unwrap().status.pending_id.clone().unwrap();
            let cancel_runtime = r.clone();
            *env.on_verify.lock().unwrap() = Some(Box::new(move || {
                cancel_pending(&cancel_runtime, &id).unwrap();
            }));
            env.now.store(NOW + 5, SeqCst);
            assert!(!evaluate_core(&r, env.clone(), false).await.unwrap());
            assert_eq!(r.data.lock().unwrap().status.phase, "canceled");
            assert_eq!(env.writes.load(SeqCst), 0);
            // A fresh low backup response which failed to persist may not fall
            // back to the older high cached record during the throttle window.
            let r = fixture_setup(Mode::Wait);
            let env = FixtureEnvironment::default();
            env.low_backup.store(true, SeqCst);
            env.fail_save.store(true, SeqCst);
            assert!(!evaluate_core(&r, env.clone(), false).await.unwrap());
            env.now.store(NOW + 5, SeqCst);
            assert!(!evaluate_core(&r, env.clone(), false).await.unwrap());
            assert_eq!(
                r.data.lock().unwrap().status.reason.as_deref(),
                Some("no_candidate")
            );
            assert_eq!(env.writes.load(SeqCst), 0);
            // A partial native write leaves the index at A and a durable failure
            // journal; no subsequent coordinator tick is allowed to retry.
            let r = fixture_setup(Mode::Stop);
            let env = FixtureEnvironment::default();
            env.process.store(0, SeqCst);
            env.fail_write.store(true, SeqCst);
            evaluate_core(&r, env.clone(), false).await.unwrap();
            env.now.store(NOW + 5, SeqCst);
            assert!(!evaluate_core(&r, env.clone(), false).await.unwrap());
            assert_eq!(
                account::get_current_account_id().unwrap().as_deref(),
                Some("A")
            );
            assert!(read_pause().unwrap().failed);
            assert!(r.data.lock().unwrap().failed);
            env.now.store(NOW + 70, SeqCst);
            evaluate_core(&r, env.clone(), false).await.unwrap();
            assert_eq!(env.writes.load(SeqCst), 1);
            // A wrong native identity must not close a running App in either mode.
            for mode in [Mode::Wait, Mode::Stop] {
                let r = fixture_setup(mode);
                let env = FixtureEnvironment::default();
                let b = account::load_account("B").unwrap();
                cli_credentials::write_session(
                    &account::get_data_dir().unwrap().join("fixture-native.json"),
                    &cli_credentials::payload(&b.token).unwrap(),
                )
                .unwrap();
                assert!(!evaluate_core(&r, env.clone(), false).await.unwrap());
                assert_eq!(
                    r.data.lock().unwrap().status.reason.as_deref(),
                    Some("credentials_changed")
                );
                assert_eq!(env.closes.load(SeqCst), 0);
                assert_eq!(env.writes.load(SeqCst), 0);
                assert_eq!(
                    account::get_current_account_id().unwrap().as_deref(),
                    Some("A")
                );
            }
            // Wrong native identity is also rejected by the integration callback.
            let r = fixture_setup(Mode::Wait);
            let env = FixtureEnvironment::default();
            env.process.store(0, SeqCst);
            let b = account::load_account("B").unwrap();
            cli_credentials::write_session(
                &account::get_data_dir().unwrap().join("fixture-native.json"),
                &cli_credentials::payload(&b.token).unwrap(),
            )
            .unwrap();
            evaluate_core(&r, env.clone(), false).await.unwrap();
            env.now.store(NOW + 5, SeqCst);
            assert!(!evaluate_core(&r, env.clone(), false).await.unwrap());
            assert_eq!(env.writes.load(SeqCst), 0);
            assert_eq!(
                account::get_current_account_id().unwrap().as_deref(),
                Some("A")
            );
        });
    }

}

#[cfg(test)]
mod config_file_tests {
    use super::*;
    #[test]
    fn external_edits_invalidate_pending_work_and_detect_stale_editors() {
        let dir = tempfile::tempdir().unwrap();
        let original = Config::default();
        let edited = Config { strategy: Strategy::RoundRobin, reserve_percentage: 15, ..original.clone() };
        save_config_at(dir.path(), &original, &edited).unwrap();
        assert!(save_config_at(dir.path(), &original, &original).is_err());
        assert_eq!(read_config_at(dir.path()).unwrap(), edited);
        let runtime = Runtime::default();
        { let mut d = runtime.data.lock().unwrap(); d.failed = true; d.canceled_source = Some("fixture".into()); }
        sync_external_config_at(&runtime, dir.path()).unwrap();
        let d = runtime.data.lock().unwrap();
        assert_eq!(d.config, edited); assert_eq!(d.revision, 1);
        assert!(d.pending.is_none()); assert!(d.canceled_source.is_none()); assert!(!d.failed);
    }
    #[test]
    fn settings_cannot_change_during_a_credential_commit() {
        let dir = tempfile::tempdir().unwrap();
        let guard = crate::cli::SwitchLock::acquire(dir.path()).unwrap();
        let edited = Config { reserve_percentage: 15, ..Config::default() };
        assert_eq!(save_config_at(dir.path(), &Config::default(), &edited).unwrap_err(), "another_account_switch_in_progress");
        assert!(!dir.path().join(CONFIG_FILE).exists()); drop(guard);
        save_config_at(dir.path(), &Config::default(), &edited).unwrap();
        let runtime = Runtime::default(); runtime.data.lock().unwrap().commit_started = true;
        sync_external_config_at(&runtime, dir.path()).unwrap();
        assert_eq!(runtime.data.lock().unwrap().config, Config::default());
    }
}
