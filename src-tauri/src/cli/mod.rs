//! agy-switch - Antigravity Tools Lite local CLI. Never initializes Tauri, a logger, or OAuth for reads.
mod output;
mod picker;
mod switch_lock;
mod settings;

use output::{AccountView, Snapshot};
use std::path::{Path, PathBuf};

const HELP: &str = "agy-switch - Antigravity Tools Lite CLI\n\nUsage:\n  agy-switch                         Interactive dashboard / menu (TUI)\n  agy-switch accounts list [--json]\n  agy-switch current [--json]\n  agy-switch quota [ACCOUNT_ID|EMAIL] [--json]\n  agy-switch switch [ACCOUNT_ID|EMAIL] [--target app|ide] [--json]\n  agy-switch stats [--json]\n  agy-switch refresh [ACCOUNT_ID|EMAIL] [--json]\n  agy-switch policy show [--json]\n  agy-switch policy set [OPTIONS] [--json]\n    --enabled true|false --mode wait|stop --strategy priority|round-robin\n    --reserve 1..98 --minimum 2..100 --model all|gemini|claude|MODEL_ID\n    --target app|app-cli|ide|vscode --candidates ID|EMAIL...\n    --clear-candidates\n  agy-switch policy order [ID|EMAIL...] [--json]\n  agy-switch accounts order [ID|EMAIL...] [--json]\n  agy-switch update check [--json]\n  agy-switch --help\n  agy-switch --version\n\nRead commands use local cached data only and never open the GUI or refresh tokens.\n'current' is Tools Lite's recorded account, not a live credential-store check.\n'switch' may refresh tokens, close/restart Antigravity, and update credentials.\nDefault target 'app' synchronizes APP credentials and an initialized agy session.\nThere is no CLI-only target: APP and agy may share the same credential store.\nPolicy edits configure the desktop scheduler; they do not start a CLI daemon.\nUpdate checks contact GitHub but never download or install.\nOrdering requires every account (or selected candidate) exactly once.\nAccounts can be managed interactively via TUI or through the GUI. ABV_DATA_DIR overrides the data directory.\n";

#[derive(Debug, PartialEq)]
enum Command {
    Help,
    Version,
    List,
    Current,
    Quota(Option<String>),
    Stats,
    Refresh(Option<String>),
    Switch { selector: String, target: String },
    InteractiveSwitch { target: String },
    InteractiveDashboard,
    PolicyShow,
    PolicySet(settings::PolicyPatch),
    PolicyOrder(Vec<String>),
    AccountOrder(Vec<String>),
    UpdateCheck,
}

#[derive(Debug)]
pub(crate) struct CliError {
    pub(crate) code: i32,
    pub(crate) message: &'static str,
}

type Result<T> = std::result::Result<T, CliError>;

impl CliError {
    fn usage() -> Self {
        Self {
            code: 2,
            message: "Invalid command or arguments. Run agy-switch --help.",
        }
    }
    pub(crate) fn data(message: &'static str) -> Self {
        Self { code: 1, message }
    }
    fn missing() -> Self {
        Self {
            code: 3,
            message: "No matching account. Run agy-switch accounts list.",
        }
    }
}

fn is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

