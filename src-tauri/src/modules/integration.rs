use crate::modules::{cli_credentials, db, device, process};
use std::fs;
#[cfg(target_os = "macos")]
use std::process::Command;

pub trait SystemIntegration: Send + Sync {
    /// 当切换账号时执行的系统层操作（如杀进程、写入文件、注入数据库）
    async fn on_account_switch(
        &self,
        account: &crate::models::Account,
        target_ide: Option<&str>,
    ) -> Result<(), String>;

    /// The CLI overrides only launch I/O; GUI launch behavior stays unchanged.
    fn start_application(&self, target_ide: Option<&str>) -> Result<(), String> {
        process::start_antigravity(target_ide)
    }

    /// 更新系统托盘（如果适用）
    fn update_tray(&self);
}

/// 桌面版实现：包含完整的进程控制 and UI 同步
pub struct DesktopIntegration {
    pub app_handle: tauri::AppHandle,
}

impl SystemIntegration for DesktopIntegration {
    async fn on_account_switch(
        &self,
        account: &crate::models::Account,
        target_ide: Option<&str>,
    ) -> Result<(), String> {
        let integration = DesktopIntegration {
            app_handle: self.app_handle.clone(),
        };
        let account = account.clone();
        let target = target_ide.map(str::to_owned);
        tokio::task::spawn_blocking(move || {
            DesktopIntegration::switch_sync(&integration, &account, target.as_deref())
        })
        .await
        .map_err(|_| "Account switch worker failed.".to_string())?
    }

    fn update_tray(&self) {
        let _ = crate::modules::tray::update_tray_menus(&self.app_handle);
    }
}

impl DesktopIntegration {
    /// Shared credential/process operations; callbacks may be headless (CLI).
    pub(crate) fn switch_sync(
        integration: &impl SystemIntegration,
        account: &crate::models::Account,
        target_ide: Option<&str>,
    ) -> Result<(), String> {
        crate::modules::logger::log_info(&format!(
            "[Desktop] Executing system switch for: {} (target_ide: {:?})",
            account.email, target_ide
        ));

        #[cfg(target_os = "linux")]
        if process::get_antigravity_executable_path(target_ide).is_none()
            && !(matches!(target_ide, Some("app" | "agy")) && cli_session_path()?.is_some()) {
            return Err("Cannot find Antigravity. Set its executable path in Settings, or install and initialize agy before switching accounts.".into());
        }

        // 1. 智能检测运行中的进程，并安全关闭对应客户端
        let is_ide_mode = target_ide == Some("ide");
        let is_app_cli_mode = target_ide == Some("app");
        let is_vscode_mode = target_ide == Some("vscode");
        let candidate_dbs = if matches!(target_ide, None | Some("all")) {
            db::get_all_candidate_db_paths(None)
        } else {
            db::get_all_candidate_db_paths(target_ide)
        };
        let candidate_dbs: Vec<_> = candidate_dbs.into_iter().filter(|path| path.is_file()).collect();
        if is_ide_mode || is_app_cli_mode {
            let other = db::get_all_candidate_db_paths(Some(if is_ide_mode { "app" } else { "ide" }));
            reject_shared_databases(&candidate_dbs, &other)?;
        }
        if (is_ide_mode || is_vscode_mode) && candidate_dbs.is_empty() {
            return Err("No initialized database found for the selected client. Open and sign in to it first, or configure its executable path in Settings.".into());
        }
        // A standalone agy Hub can outlive the App and retain its old session.
        // Observe it before closing anything; unknown ownership must block writes.
        let hubs = if is_ide_mode || is_vscode_mode { vec![] } else {
            crate::modules::app_hub::snapshot().map_err(|_| "Cannot verify the Antigravity App Hub. Finish running agy commands before retrying.")?
        };
        let app_running = if is_ide_mode || is_vscode_mode { false } else { process::is_antigravity_running(None) };
        let ide_running = if is_app_cli_mode || is_vscode_mode { false } else { process::is_antigravity_running(Some("ide")) };

        if is_vscode_mode {
            // VS Code 插件模式：不退出 VS Code 宿主，直接进行凭据热更新
        } else if is_ide_mode {
            if ide_running {
                process::close_antigravity(20, Some("ide"))?;
            }
        } else if is_app_cli_mode {
            if app_running {
                process::close_antigravity(20, None)?;
            }
        } else {
            if app_running {
                process::close_antigravity(20, None)?;
            }
            if ide_running {
                process::close_antigravity(20, Some("ide"))?;
            }
        }

        update_client_credentials(
            || crate::modules::app_hub::close(&hubs, false).map_err(|_| "Cannot stop the Antigravity App Hub; credentials were not changed.".into()),
            || write_client_storage(account, target_ide, &candidate_dbs, dirs::home_dir().as_deref(), || write_to_system_keyring(account, false)),
            || crate::modules::app_hub::restart(&hubs),
        )?;

        // 3. 智能联动重启：刚才开着哪个，就重新拉起哪个（VS Code 插件免重启）
        if is_vscode_mode {
            // 免重启宿主编辑器
        } else if is_ide_mode {
            if ide_running {
                integration.start_application(Some("ide"))?;
            }
        } else if is_app_cli_mode {
            if app_running {
                integration.start_application(None)?;
            }
        } else {
            if app_running {
                integration.start_application(None)?;
            }
            if ide_running {
                integration.start_application(Some("ide"))?;
            }
        }

        // 4. 更新托盘
        integration.update_tray();

        Ok(())
    }
}

