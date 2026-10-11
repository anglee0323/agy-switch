use crate::models::{Account, TokenData};
use crate::modules::{account, db};
use crate::utils::protobuf;
use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ImportedOAuthState {
    pub refresh_token: String,
    pub is_gcp_tos: bool,
    pub project_id: Option<String>,
}

pub(crate) enum CurrentIdentity {
    AppEmail(String),
    Credentials(ImportedOAuthState),
}

impl CurrentIdentity {
    pub(crate) fn matches(&self, account: &Account) -> bool {
        match self {
            Self::AppEmail(email) => account.email.eq_ignore_ascii_case(email),
            Self::Credentials(state) => account.token.refresh_token == state.refresh_token,
        }
    }
}

fn resolve_current_identity(
    live_email: impl FnOnce() -> Result<Option<String>, String>,
    credentials: impl FnOnce() -> Result<ImportedOAuthState, String>,
) -> Result<CurrentIdentity, String> {
    match live_email()? {
        Some(email) => Ok(CurrentIdentity::AppEmail(email)),
        None => credentials().map(CurrentIdentity::Credentials),
    }
}

/// A live App identity takes precedence over shared, possibly stale credentials.
/// Failure to verify a running App must not turn into a keyring fallback.
pub(crate) fn get_current_identity(target_ide: Option<&str>) -> Result<CurrentIdentity, String> {
    if matches!(target_ide, None | Some("app" | "app_cli" | "all")) {
        let config = crate::modules::config::load_app_config()?;
        return resolve_current_identity(
            || {
                crate::modules::app_identity::running_email(config.antigravity_executable.as_deref())
            },
            || get_oauth_state_from_db(target_ide),
        );
    }
    get_oauth_state_from_db(target_ide).map(CurrentIdentity::Credentials)
}

