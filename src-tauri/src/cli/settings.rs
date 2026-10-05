use super::{output::{terminal_text, Snapshot}, picker::Lang, CliError, Result};
use crate::modules::auto_switch::{self, Config, Mode, Strategy, Target};
use std::{collections::HashSet, path::Path};

#[derive(Debug, Default, PartialEq)]
pub(super) struct PolicyPatch {
    enabled: Option<bool>, mode: Option<Mode>, strategy: Option<Strategy>, target: Option<Target>,
    reserve: Option<u8>, minimum: Option<u8>, model: Option<String>, candidates: Option<Vec<String>>,
}

pub(super) fn parse_patch(args: &[&str]) -> Result<PolicyPatch> {
    if args.is_empty() { return Err(CliError::usage()); }
    let mut patch = PolicyPatch::default();
    let mut seen = HashSet::new();
    let mut i = 0;
    while i < args.len() {
        let key = args[i]; i += 1;
        if !seen.insert(key) { return Err(CliError::usage()); }
        if key == "--clear-candidates" {
            if seen.contains("--candidates") { return Err(CliError::usage()); }
            patch.candidates = Some(vec![]); continue;
        }
        if key == "--candidates" {
            if seen.contains("--clear-candidates") { return Err(CliError::usage()); }
            let start = i;
            while i < args.len() && !args[i].starts_with('-') { i += 1; }
            if start == i { return Err(CliError::usage()); }
            patch.candidates = Some(args[start..i].iter().map(|s| (*s).into()).collect()); continue;
        }
        let value = args.get(i).filter(|v| !v.starts_with('-')).ok_or_else(CliError::usage)?; i += 1;
        match key {
            "--enabled" => patch.enabled = Some(match *value { "true" => true, "false" => false, _ => return Err(CliError::usage()) }),
            "--mode" => patch.mode = Some(match *value { "wait" => Mode::Wait, "stop" => Mode::Stop, _ => return Err(CliError::usage()) }),
            "--strategy" => patch.strategy = Some(match *value { "priority" => Strategy::Priority, "round-robin" => Strategy::RoundRobin, _ => return Err(CliError::usage()) }),
            "--target" => patch.target = Some(match *value { "app" => Target::App, "app-cli" => Target::AppCli, "ide" => Target::Ide, "vscode" => Target::Vscode, _ => return Err(CliError::usage()) }),
            "--reserve" => patch.reserve = Some(value.parse().map_err(|_| CliError::usage())?),
            "--minimum" => patch.minimum = Some(value.parse().map_err(|_| CliError::usage())?),
            "--model" if !value.trim().is_empty() && value.len() <= 200 && !value.chars().any(char::is_control) => patch.model = Some(value.trim().into()),
            _ => return Err(CliError::usage()),
        }
    }
    Ok(patch)
}

pub(super) fn read_policy(root: &Path) -> Result<Config> {
    auto_switch::read_config_at(root).map_err(|_| CliError::data("Cannot read smart-switch settings. Check auto_switch.json."))
}

fn resolve_ids(snapshot: &Snapshot, selectors: &[String]) -> Result<Vec<String>> {
    let ids = selectors.iter().map(|s| snapshot.select(s).map(|a| a.id.clone())).collect::<Result<Vec<_>>>()?;
    if ids.iter().collect::<HashSet<_>>().len() != ids.len() { return Err(CliError { code: 2, message: "Each account must appear only once." }); }
    Ok(ids)
}

pub(super) fn save_policy(root: &Path, expected: &Config, config: &Config) -> Result<()> {
    config.validate().map_err(|_| CliError { code: 2, message: "Invalid policy: reserve must be 1–98%, minimum must exceed reserve (up to 100%), and enabling requires unique backup accounts." })?;
    let snapshot = Snapshot::read(root)?;
    if config.enabled && config.candidate_account_ids.iter().any(|id| !snapshot.accounts.iter().any(|a| &a.id == id)) { return Err(CliError::missing()); }
    if config == expected { return Ok(()); }
    std::fs::create_dir_all(root).map_err(|_| CliError::data("Cannot create the settings directory."))?;
    auto_switch::save_config_at(root, expected, config).map_err(|error| match error.as_str() {
        "another_account_switch_in_progress" => CliError { code: 5, message: "Another switch or settings change is in progress. Wait and retry." },
        "auto_switch_settings_changed" => CliError { code: 5, message: "Smart-switch settings changed in another client. Reload and retry." },
        _ => CliError::data("Cannot save smart-switch settings. Reload before retrying."),
    })
}