/// Restart the observed Hub even after a failed write. Start it before the App
/// can reconnect, so its cached credentials cannot win a race with the switch.
fn update_client_credentials(
    stop: impl FnOnce() -> Result<(), String>,
    write: impl FnOnce() -> Result<(), String>,
    restart: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    stop()?;
    let written = write();
    let restarted = restart();
    written.and(restarted)
}

/// Credential writes are scoped separately from process control for isolated verification.
fn write_client_storage(
    account: &crate::models::Account,
    target: Option<&str>,
    databases: &[std::path::PathBuf],
    home: Option<&std::path::Path>,
    write_app_and_cli: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if matches!(target, None | Some("app" | "all")) { write_app_and_cli()?; }
    if matches!(target, None | Some("vscode" | "all")) {
        if let Some(home) = home {
            let gemini = home.join(".gemini");
            if gemini.is_dir() {
                cli_credentials::write_session(&gemini.join("jetski-standalone-oauth-token"), &cli_credentials::payload(&account.token)?)?;
            }
        }
    }
    for path in databases {
        let storage = path.with_file_name("storage.json");
        if storage.is_file() {
            if let Some(profile) = &account.device_profile { device::write_profile(&storage, profile)?; }
        }
        fs::copy(path, path.with_extension("vscdb.backup")).map_err(|e| format!("Cannot back up client database: {e}"))?;
        db::inject_token(path, &account.token.access_token, &account.token.refresh_token, account.token.expiry_timestamp,
            &account.email, account.token.is_gcp_tos, account.token.project_id.as_deref(), account.token.id_token.as_deref(), account.token.oauth_client_key.as_deref())?;
    }
    Ok(())
}

fn reject_shared_databases(selected: &[std::path::PathBuf], other: &[std::path::PathBuf]) -> Result<(), String> {
    if selected.iter().any(|path| other.contains(path)) {
        return Err("App and IDE share a user-data directory. Configure separate client data directories before switching independently.".into());
    }
    Ok(())
}

fn cli_session_path() -> Result<Option<std::path::PathBuf>, String> {
    let home = dirs::home_dir().ok_or("Failed to resolve user home directory")?;
    cli_credentials::session_path(&home)
}

pub fn read_cli_credentials() -> Result<crate::modules::migration::ImportedOAuthState, String> {
    let path = cli_session_path()?.ok_or("No initialized agy CLI session found.")?;
    let payload = std::fs::read_to_string(path)
        .map_err(|error| format!("Cannot read agy session: {error}"))?;
    parse_keyring_payload(&payload)
}