fn parse(args: &[String], interactive: bool) -> Result<(Command, bool)> {
    let json = args.iter().any(|arg| arg == "--json");
    if args.iter().filter(|arg| arg.as_str() == "--json").count() > 1 {
        return Err(CliError::usage());
    }
    let mut args: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|arg| *arg != "--json")
        .collect();
    if args.first() == Some(&"--cli") {
        args.remove(0);
    }
    if args.first() == Some(&"accounts") {
        args.remove(0);
    }
    let command = match args.as_slice() {
        [] if interactive && !json => Command::InteractiveDashboard,
        [] | ["--help"] | ["-h"] | ["help"] => Command::Help,
        ["--version"] | ["-V"] => Command::Version,
        ["list"] => Command::List,
        ["current"] => Command::Current,
        ["quota"] => Command::Quota(None),
        ["quota", selector] if !selector.starts_with('-') => {
            Command::Quota(Some((*selector).into()))
        }
        ["stats"] => Command::Stats,
        ["refresh"] => Command::Refresh(None),
        ["refresh", selector] if !selector.starts_with('-') => {
            Command::Refresh(Some((*selector).into()))
        }
        ["policy"] | ["policy", "show"] => Command::PolicyShow,
        ["policy", "set", rest @ ..] => Command::PolicySet(settings::parse_patch(rest)?),
        ["policy", "order", rest @ ..] if rest.iter().all(|s| !s.starts_with('-')) => Command::PolicyOrder(rest.iter().map(|s| (*s).into()).collect()),
        ["order", rest @ ..] if rest.iter().all(|s| !s.starts_with('-')) => Command::AccountOrder(rest.iter().map(|s| (*s).into()).collect()),
        ["update", "check"] => Command::UpdateCheck,
        ["switch"] if interactive && !json => Command::InteractiveSwitch {
            target: "app".into(),
        },
        ["switch", "--target", target]
            if interactive && !json && matches!(*target, "app" | "ide") =>
        {
            Command::InteractiveSwitch {
                target: (*target).into(),
            }
        }
        ["switch", selector] if !selector.starts_with('-') => Command::Switch {
            selector: (*selector).into(),
            target: "app".into(),
        },
        ["switch", selector, "--target", target]
            if !selector.starts_with('-') && matches!(*target, "app" | "ide") =>
        {
            Command::Switch {
                selector: (*selector).into(),
                target: (*target).into(),
            }
        }
        _ => return Err(CliError::usage()),
    };
    Ok((command, json))
}

/// `None` preserves normal GUI launch. Called before any app initialization.
pub fn run_if_requested() -> Option<i32> {
    let args: Vec<_> = std::env::args_os().collect();
    let named_cli = args
        .first()
        .and_then(|arg| Path::new(arg).file_stem())
        .is_some_and(|name| name == "agy-switch");
    if !named_cli
        && (args.len() == 1
            || (args.len() == 2
                && (args[1] == "--autostart" || args[1].to_string_lossy().starts_with("-psn_"))))
    {
        return None;
    }
    // Release builds are GUI-subsystem executables on Windows. Attach only in
    // CLI mode, before stdout/stderr are first used; never allocate a new console.
    #[cfg(windows)]
    unsafe {
        #[link(name = "Kernel32")]
        extern "system" {
            fn AttachConsole(process_id: u32) -> i32;
        }
        let _ = AttachConsole(u32::MAX);
    }
    Some(run())
}

/// Console entry point: always selects CLI mode, even if the executable is renamed.
pub fn run() -> i32 {
    let args: Vec<_> = std::env::args_os().collect();
    let json = args.iter().any(|arg| arg == "--json");
    let interactive = is_tty() && !json;
    let strings: std::result::Result<Vec<String>, _> = args
        .into_iter()
        .skip(1)
        .map(|arg| arg.into_string())
        .collect();
    let result = strings
        .map_err(|_| CliError::usage())
        .and_then(|args| parse(&args, interactive))
        .and_then(|(command, json)| execute(command, json));
    match result {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            0
        }
        Err(error) => {
            if json {
                eprintln!(
                    "{}",
                    serde_json::json!({"schema_version": 1, "error": {"code": error.code, "message": error.message}})
                );
            } else {
                eprintln!("agy-switch: {}", error.message);
            }
            error.code
        }
    }
}

pub(crate) fn data_dir() -> Result<PathBuf> {
    if let Some(path) =
        std::env::var_os("ABV_DATA_DIR").filter(|path| !path.to_string_lossy().trim().is_empty())
    {
        return Ok(PathBuf::from(path));
    }
    dirs::home_dir()
        .map(|home| home.join(".antigravity_tools"))
        .ok_or_else(|| CliError::data("Cannot resolve the home directory."))
}