/// Scan and import V1 data
pub async fn import_from_v1() -> Result<Vec<Account>, String> {
    use crate::modules::oauth;

    let home = dirs::home_dir().ok_or("Failed to get home directory")?;

    // V1 data directory (confirmed cross-platform consistency from utils.py)
    let v1_dir = home.join(".antigravity-agent");

    let mut imported_accounts = Vec::new();

    // Try multiple possible filenames
    let index_files = vec![
        "antigravity_accounts.json", // Directly use string literal
        "accounts.json",
    ];

    let mut found_index = false;

    for index_filename in index_files {
        let v1_accounts_path = v1_dir.join(index_filename);

        if !v1_accounts_path.exists() {
            continue;
        }

        found_index = true;
        crate::modules::logger::log_info(&format!("V1 data discovered: {:?}", v1_accounts_path));

        let content = match fs::read_to_string(&v1_accounts_path) {
            Ok(c) => c,
            Err(e) => {
                crate::modules::logger::log_warn(&format!("Failed to read index: {}", e));
                continue;
            }
        };

        let v1_index: Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                crate::modules::logger::log_warn(&format!("Failed to parse index JSON: {}", e));
                continue;
            }
        };

        // Compatible with two formats: direct map, or contains "accounts" field
        let accounts_map = if let Some(map) = v1_index.as_object() {
            if let Some(accounts) = map.get("accounts").and_then(|v| v.as_object()) {
                accounts
            } else {
                map
            }
        } else {
            continue;
        };

        for (id, acc_info) in accounts_map {
            let email_placeholder = acc_info
                .get("email")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();

            // Skip non-account keys (e.g. "current_account_id")
            if !acc_info.is_object() {
                continue;
            }

            let backup_file_str = acc_info.get("backup_file").and_then(|v| v.as_str());
            let data_file_str = acc_info.get("data_file").and_then(|v| v.as_str());

            // Prefer backup_file, then data_file
            let target_file = backup_file_str.or(data_file_str);

            if target_file.is_none() {
                crate::modules::logger::log_warn(&format!(
                    "Account {} ({}) missing data file path",
                    id, email_placeholder
                ));
                continue;
            }

            let mut backup_path = PathBuf::from(target_file.unwrap());

            // If relative path, try joining with v1_dir
            if !backup_path.exists() {
                backup_path = v1_dir.join(backup_path.file_name().unwrap_or_default());
            }

            // Try joining data/ or backups/ subdirectories again
            if !backup_path.exists() {
                let file_name = backup_path.file_name().unwrap_or_default();
                let try_backups = v1_dir.join("backups").join(file_name);
                if try_backups.exists() {
                    backup_path = try_backups;
                } else {
                    let try_accounts = v1_dir.join("accounts").join(file_name);
                    if try_accounts.exists() {
                        backup_path = try_accounts;
                    }
                }
            }

            if !backup_path.exists() {
                crate::modules::logger::log_warn(&format!(
                    "Account {} ({}) backup file not found: {:?}",
                    id, email_placeholder, backup_path
                ));
                continue;
            }

            // Read backup file
            if let Ok(backup_content) = fs::read_to_string(&backup_path) {
                if let Ok(backup_json) = serde_json::from_str::<Value>(&backup_content) {
                    // Compatible with two formats:
                    // 1. V1 backup: jetskiStateSync.agentManagerInitState -> Protobuf
                    // 2. V2/Script data: JSON containing "token" field

                    let mut refresh_token_opt = None;

                    // Try format 2
                    if let Some(token_data) = backup_json.get("token") {
                        if let Some(rt) = token_data.get("refresh_token").and_then(|v| v.as_str()) {
                            refresh_token_opt = Some(rt.to_string());
                        }
                    }

                    // Try format 1
                    if refresh_token_opt.is_none() {
                        if let Some(state_b64) = backup_json
                            .get("jetskiStateSync.agentManagerInitState")
                            .and_then(|v| v.as_str())
                        {
                            // Parse Protobuf
                            if let Ok(blob) = general_purpose::STANDARD.decode(state_b64) {
                                if let Ok(Some(oauth_data)) = protobuf::find_field(&blob, 6) {
                                    if let Ok(Some(refresh_bytes)) =
                                        protobuf::find_field(&oauth_data, 3)
                                    {
                                        if let Ok(rt) = String::from_utf8(refresh_bytes) {
                                            refresh_token_opt = Some(rt);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some(refresh_token) = refresh_token_opt {
                        crate::modules::logger::log_info(&format!(
                            "Importing account: {}",
                            email_placeholder
                        ));
                        let (email, access_token, expires_in, oauth_client_key) =
                            match oauth::refresh_access_token(&refresh_token, None).await {
                                Ok(token_resp) => {
                                    let oauth_client_key = token_resp.oauth_client_key.clone();
                                    match oauth::get_user_info(&token_resp.access_token).await {
                                        Ok(user_info) => (
                                            user_info.email,
                                            token_resp.access_token,
                                            token_resp.expires_in,
                                            oauth_client_key,
                                        ),
                                        Err(_) => (
                                            email_placeholder.clone(),
                                            token_resp.access_token,
                                            token_resp.expires_in,
                                            oauth_client_key,
                                        ),
                                    }
                                }
                                Err(e) => {
                                    crate::modules::logger::log_warn(&format!(
                                        "Token refresh failed (likely expired): {}",
                                        e
                                    ));
                                    (
                                        email_placeholder.clone(),
                                        "imported_access_token".to_string(),
                                        0,
                                        None,
                                    )
                                }
                            };
                        let token_data = TokenData::new(
                            access_token,
                            refresh_token,
                            expires_in,
                            Some(email.clone()),
                            None, // project_id will be fetched on demand
                            None, // session_id
                            true, // V1 tokens are Antigravity Google OAuth tokens
                            None, // V1 doesn't have id_token saved
                        )
                        .with_oauth_client_key(oauth_client_key);
                        // Name already fetched in get_user_info at line 153, but outside match scope, use None to be safe
                        match account::upsert_account(email.clone(), None, token_data) {
                            Ok(acc) => {
                                crate::modules::logger::log_info(&format!(
                                    "Import successful: {}",
                                    email
                                ));
                                imported_accounts.push(acc);
                            }
                            Err(e) => crate::modules::logger::log_error(&format!(
                                "Import save failed {}: {}",
                                email, e
                            )),
                        }
                    } else {
                        crate::modules::logger::log_warn(&format!(
                            "Account {} data file missing Refresh Token",
                            email_placeholder
                        ));
                    }
                }
            }
        }
    }

    if !found_index {
        return Err("V1 account data file not found".to_string());
    }

    Ok(imported_accounts)
}

/// Import account from custom database path
pub async fn import_from_custom_db_path(path_str: String) -> Result<Account, String> {
    use crate::modules::oauth;

    let path = PathBuf::from(path_str);
    if !path.exists() {
        return Err(format!("File does not exist: {:?}", path));
    }

    let oauth_state = extract_oauth_state_from_file(&path)?;
    let refresh_token = oauth_state.refresh_token.clone();

    // 3. Use Refresh Token to get latest Access Token and user info
    crate::modules::logger::log_info("Getting user info using Refresh Token...");
    let token_resp = oauth::refresh_access_token(&refresh_token, None).await?;
    let user_info = oauth::get_user_info(&token_resp.access_token).await?;

    let email = user_info.email;

    crate::modules::logger::log_info(&format!("Successfully retrieved account info: {}", email));

    let token_data = TokenData::new(
        token_resp.access_token,
        refresh_token,
        token_resp.expires_in,
        Some(email.clone()),
        oauth_state.project_id,
        None, // session_id will be generated in token_manager
        oauth_state.is_gcp_tos,
        token_resp.id_token,
    )
    .with_oauth_client_key(token_resp.oauth_client_key);
    // 4. Add or update account
    account::upsert_account(email.clone(), user_info.name, token_data)
}

#[derive(serde::Serialize)]
pub struct LocalImportReport {
    pub accounts: Vec<Account>,
    pub sources: Vec<LocalImportSource>,
}
#[derive(serde::Serialize)]
pub struct LocalImportSource {
    pub source: String,
    pub status: String,
}

fn discover_local_sources(target: Option<&str>) -> Vec<(String, Result<ImportedOAuthState, String>)> {
    use crate::modules::integration;
    let mut sources = Vec::new();
    if matches!(target, None | Some("all" | "app" | "app_cli" | "agy")) {
        sources.push(("App / CLI credential store".into(), integration::read_from_system_keyring()));
        sources.push(("agy CLI session".into(), integration::read_cli_credentials()));
    }
    if matches!(target, None | Some("all" | "vscode")) {
        if let Some(home) = dirs::home_dir() {
            let path = home.join(".gemini/jetski-standalone-oauth-token");
            if path.is_file() {
                sources.push(("VS Code plugin session".into(), fs::read_to_string(path).map_err(|_| "read_failed".into()).and_then(|payload| integration::parse_keyring_payload(&payload))));
            }
        }
    }
    if matches!(target, None | Some("all")) {
        if let Some(home) = dirs::home_dir() { sources.extend(discover_v1_sources(&home.join(".antigravity-agent"))); }
    }
    for path in db::get_all_candidate_db_paths(target) {
        if path.is_file() {
            sources.push((path.to_string_lossy().into_owned(), extract_oauth_state_from_file(&path)));
        }
    }
    sources
}

fn discover_v1_sources(dir: &std::path::Path) -> Vec<(String, Result<ImportedOAuthState, String>)> {
    let mut sources = Vec::new();
    for name in ["antigravity_accounts.json", "accounts.json"] {
        let index = dir.join(name);
        if !index.is_file() { continue; }
        let parsed = fs::read_to_string(&index).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok());
        let Some(parsed) = parsed else { sources.push(("Legacy account index".into(), Err("read_failed".into()))); continue; };
        let entries = parsed.get("accounts").and_then(Value::as_object).or_else(|| parsed.as_object());
        if let Some(entries) = entries {
            for entry in entries.values().filter(|entry| entry.is_object()) {
                let file = entry.get("backup_file").or_else(|| entry.get("data_file")).and_then(Value::as_str);
                let Some(file) = file else { sources.push(("Legacy account backup".into(), Err("missing_path".into()))); continue; };
                let original = PathBuf::from(file);
                let basename = original.file_name().unwrap_or_default();
                let path = [original.clone(), dir.join(basename), dir.join("backups").join(basename), dir.join("accounts").join(basename)].into_iter().find(|path| path.is_file());
                let state = path.ok_or_else(|| "missing_backup".to_string()).and_then(|path| {
                    let text = fs::read_to_string(path).map_err(|_| "read_failed".to_string())?;
                    if let Ok(state) = crate::modules::integration::parse_keyring_payload(&text) { return Ok(state); }
                    let json: Value = serde_json::from_str(&text).map_err(|_| "invalid_backup")?;
                    let encoded = json.get("jetskiStateSync.agentManagerInitState").and_then(Value::as_str).ok_or("missing_credentials")?;
                    let bytes = general_purpose::STANDARD.decode(encoded).map_err(|_| "invalid_backup")?;
                    let oauth = protobuf::find_field(&bytes, 6)?.ok_or("missing_credentials")?;
                    let token = String::from_utf8(protobuf::find_field(&oauth, 3)?.ok_or("missing_credentials")?).map_err(|_| "invalid_backup")?;
                    Ok(ImportedOAuthState { refresh_token: token, is_gcp_tos: true, project_id: None })
                });
                sources.push(("Legacy account backup".into(), state));
            }
        }
    }
    sources
}

async fn import_discovered_sources<F, Fut>(
    sources: Vec<(String, Result<ImportedOAuthState, String>)>,
    mut validate: F,
) -> LocalImportReport
where F: FnMut(ImportedOAuthState) -> Fut, Fut: std::future::Future<Output = Result<Account, String>> {
    let mut report = LocalImportReport { accounts: Vec::new(), sources: Vec::new() };
    let mut imported_tokens = std::collections::HashSet::new();
    for (source, state) in sources {
        let status = match state {
            Err(_) => "unavailable",
            Ok(state) if state.refresh_token.trim().is_empty() => "missing_credentials",
            Ok(state) if imported_tokens.contains(&state.refresh_token) => "duplicate",
            Ok(state) => {
                let refresh_token = state.refresh_token.clone();
                match validate(state).await {
                    Ok(account) => {
                        imported_tokens.insert(refresh_token);
                        if !report.accounts.iter().any(|saved| saved.id == account.id) { report.accounts.push(account); }
                        "imported"
                    }
                    Err(_) => "validation_failed",
                }
            }
        };
        // Never put OAuth responses or credentials in diagnostics.
        report.sources.push(LocalImportSource { source, status: status.into() });
    }
    report
}

async fn validate_local_state(state: ImportedOAuthState) -> Result<Account, String> {
    let response = crate::modules::oauth::refresh_access_token(&state.refresh_token, None).await?;
    let user = crate::modules::oauth::get_user_info(&response.access_token).await?;
    if user.email.trim().is_empty() { return Err("Account identity unavailable.".into()); }
    let token = TokenData::new(response.access_token, state.refresh_token, response.expires_in,
        Some(user.email.clone()), state.project_id, None, state.is_gcp_tos, response.id_token)
        .with_oauth_client_key(response.oauth_client_key);
    account::upsert_account(user.email, user.name, token)
}

/// Each client is discovered independently; one failed source cannot hide another.
pub async fn import_local_accounts_report(target: Option<&str>) -> Result<LocalImportReport, String> {
    let target = target.map(str::to_owned);
    let sources = tokio::task::spawn_blocking(move || discover_local_sources(target.as_deref()))
        .await.map_err(|_| "Local account discovery failed.".to_string())?;
    Ok(import_discovered_sources(sources, validate_local_state).await)
}

pub async fn import_all_local_accounts(target: Option<&str>) -> Result<Vec<Account>, String> {
    let report = import_local_accounts_report(target).await?;
    if report.accounts.is_empty() { return Err("No account imported. Check client login state, database location and system proxy; expired credentials require signing in again.".into()); }
    Ok(report.accounts)
}

/// Import only the state observed by synchronization, never the first account
/// returned by a separate all-sources scan.
pub(crate) async fn import_observed_oauth_state(
    state: ImportedOAuthState,
) -> Result<Account, String> {
    use crate::modules::oauth;
    let identity = CurrentIdentity::Credentials(state.clone());
    if let Some(account) = account::list_accounts()?
        .into_iter()
        .find(|account| identity.matches(account))
    {
        return Ok(account);
    }
    let response = oauth::refresh_access_token(&state.refresh_token, None).await?;
    let user = oauth::get_user_info(&response.access_token).await?;
    let token = TokenData::new(
        response.access_token,
        state.refresh_token,
        response.expires_in,
        Some(user.email.clone()),
        state.project_id,
        None,
        state.is_gcp_tos,
        response.id_token,
    )
    .with_oauth_client_key(response.oauth_client_key);
    account::upsert_account(user.email, user.name, token)
}

fn extract_enterprise_project_id_from_conn(
    conn: &rusqlite::Connection,
) -> Result<Option<String>, String> {
    let entry_b64: Option<String> = conn
        .query_row(
            "SELECT value FROM ItemTable WHERE key = ?",
            ["antigravityUnifiedStateSync.enterprisePreferences"],
            |row| row.get(0),
        )
        .ok();

    let Some(entry_b64) = entry_b64 else {
        return Ok(None);
    };

    let (sentinel_key, payload) = protobuf::decode_unified_state_entry(&entry_b64)?;
    if sentinel_key != "enterpriseGcpProjectId" {
        return Ok(None);
    }

    let Some(project_bytes) = protobuf::find_field(&payload, 3)? else {
        return Ok(None);
    };

    let project_id = String::from_utf8(project_bytes)
        .map_err(|_| "enterpriseGcpProjectId is not UTF-8 encoded".to_string())?;
    if project_id.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(project_id))
    }
}

pub(crate) fn extract_oauth_state_from_file(db_path: &PathBuf) -> Result<ImportedOAuthState, String> {
    use base64::{engine::general_purpose, Engine as _};

    if !db_path.exists() {
        return Err(format!("Database file not found: {:?}", db_path));
    }

    // Connect to database
    let conn = rusqlite::Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("Failed to open database: {}", e))?;

    // 1. 尝试新版格式 (>= 1.16.5)
    // 键: antigravityUnifiedStateSync.oauthToken
    // 结构: Outer(F1) -> Inner(F2) -> Inner2(F1) -> Base64 -> OAuthInfo
    let new_format_data: Option<String> = conn
        .query_row(
            "SELECT value FROM ItemTable WHERE key = ?",
            ["antigravityUnifiedStateSync.oauthToken"],
            |row| row.get(0),
        )
        .ok();

    if let Some(outer_b64) = new_format_data {
        crate::modules::logger::log_info(
            "Detected new format database (antigravityUnifiedStateSync.oauthToken)",
        );
        let parsed = (|| -> Result<ImportedOAuthState, String> {
            let (sentinel_key, oauth_info_blob) = protobuf::decode_unified_state_entry(&outer_b64)?;
            if sentinel_key != "oauthTokenInfoSentinelKey" {
                return Err("Unexpected OAuth sentinel key.".into());
            }

            // 解析 OAuthInfo (Field 3) -> Refresh Token
            let refresh_bytes = protobuf::find_field(&oauth_info_blob, 3)
                .map_err(|e| format!("Parsing OAuthInfo Field 3 failed: {}", e))?
                .ok_or("Refresh Token not found in OAuthInfo (Field 3)")?;

            let refresh_token = String::from_utf8(refresh_bytes)
                .map_err(|_| "Refresh Token is not UTF-8 encoded".to_string())?;
            let is_gcp_tos = protobuf::find_varint_field(&oauth_info_blob, 6)?.unwrap_or(1) != 0;
            let project_id = extract_enterprise_project_id_from_conn(&conn)?;

            Ok(ImportedOAuthState {
                refresh_token,
                is_gcp_tos,
                project_id,
            })
        })();
        if let Ok(state) = parsed { return Ok(state); }
    }

    // 2. 尝试旧版格式 (< 1.16.5)
    crate::modules::logger::log_info(
        "Falling back to old format database (jetskiStateSync.agentManagerInitState)",
    );
    let current_data: String = conn
        .query_row(
            "SELECT value FROM ItemTable WHERE key = ?",
            ["jetskiStateSync.agentManagerInitState"],
            |row| row.get(0),
        )
        .map_err(|_| "Login state data not found in either format".to_string())?;

    // Base64 decode
    let blob = general_purpose::STANDARD
        .decode(&current_data)
        .map_err(|e| format!("Base64 decoding failed: {}", e))?;

    // 1. Find oauthTokenInfo (Field 6)
    let oauth_data = protobuf::find_field(&blob, 6)
        .map_err(|e| format!("Protobuf parsing failed: {}", e))?
        .ok_or("OAuth data not found (Field 6)")?;

    // 2. Extract refresh_token (Field 3)
    let refresh_bytes = protobuf::find_field(&oauth_data, 3)
        .map_err(|e| format!("OAuth data parsing failed: {}", e))?
        .ok_or("Refresh Token not included in data (Field 3)")?;

    let refresh_token = String::from_utf8(refresh_bytes)
        .map_err(|_| "Refresh Token is not UTF-8 encoded".to_string())?;

    Ok(ImportedOAuthState {
        refresh_token,
        is_gcp_tos: true,
        project_id: extract_enterprise_project_id_from_conn(&conn)?,
    })
}