/// Sync the APP's platform credential store and an existing native agy session.
pub(crate) fn write_to_system_keyring(
    account: &crate::models::Account,
    cli_only: bool,
) -> Result<(), String> {
    let payload_json = cli_credentials::payload(&account.token)?;
    let cli_path = cli_session_path()?;

    // Explicit CLI switches do not need or modify the APP credential store.
    // Require initialization so a missing session cannot be reported as success.
    if cli_only {
        return cli_credentials::sync_session(cli_path.as_deref(), &payload_json, true);
    }

    crate::modules::logger::log_info(&format!(
        "[Desktop] Writing token to system credential store for: {}",
        account.email
    ));

    // 2. 跨平台凭据注入
    #[cfg(target_os = "macos")]
    {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let encoded_payload = STANDARD.encode(&payload_json);
        let full_keyring_value = format!("go-keyring-base64:{}", encoded_payload);

        // 2.1 macOS Keychain Access
        // 删除旧的
        let _ = Command::new("security")
            .args([
                "delete-generic-password",
                "-s",
                "gemini",
                "-a",
                "antigravity",
            ])
            .output();

        // 写入新的 (-A 参数允许所有本地应用免密码、无感直接读取凭据)
        let output = Command::new("security")
            .args([
                "add-generic-password",
                "-s",
                "gemini",
                "-a",
                "antigravity",
                "-w",
                &full_keyring_value,
                "-A",
            ])
            .output()
            .map_err(|e| format!("Failed to execute security command: {}", e))?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            return Err(format!("macOS security command failed: {}", err_msg.trim()));
        }
    }

    #[cfg(target_os = "windows")]
    {
        // 2.2 Windows Credential Manager direct Win32 API calls to write raw UTF-8 bytes
        use std::os::windows::ffi::OsStrExt;
        use std::ptr;

        #[repr(C)]
        struct FILETIME {
            dw_low_date_time: u32,
            dw_high_date_time: u32,
        }

        #[repr(C)]
        struct CREDENTIALW {
            flags: u32,
            cred_type: u32,
            target_name: *const u16,
            comment: *const u16,
            last_written: FILETIME,
            credential_blob_size: u32,
            credential_blob: *const u8,
            persist: u32,
            attribute_count: u32,
            attributes: *const std::ffi::c_void,
            target_alias: *const u16,
            user_name: *const u16,
        }

        #[link(name = "advapi32")]
        extern "system" {
            fn CredWriteW(credential: *const CREDENTIALW, flags: u32) -> i32;
            fn CredDeleteW(target_name: *const u16, type_: u32, flags: u32) -> i32;
        }

        let target = "gemini:antigravity";
        let user = "antigravity";
        let secret = payload_json.as_bytes();

        let target_wide: Vec<u16> = std::ffi::OsStr::new(target)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let user_wide: Vec<u16> = std::ffi::OsStr::new(user)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let cred = CREDENTIALW {
            flags: 0,
            cred_type: 1, // CRED_TYPE_GENERIC
            target_name: target_wide.as_ptr(),
            comment: ptr::null(),
            last_written: FILETIME {
                dw_low_date_time: 0,
                dw_high_date_time: 0,
            },
            credential_blob_size: secret.len() as u32,
            credential_blob: secret.as_ptr(),
            persist: 2, // CRED_PERSIST_LOCAL_MACHINE
            attribute_count: 0,
            attributes: ptr::null(),
            target_alias: ptr::null(),
            user_name: user_wide.as_ptr(),
        };

        unsafe {
            // Delete first to ensure we write clean
            let _ = CredDeleteW(target_wide.as_ptr(), 1, 0);

            let res = CredWriteW(&cred, 0);
            if res == 0 {
                let err = std::io::Error::last_os_error();
                return Err(format!("Windows CredWriteW failed: {}", err));
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(path) = cli_path {
            let cli_payload = payload_json.clone();
            crate::modules::linux_credentials::write_with_commit(&payload_json, move || {
                cli_credentials::write_session(&path, &cli_payload)
            })?;
        } else {
            crate::modules::linux_credentials::write(&payload_json)?;
        }
    }

    #[cfg(not(target_os = "linux"))]
    cli_credentials::sync_session(cli_path.as_deref(), &payload_json, false).map_err(|error| {
        format!("System credential store was updated, but agy session sync failed: {error}")
    })?;

    crate::modules::logger::log_info(
        "[Desktop] Successfully wrote token to system credential store.",
    );

    Ok(())
}

/// 辅助方法：从宿主操作系统的 Keychain/Credentials Manager 读取 Token
pub fn read_from_system_keyring() -> Result<crate::modules::migration::ImportedOAuthState, String> {
    #[cfg(target_os = "macos")]
    {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let output = Command::new("security")
            .args([
                "find-generic-password",
                "-s",
                "gemini",
                "-a",
                "antigravity",
                "-w",
            ])
            .output()
            .map_err(|e| format!("Failed to execute security command: {}", e))?;

        if !output.status.success() {
            return Err("No credential found in macOS Keychain".to_string());
        }

        let secret_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let payload_str = if secret_str.starts_with("go-keyring-base64:") {
            let b64_part = &secret_str["go-keyring-base64:".len()..];
            let decoded = STANDARD
                .decode(b64_part)
                .map_err(|e| format!("Base64 decode failed: {}", e))?;
            String::from_utf8(decoded).map_err(|e| format!("UTF-8 decode failed: {}", e))?
        } else {
            secret_str
        };

        return parse_keyring_payload(&payload_str);
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        use std::ptr;

        #[repr(C)]
        struct FILETIME {
            dw_low_date_time: u32,
            dw_high_date_time: u32,
        }

        #[repr(C)]
        struct CREDENTIALW {
            flags: u32,
            cred_type: u32,
            target_name: *const u16,
            comment: *const u16,
            last_written: FILETIME,
            credential_blob_size: u32,
            credential_blob: *mut u8,
            persist: u32,
            attribute_count: u32,
            attributes: *const std::ffi::c_void,
            target_alias: *const u16,
            user_name: *const u16,
        }

        #[link(name = "advapi32")]
        extern "system" {
            fn CredReadW(
                target_name: *const u16,
                type_: u32,
                flags: u32,
                credential: *mut *mut CREDENTIALW,
            ) -> i32;
            fn CredFree(buffer: *mut std::ffi::c_void);
        }

        let target = "gemini:antigravity";
        let target_wide: Vec<u16> = std::ffi::OsStr::new(target)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut cred_ptr: *mut CREDENTIALW = ptr::null_mut();
        unsafe {
            let res = CredReadW(target_wide.as_ptr(), 1, 0, &mut cred_ptr);
            if res == 0 || cred_ptr.is_null() {
                return Err("No credential found in Windows Credential Manager".to_string());
            }

            let cred = &*cred_ptr;
            let blob = std::slice::from_raw_parts(
                cred.credential_blob,
                cred.credential_blob_size as usize,
            );
            let payload_str = String::from_utf8_lossy(blob).to_string();
            CredFree(cred_ptr as *mut std::ffi::c_void);

            return parse_keyring_payload(&payload_str);
        }
    }

    #[cfg(target_os = "linux")]
    {
        return parse_keyring_payload(&crate::modules::linux_credentials::read()?);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Err("Keyring not supported on this operating system".to_string())
    }
}

pub(crate) fn parse_keyring_payload(
    payload_str: &str,
) -> Result<crate::modules::migration::ImportedOAuthState, String> {
    let json: serde_json::Value = serde_json::from_str(payload_str)
        .map_err(|e| format!("Failed to parse keyring payload JSON: {}", e))?;

    let refresh_token = json
        .get("token")
        .and_then(|t| t.get("refresh_token"))
        .and_then(|v| v.as_str())
        .or_else(|| json.get("refresh_token").and_then(|v| v.as_str()))
        .ok_or_else(|| "Refresh Token not found in keyring payload".to_string())?
        .to_string();

    Ok(crate::modules::migration::ImportedOAuthState {
        refresh_token,
        is_gcp_tos: true,
        project_id: None,
    })
}

/// Desktop integration manager used by account and OAuth services.
#[derive(Clone)]
pub struct SystemManager {
    pub app_handle: tauri::AppHandle,
}

impl SystemManager {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }

    pub fn update_tray(&self) {
        DesktopIntegration {
            app_handle: self.app_handle.clone(),
        }
        .update_tray();
    }
}