fn execute(command: Command, json: bool) -> Result<String> {
    match command {
        Command::Help => {
            return Ok(if json {
                serde_json::json!({"schema_version": 1, "help": HELP}).to_string()
            } else {
                HELP.into()
            })
        }
        Command::Version => {
            return Ok(if json {
                serde_json::json!({"schema_version": 1, "name": "agy-switch", "version": env!("CARGO_PKG_VERSION")}).to_string()
            } else {
                format!("agy-switch {}", env!("CARGO_PKG_VERSION"))
            })
        }
        Command::PolicyShow => return settings::show_policy(&data_dir()?, json),
        Command::PolicySet(patch) => return settings::set_policy(&data_dir()?, patch, json),
        Command::PolicyOrder(selectors) => return settings::order_policy(&data_dir()?, &selectors, json),
        Command::AccountOrder(selectors) => return settings::order_accounts(&data_dir()?, &selectors, json),
        Command::UpdateCheck => return settings::check_update(json, picker::Lang::current(&data_dir()?)),
        _ => {}
    }
    let snapshot = Snapshot::read(&data_dir()?)?;
    match command {
        Command::List => {
            if json {
                return Ok(serde_json::json!({"schema_version": 1, "accounts": snapshot.accounts, "current_target": snapshot.current_target}).to_string());
            }
            if snapshot.accounts.is_empty() {
                return Ok("No saved accounts. Add an account in the Tools Lite GUI.".into());
            }
            Ok(snapshot
                .accounts
                .iter()
                .map(AccountView::line)
                .collect::<Vec<_>>()
                .join("\n"))
        }
        Command::Current => {
            let account = snapshot.current()?;
            Ok(if json {
                serde_json::json!({"schema_version": 1, "account": account, "current_target": snapshot.current_target, "source": "local_index"}).to_string()
            } else {
                format!(
                    "{}\nRecorded target: {} (local index)",
                    account.line(),
                    snapshot.current_target.as_deref().unwrap_or("app")
                )
            })
        }
        Command::Quota(selector) => {
            let account = match selector {
                Some(selector) => snapshot.select(&selector)?,
                None => snapshot.current()?,
            };
            let quota = account.quota.as_ref().ok_or(CliError {
                code: 4,
                message: "No cached quota. Refresh this account's quota in the Tools Lite GUI.",
            })?;
            Ok(if json {
                serde_json::json!({"schema_version": 1, "account_id": account.id, "email": account.email, "cached": true, "quota": quota}).to_string()
            } else {
                quota.human(&account.email)
            })
        }
        Command::Switch { selector, target } => {
            let account = snapshot.select(&selector)?;
            let runtime = tokio::runtime::Runtime::new()
                .map_err(|_| CliError::data("Could not start the account-switch runtime."))?;
            let target_ide = match target.as_str() {
                "ide" => Some("ide"),
                _ => None,
            };
            runtime
                .block_on(crate::modules::account::switch_account(
                    &account.id,
                    target_ide,
                    &HeadlessIntegration,
                ))
                .map_err(|error| switch_error(&error))?;
            Ok(if json {
                serde_json::json!({"schema_version": 1, "switched": true, "account_id": account.id, "email": account.email, "target": target}).to_string()
            } else {
                format!(
                    "Switched {} (target: {}). Start a new agy command to use the updated session.",
                    output::terminal_text(&account.email),
                    target
                )
            })
        }
        Command::Stats => {
            let summary = crate::modules::native_token_stats::get_local_token_usage()
                .map_err(|_| CliError::data("Failed to read local token statistics."))?;
            Ok(if json {
                serde_json::to_string(&summary).unwrap_or_default()
            } else {
                picker::format_token_stats_human(&summary, picker::Lang::current(&data_dir()?))
            })
        }
        Command::Refresh(selector) => {
            let runtime = tokio::runtime::Runtime::new()
                .map_err(|_| CliError::data("Could not start async runtime for quota refresh."))?;
            match selector {
                Some(sel) => {
                    let account_view = snapshot.select(&sel)?;
                    let mut account = crate::modules::account::load_account(&account_view.id)
                        .map_err(|_| CliError::missing())?;
                    let quota = runtime
                        .block_on(crate::modules::account::fetch_quota_with_retry(&mut account))
                        .map_err(|_| CliError::data("Failed to refresh quota from Google API."))?;
                    let _ = crate::modules::account::update_account_quota(&account.id, quota.clone());
                    Ok(if json {
                        serde_json::json!({
                            "schema_version": 1,
                            "account_id": account.id,
                            "email": account.email,
                            "refreshed": true,
                            "quota": quota
                        })
                        .to_string()
                    } else {
                        format!("Refreshed quota for {}.", output::terminal_text(&account.email))
                    })
                }
                None => {
                    let stats = runtime
                        .block_on(crate::modules::account::refresh_all_quotas_logic())
                        .map_err(|_| CliError::data("Failed to batch refresh quotas."))?;
                    Ok(if json {
                        serde_json::json!({
                            "schema_version": 1,
                            "total": stats.total,
                            "success": stats.success,
                            "failed": stats.failed,
                            "details": stats.details
                        })
                        .to_string()
                    } else {
                        format!(
                            "Batch quota refresh complete: {} succeeded, {} failed (total: {}).",
                            stats.success, stats.failed, stats.total
                        )
                    })
                }
            }
        }
        Command::InteractiveDashboard => {
            picker::run_interactive_dashboard(&data_dir()?)?;
            Ok(String::new())
        }
        Command::InteractiveSwitch { target } => {
            if snapshot.accounts.is_empty() {
                return Ok("No saved accounts. Add an account in the Tools Lite GUI.".into());
            }
            let lang = picker::Lang::current(&data_dir()?);
            let selected_account = match picker::select_account_interactive(&snapshot.accounts, lang) {
                Some(acc) => acc,
                None => return Ok(String::new()),
            };
            let runtime = tokio::runtime::Runtime::new()
                .map_err(|_| CliError::data("Could not start the account-switch runtime."))?;
            let target_ide = match target.as_str() {
                "ide" => Some("ide"),
                _ => None,
            };
            runtime
                .block_on(crate::modules::account::switch_account(
                    &selected_account.id,
                    target_ide,
                    &HeadlessIntegration,
                ))
                .map_err(|error| switch_error(&error))?;
            Ok(format!(
                "Switched {} (target: {}). Start a new agy command to use the updated session.",
                output::terminal_text(&selected_account.email),
                target
            ))
        }
        _ => unreachable!(),
    }
}