/// Get current Refresh Token from System Keyring or candidate databases
pub fn get_refresh_token_from_db(target_ide: Option<&str>) -> Result<String, String> {
    get_oauth_state_from_db(target_ide).map(|state| state.refresh_token)
}

fn get_oauth_state_from_db(target_ide: Option<&str>) -> Result<ImportedOAuthState, String> {
    if target_ide == Some("agy")
        || (target_ide.is_none()
            && crate::modules::process::get_antigravity_executable_path(target_ide).is_none())
    {
        if let Ok(state) = integration::read_cli_credentials() {
            return Ok(state);
        }
    }
    use crate::modules::integration;

    if matches!(target_ide, None | Some("app" | "app_cli" | "agy" | "all")) {
        if let Ok(oauth_state) = integration::read_from_system_keyring() {
            return Ok(oauth_state);
        }
    }

    let candidate_paths = db::get_all_candidate_db_paths(target_ide);
    for db_path in candidate_paths {
        if db_path.exists() {
            if let Ok(state) = extract_oauth_state_from_file(&db_path) {
                return Ok(state);
            }
        }
    }

    Err("Login state data not found in keyring or any database format".to_string())
}

#[cfg(test)]
mod current_identity_tests {
    use super::*;

    fn fixture(id: &str, email: &str) -> Account {
        Account::new(id.into(), email.into(), TokenData::new(
            format!("fixture-access-{id}"), format!("fixture-refresh-{id}"), 3600,
            Some(email.into()), None, None, false, None,
        ))
    }