impl SystemIntegration for SystemManager {
    async fn on_account_switch(
        &self,
        account: &crate::models::Account,
        target_ide: Option<&str>,
    ) -> Result<(), String> {
        DesktopIntegration {
            app_handle: self.app_handle.clone(),
        }
        .on_account_switch(account, target_ide)
        .await
    }

    fn update_tray(&self) {
        SystemManager::update_tray(self);
    }
}

#[cfg(all(test, target_os = "linux"))]
mod linux_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    #[ignore = "requires a disposable D-Bus/keyring session"]
    fn isolated_linux_app_and_cli_sync() {
        assert_eq!(
            std::env::var("ANTIGRAVITY_TEST_SECRET_SERVICE").as_deref(),
            Ok("isolated")
        );
        let home = dirs::home_dir().unwrap();
        std::fs::create_dir_all(home.join(".gemini/antigravity-cli")).unwrap();
        let path = cli_session_path().unwrap().unwrap();
        let mut account = crate::models::Account::new(
            "fixture".into(),
            "fixture@example.invalid".into(),
            crate::models::TokenData::new(
                "fixture-access".into(),
                "fixture-app".into(),
                3600,
                None,
                None,
                None,
                true,
                Some("fixture-id".into()),
            ),
        );
        write_to_system_keyring(&account, false).unwrap();
        let app_payload = crate::modules::linux_credentials::read().unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), app_payload);

        // An explicit CLI switch must leave the APP's active credential alone.
        account.token.refresh_token = "fixture-cli-only".into();
        write_to_system_keyring(&account, true).unwrap();
        assert_eq!(
            read_cli_credentials().unwrap().refresh_token,
            "fixture-cli-only"
        );
        assert_eq!(
            crate::modules::linux_credentials::read().unwrap(),
            app_payload
        );

        // Force the real APP+CLI commit callback to fail and verify rollback.
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        let failure = write_to_system_keyring(&account, false).unwrap_err();
        assert!(failure.contains("Previous credentials were restored"));
        assert_eq!(
            crate::modules::linux_credentials::read().unwrap(),
            app_payload
        );
        assert!(path.is_dir());
        std::fs::remove_dir(&path).unwrap();
    }

    #[test]
    #[ignore = "requires a disposable HOME; run with --test-threads=1"]
    fn isolated_linux_cli_switch() {
        assert_eq!(
            std::env::var("ANTIGRAVITY_TEST_SECRET_SERVICE").as_deref(),
            Ok("isolated")
        );
        let home = dirs::home_dir().unwrap();
        let bin = home.join(".local/bin");
        let session = home.join(".gemini/antigravity-cli");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&session).unwrap();
        let executable = bin.join("agy");
        std::fs::write(&executable, b"#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let unrelated = home.join(".gemini/oauth_creds.json");
        let unrelated_accounts = home.join(".gemini/google_accounts.json");
        std::fs::write(&unrelated, b"unrelated-gemini-fixture").unwrap();
        std::fs::write(&unrelated_accounts, b"unrelated-accounts-fixture").unwrap();
        let mut account = crate::models::Account::new(
            "fixture".into(),
            "fixture@example.invalid".into(),
            crate::models::TokenData::new(
                "fixture-access".into(),
                "fixture-old".into(),
                3600,
                None,
                None,
                None,
                true,
                None,
            ),
        );
        write_to_system_keyring(&account, true).unwrap();
        assert_eq!(read_cli_credentials().unwrap().refresh_token, "fixture-old");
        account.token.refresh_token = "fixture-new".into();
        write_to_system_keyring(&account, true).unwrap();
        assert_eq!(read_cli_credentials().unwrap().refresh_token, "fixture-new");
        let metadata = std::fs::metadata(session.join("antigravity-oauth-token")).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        assert_eq!(
            std::fs::read(unrelated).unwrap(),
            b"unrelated-gemini-fixture"
        );
        assert_eq!(
            std::fs::read(unrelated_accounts).unwrap(),
            b"unrelated-accounts-fixture"
        );
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;
    use crate::models::{Account, TokenData};

    #[test]
    fn manual_switch_stops_old_hub_before_writing_and_recovers_on_failure() {
        use std::cell::RefCell;
        for fail in ["", "stop", "write", "restart"] {
            let calls = RefCell::new(Vec::new());
            let operation = |name| {
                calls.borrow_mut().push(name);
                if fail == name { Err(format!("fixture-{name}")) } else { Ok(()) }
            };
            let result = update_client_credentials(|| operation("stop"), || operation("write"), || operation("restart"));
            assert_eq!(result.is_ok(), fail.is_empty());
            assert_eq!(*calls.borrow(), if fail == "stop" { vec!["stop"] } else { vec!["stop", "write", "restart"] });
            if !fail.is_empty() { assert_eq!(result.unwrap_err(), format!("fixture-{fail}")); }
        }
    }

    #[test]
    fn shared_data_directories_are_rejected_before_switching() {
        let app = std::path::PathBuf::from("/synthetic/app/state.vscdb");
        let ide = std::path::PathBuf::from("/synthetic/ide/state.vscdb");
        assert!(reject_shared_databases(&[app.clone()], &[app.clone()]).is_err());
        assert!(reject_shared_databases(&[app], &[ide]).is_ok());
        let app_folders = db::data_folder_names(Some("app"));
        let ide_folders = db::data_folder_names(Some("ide"));
        assert!(app_folders.iter().all(|folder| !ide_folders.contains(folder)));
        assert!(ide_folders.contains(&"Antigravity_IDE"));
        assert!(ide_folders.contains(&"antigravity-ide"));
    }

    fn database(root: &std::path::Path, name: &str) -> std::path::PathBuf {
        let dir = root.join(name);
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("storage.json"), b"{}").unwrap();
        let path = dir.join("state.vscdb");
        rusqlite::Connection::open(&path).unwrap().execute("CREATE TABLE ItemTable (key TEXT PRIMARY KEY, value TEXT)", []).unwrap();
        path
    }

    #[test]
    fn production_storage_writes_keep_app_cli_and_ide_independent() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path();
        fs::create_dir(home.join(".gemini")).unwrap();
        let plugin = home.join(".gemini/jetski-standalone-oauth-token");
        fs::write(&plugin, "fixture-plugin-original").unwrap();
        let app = database(home, "App");
        let ide = database(home, "IDE");
        let original_app = fs::read(&app).unwrap();
        let mut account = Account::new("B".into(), "b@example.test".into(), TokenData::new("fixture-access-B".into(), "fixture-refresh-B".into(), 3600, None, None, None, false, None));
        account.device_profile = Some(device::generate_profile());
        write_client_storage(&account, Some("ide"), &[ide.clone()], Some(home), || panic!("IDE must never write App / CLI credentials")).unwrap();
        assert_eq!(fs::read(&app).unwrap(), original_app);
        assert_eq!(fs::read_to_string(&plugin).unwrap(), "fixture-plugin-original");
        assert!(crate::modules::migration::extract_oauth_state_from_file(&ide).unwrap().refresh_token == account.token.refresh_token);
        let original_ide = fs::read(&ide).unwrap();
        account.token.refresh_token = "fixture-refresh-A".into();
        let mock_keyring = home.join("mock-keyring");
        let cli = home.join("mock-cli");
        write_client_storage(&account, Some("app"), &[app.clone()], Some(home), || {
            let payload = cli_credentials::payload(&account.token)?;
            cli_credentials::write_session(&mock_keyring, &payload)?;
            cli_credentials::write_session(&cli, &payload)
        }).unwrap();
        assert_eq!(fs::read(&ide).unwrap(), original_ide);
        assert_eq!(fs::read(&mock_keyring).unwrap(), fs::read(&cli).unwrap());
        assert_eq!(fs::read_to_string(&plugin).unwrap(), "fixture-plugin-original");
        assert!(crate::modules::migration::extract_oauth_state_from_file(&app).unwrap().refresh_token == account.token.refresh_token);
        let conn = rusqlite::Connection::open(&app).unwrap();
        let machine_id: String = conn.query_row("SELECT value FROM ItemTable WHERE key = 'storage.serviceMachineId'", [], |row| row.get(0)).unwrap();
        assert_eq!(machine_id, account.device_profile.unwrap().dev_device_id);
    }
}