// Never forward server responses, credentials, or untrusted filenames to the terminal.
fn switch_error(error: &str) -> CliError {
    if error.contains("cli_app_installation_required") {
        return CliError::data("Cannot find Antigravity APP. Set its executable path in Settings; CLI-only file synchronization is not supported.");
    }
    if error.contains("another_account_switch_in_progress") {
        return CliError {
            code: 5,
            message: "Another account switch is in progress. Wait and retry.",
        };
    }
    if error.contains("No initialized native agy")
        || error.contains("No initialized agy")
        || error.contains("not initialized")
    {
        return CliError::data(
            "The native agy session is not initialized. Log in with agy first, then retry.",
        );
    }
    if error.contains("APP") && (error.contains("updated") || error.contains("recovery failed")) {
        return CliError::data("The switch may be partially applied. Check the active accounts in Antigravity and agy before retrying.");
    }
    CliError::data("Account switch failed; credentials may be partially updated. Check Antigravity and agy. Use the Tools Lite GUI to diagnose or sign in again.")
}

fn ensure_cli_switch_target(
    target: Option<&str>,
    app_available: bool,
) -> std::result::Result<(), String> {
    if target.is_none() && !app_available {
        return Err("cli_app_installation_required".into());
    }
    Ok(())
}

pub(crate) struct HeadlessIntegration;
impl crate::modules::integration::SystemIntegration for HeadlessIntegration {
    async fn on_account_switch(
        &self,
        account: &crate::models::Account,
        target_ide: Option<&str>,
    ) -> std::result::Result<(), String> {
        let account = account.clone();
        let target = target_ide.map(str::to_owned);
        tokio::task::spawn_blocking(move || {
            let app_available = target.is_some()
                || crate::modules::process::get_antigravity_executable_path(None).is_some();
            ensure_cli_switch_target(target.as_deref(), app_available)?;
            crate::modules::integration::DesktopIntegration::switch_sync(
                &HeadlessIntegration,
                &account,
                target.as_deref(),
            )
        })
        .await
        .map_err(|_| "Account switch worker failed.".to_string())?
    }
    fn start_application(&self, target_ide: Option<&str>) -> std::result::Result<(), String> {
        crate::modules::process::start_antigravity_detached(target_ide)
    }
    fn update_tray(&self) {}
    fn show_notification(&self, _title: &str, _body: &str) {}
}