    #[test]
    fn running_second_account_wins_over_first_stored_account() {
        let accounts = [fixture("first", "first@example.test"), fixture("second", "second@example.test")];
        let live = resolve_current_identity(
            || Ok(Some("SECOND@example.test".into())),
            || panic!("A running App must not consult the stale keyring"),
        ).unwrap();
        assert_eq!(accounts.iter().find(|a| live.matches(a)).unwrap().id, "second");
        // A shared keyring that still contains account one cannot select it
        // when the observed identity came from the running App.
        assert!(!live.matches(&accounts[0]));
    }

    #[test]
    fn credential_sync_selects_the_exact_observation_not_scan_order() {
        let accounts = [fixture("first", "first@example.test"), fixture("second", "second@example.test")];
        let observed = resolve_current_identity(|| Ok(None), || Ok(ImportedOAuthState {
            refresh_token: "fixture-refresh-second".into(), is_gcp_tos: false, project_id: None,
        })).unwrap();
        assert_eq!(accounts.iter().find(|a| observed.matches(a)).unwrap().id, "second");
        let unknown = CurrentIdentity::AppEmail("unknown@example.test".into());
        assert!(accounts.iter().find(|a| unknown.matches(a)).is_none());
    }

    #[test]
    fn unverifiable_running_app_never_falls_back_to_shared_credentials() {
        let result = resolve_current_identity(
            || Err("running_app_identity_unavailable".into()),
            || panic!("A failed live read must not overwrite the record from the keyring"),
        );
        assert_eq!(result.err().as_deref(), Some("running_app_identity_unavailable"));
    }
}