pub(super) fn policy_text(config: &Config, lang: Lang) -> String {
    let zh = lang == Lang::Zh;
    let enabled = if zh { if config.enabled { "开启" } else { "关闭" } } else if config.enabled { "On" } else { "Off" };
    let mode = match (lang, config.mode) { (Lang::Zh, Mode::Wait) => "等待空闲", (Lang::Zh, Mode::Stop) => "达到阈值时切换", (Lang::En, Mode::Wait) => "Wait for inactivity", (Lang::En, Mode::Stop) => "Switch at threshold" };
    let strategy = match (lang, config.strategy) { (Lang::Zh, Strategy::Priority) => "优先顺序", (Lang::Zh, Strategy::RoundRobin) => "轮询", (Lang::En, Strategy::Priority) => "Priority", (Lang::En, Strategy::RoundRobin) => "Round robin" };
    let target = match config.target { Target::App => if zh { "全域同步" } else { "Global sync" }, Target::AppCli => "APP + agy", Target::Ide => "IDE", Target::Vscode => "VS Code" };
    let model = match (lang, config.monitored_model.as_str()) { (Lang::Zh, "all") => "全部模型", (Lang::En, "all") => "All models", (_, "gemini") => "Gemini", (Lang::Zh, "claude") => "Claude 和 GPT", (Lang::En, "claude") => "Claude & GPT", (_, value) => value };
    if zh { format!("智能切换策略: {enabled}\n切换时机: {mode}\n账号选择顺序: {strategy}\n同步目标: {target}\n监控模型: {}\n保留额度: {}%\n候选最低额度: {}%\n候选账号顺序: {}\n后台执行需要运行 agy-switch 桌面应用。", terminal_text(model), config.reserve_percentage, config.candidate_min_percentage, config.candidate_account_ids.iter().map(|s| terminal_text(s)).collect::<Vec<_>>().join(" → ")) }
    else { format!("Smart switching: {enabled}\nSwitch timing: {mode}\nAccount selection order: {strategy}\nSync target: {target}\nMonitored models: {}\nReserve: {}%\nBackup minimum: {}%\nCandidate order: {}\nBackground execution requires the agy-switch desktop app.", terminal_text(model), config.reserve_percentage, config.candidate_min_percentage, config.candidate_account_ids.iter().map(|s| terminal_text(s)).collect::<Vec<_>>().join(" → ")) }
}

fn policy_output(config: &Config, json: bool, lang: Lang) -> String {
    if json { serde_json::json!({"schema_version": 1, "policy": config, "executor": "desktop_app"}).to_string() } else { policy_text(config, lang) }
}
pub(super) fn show_policy(root: &Path, json: bool) -> Result<String> {
    Ok(policy_output(&read_policy(root)?, json, Lang::current(root)))
}
pub(super) fn set_policy(root: &Path, patch: PolicyPatch, json: bool) -> Result<String> {
    let expected = read_policy(root)?;
    let mut config = expected.clone();
    if let Some(v) = patch.enabled { config.enabled = v; }
    if let Some(v) = patch.mode { config.mode = v; }
    if let Some(v) = patch.strategy { config.strategy = v; }
    if let Some(v) = patch.target { config.target = v; }
    if let Some(v) = patch.reserve { config.reserve_percentage = v; }
    if let Some(v) = patch.minimum { config.candidate_min_percentage = v; }
    if let Some(v) = patch.model { config.monitored_model = v.trim().into(); }
    if let Some(v) = patch.candidates { config.candidate_account_ids = resolve_ids(&Snapshot::read(root)?, &v)?; }
    save_policy(root, &expected, &config)?;
    Ok(policy_output(&config, json, Lang::current(root)))
}
pub(super) fn order_policy(root: &Path, selectors: &[String], json: bool) -> Result<String> {
    let expected = read_policy(root)?;
    if selectors.is_empty() { return Ok(policy_output(&expected, json, Lang::current(root))); }
    let mut config = expected.clone();
    config.candidate_account_ids = resolve_ids(&Snapshot::read(root)?, selectors)?;
    if !same_members(&config.candidate_account_ids, &expected.candidate_account_ids) { return Err(CliError { code: 2, message: "Provide every selected candidate exactly once. Use policy set --candidates to change the selection." }); }
    save_policy(root, &expected, &config)?;
    Ok(policy_output(&config, json, Lang::current(root)))
}
fn same_members(a: &[String], b: &[String]) -> bool { a.len() == b.len() && a.iter().collect::<HashSet<_>>() == b.iter().collect::<HashSet<_>>() }