pub(crate) use switch_lock::SwitchLock;

#[cfg(test)]
mod tests {
    use super::*;
    fn args(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| (*arg).into()).collect()
    }
    fn parse_test(args_slice: &[&str]) -> Result<(Command, bool)> {
        parse(&args(args_slice), false)
    }
    #[test]
    fn parses_commands_and_json() {
        assert_eq!(
            parse_test(&["accounts", "list", "--json"]).unwrap(),
            (Command::List, true)
        );
        assert_eq!(
            parse_test(&["--json", "current"]).unwrap(),
            (Command::Current, true)
        );
        assert_eq!(
            parse_test(&["--cli", "quota", "a@example.invalid"]).unwrap(),
            (Command::Quota(Some("a@example.invalid".into())), false)
        );
        assert_eq!(
            parse_test(&["switch", "abc", "--target", "ide"]).unwrap(),
            (
                Command::Switch {
                    selector: "abc".into(),
                    target: "ide".into()
                },
                false
            )
        );
        assert_eq!(
            parse(&args(&[]), true).unwrap(),
            (Command::InteractiveDashboard, false)
        );
        assert_eq!(
            parse(&args(&["switch"]), true).unwrap(),
            (
                Command::InteractiveSwitch {
                    target: "app".into()
                },
                false
            )
        );
        assert_eq!(
            parse_test(&["stats", "--json"]).unwrap(),
            (Command::Stats, true)
        );
        assert_eq!(
            parse_test(&["refresh"]).unwrap(),
            (Command::Refresh(None), false)
        );
    }
    #[test]
    fn rejects_ambiguous_or_unknown_arguments() {
        for input in [
            &["switch"][..],
            &["switch", "a", "--target", "unknown"],
            &["switch", "a", "--target", "cli"],
            &["quota", "--refresh"],
            &["export"],
            &["list", "--json", "--json"],
            &["switch", "a", "extra"],
            &["--wat"],
        ] {
            assert_eq!(parse_test(input).unwrap_err().code, 2);
        }
    }
    #[test]
    fn missing_app_cannot_fall_back_to_file_only_switch() {
        assert!(ensure_cli_switch_target(None, false).is_err());
        assert!(ensure_cli_switch_target(None, true).is_ok());
        assert!(ensure_cli_switch_target(Some("ide"), false).is_ok());
    }
    #[test]
    fn errors_do_not_echo_secrets() {
        let error = switch_error("server says secret-refresh-token, access_token=abc");
        assert!(!error.message.contains("secret-refresh-token"));
        assert!(!error.message.contains("abc"));
    }
}