#[cfg(test)]
mod import_report_tests {
    use super::*;

    #[tokio::test]
    async fn partial_failure_is_reported_and_does_not_hide_another_client_or_retry() {
        let state = |token: &str| Ok(ImportedOAuthState { refresh_token: token.into(), is_gcp_tos: false, project_id: None });
        let mut attempts = 0;
        let report = import_discovered_sources(vec![
            ("App".into(), state("fixture-A")), ("CLI".into(), state("fixture-A")),
            ("IDE".into(), state("fixture-B")), ("IDE duplicate".into(), state("fixture-B")),
            ("Unreadable".into(), Err("secret-bearing error must not be shown".into())),
        ], |state| {
            attempts += 1;
            let attempt = attempts;
            async move {
                if attempt == 1 { return Err("synthetic network error".into()); }
                Ok(Account::new(state.refresh_token.clone(), "fixture@example.test".into(), TokenData::new("fixture-access".into(), state.refresh_token, 3600, None, None, None, false, None)))
            }
        }).await;
        assert_eq!(report.accounts.len(), 2);
        assert_eq!(report.sources.iter().map(|source| source.status.as_str()).collect::<Vec<_>>(), ["validation_failed", "imported", "imported", "duplicate", "unavailable"]);
        assert!(report.sources.iter().all(|source| !source.status.contains("secret")));
    }

    #[test]
    fn corrupt_new_format_falls_back_to_valid_legacy_state_without_writes() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.vscdb");
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute("CREATE TABLE ItemTable (key TEXT PRIMARY KEY, value TEXT)", []).unwrap();
            let oauth = protobuf::create_oauth_info("fixture-access", "fixture-refresh", 123, false, None, None);
            let legacy = general_purpose::STANDARD.encode(protobuf::encode_len_delim_field(6, &oauth));
            conn.execute("INSERT INTO ItemTable VALUES (?1, ?2)", ["jetskiStateSync.agentManagerInitState", &legacy]).unwrap();
            conn.execute("INSERT INTO ItemTable VALUES (?1, ?2)", ["antigravityUnifiedStateSync.oauthToken", "corrupt"]).unwrap();
        }
        let original = fs::read(&path).unwrap();
        assert_eq!(extract_oauth_state_from_file(&path).unwrap().refresh_token, "fixture-refresh");
        assert_eq!(fs::read(&path).unwrap(), original);
    }
}