pub(super) fn save_account_order(root: &Path, expected: &[String], ordered: &[String]) -> Result<()> {
    if !same_members(expected, ordered) || ordered.iter().collect::<HashSet<_>>().len() != ordered.len() { return Err(CliError::usage()); }
    crate::modules::account::reorder_accounts_at(root, ordered, Some(expected)).map_err(|e| match e.as_str() {
        "account_order_changed" => CliError { code: 5, message: "Account order changed in another client. Reload and retry." },
        "another_account_switch_in_progress" => CliError { code: 5, message: "Another switch or settings change is in progress. Wait and retry." },
        _ => CliError::data("Cannot save account order. Reload before retrying."),
    })
}
pub(super) fn order_accounts(root: &Path, selectors: &[String], json: bool) -> Result<String> {
    let snapshot = Snapshot::read(root)?;
    let current = snapshot.accounts.iter().map(|a| a.id.clone()).collect::<Vec<_>>();
    let ordered = if selectors.is_empty() { current.clone() } else { resolve_ids(&snapshot, selectors)? };
    if !selectors.is_empty() { save_account_order(root, &current, &ordered)?; }
    Ok(if json { serde_json::json!({"schema_version": 1, "account_ids": ordered}).to_string() }
        else { ordered.iter().enumerate().map(|(i, id)| format!("{}. {}  {}", i+1, terminal_text(id), terminal_text(&snapshot.select(id).unwrap().email))).collect::<Vec<_>>().join("\n") })
}

pub(super) fn check_update(json: bool, lang: Lang) -> Result<String> {
    let runtime = tokio::runtime::Runtime::new().map_err(|_| CliError::data("Cannot start the update-check runtime."))?;
    let info = runtime.block_on(crate::modules::updater::check_for_updates()).map_err(|_| CliError::data("Update check failed. Check the network or GitHub rate limit and retry."))?;
    Ok(update_output(&info, json, lang))
}
fn update_output(info: &crate::modules::updater::UpdateInfo, json: bool, lang: Lang) -> String {
    if json { serde_json::json!({"schema_version": 1, "update": info, "check_only": true}).to_string() }
    else { match lang {
        Lang::Zh => format!("当前版本: {}\n最新版本: {}\n{}\n{}", info.current_version, info.latest_version, if info.has_update { "有新版本；请在 App 中安装更新，或通过原安装方式升级。" } else { "已是最新稳定版。" }, info.release_url),
        Lang::En => format!("Installed: {}\nLatest: {}\n{}\n{}", info.current_version, info.latest_version, if info.has_update { "Update available. Install in the app or use your original installation method." } else { "Up to date with the latest stable release." }, info.release_url),
    } }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parser_rejects_conflicting_duplicate_or_missing_options() {
        for args in [vec![], vec!["--strategy", "random"], vec!["--enabled", "1"], vec!["--reserve"], vec!["--reserve", "10", "--reserve", "20"], vec!["--candidates", "A", "--clear-candidates"], vec!["--clear-candidates", "--candidates", "A"]] { assert!(parse_patch(&args).is_err()); }
        let p = parse_patch(&["--candidates", "A", "B", "--enabled", "true", "--strategy", "round-robin"]).unwrap();
        assert_eq!(p.candidates.unwrap(), vec!["A", "B"]); assert_eq!(p.strategy, Some(Strategy::RoundRobin));
    }
    #[test]
    fn defaults_are_read_only_and_invalid_data_is_not_repaired() {
        let dir = tempfile::tempdir().unwrap(); let root = dir.path().join("missing");
        assert_eq!(read_policy(&root).unwrap(), Config::default()); assert!(!root.exists());
        std::fs::write(dir.path().join("auto_switch.json"), "broken").unwrap();
        assert!(read_policy(dir.path()).is_err()); assert_eq!(std::fs::read_to_string(dir.path().join("auto_switch.json")).unwrap(), "broken");
    }
    #[test]
    fn update_result_is_check_only_and_keeps_languages_separate() {
        let info = crate::modules::updater::UpdateInfo { current_version: "4.8.1".into(), latest_version: "v4.8.2".into(), has_update: true, release_url: "https://github.com/anglee0323/agy-switch/releases/tag/v4.8.2".into() };
        assert!(update_output(&info, false, Lang::En).contains("Update available"));
        assert!(update_output(&info, false, Lang::Zh).contains("有新版本"));
        assert_eq!(serde_json::from_str::<serde_json::Value>(&update_output(&info, true, Lang::En)).unwrap()["check_only"], true);
    }
    #[test]
    fn policy_summary_separates_timing_and_order_in_both_languages() {
        let en = policy_text(&Config::default(), Lang::En);
        let zh = policy_text(&Config::default(), Lang::Zh);
        assert!(en.contains("Switch timing:") && en.contains("Account selection order:"));
        assert!(zh.contains("切换时机:") && zh.contains("账号选择顺序:"));
        assert!(!en.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c)));
    }
}
