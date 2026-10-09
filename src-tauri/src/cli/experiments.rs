//! Experimental controls share the desktop's saved translation switch.
use super::{picker::Lang, CliError, Result};
use crate::modules::app_experiments;
use std::path::Path;

pub(super) fn read(root: &Path) -> Result<bool> {
    app_experiments::translation_enabled_at(root).map_err(|_| {
        CliError::data("Cannot read experimental settings. Check app_experiments.json.")
    })
}

pub(super) fn configure(root: &Path, enabled: bool) -> Result<()> {
    app_experiments::configure_translation_at(root, enabled).map_err(|_| {
        CliError::data(
            "Cannot save experimental settings. Check app_experiments.json and file permissions.",
        )
    })
}

pub(super) fn description(lang: Lang) -> &'static str {
    match lang {
        Lang::Zh => "仅对 Windows/macOS Antigravity App 生效；Antigravity IDE、agy CLI、VS Code 和 JetBrains 插件不受影响。通过现有本机连接注入翻译脚本，只替换已知界面文字，不修改 App 安装文件或源文件，不改权限或账号设置，也不上传聊天内容。脚本在 App 页面中执行，会观察界面变化；这不代表注入毫无风险。关闭后恢复原文，停止续期后最迟 15 秒恢复。持续汉化需保持 Switch 桌面进程运行，或运行 agy-switch experiments run。",
        Lang::En => "Applies only to Antigravity App on Windows/macOS. Antigravity IDE, agy CLI, VS Code and JetBrains plugins are unaffected. A translation script is injected through its existing local connection to replace known UI labels. It does not modify App installation/source files, permissions or accounts, or upload conversations. The script runs inside App pages and observes UI changes; injection is not risk-free. Turning it off restores the original text; stopping renewals restores it within 15 seconds. Keep Switch desktop running or use agy-switch experiments run.",
    }
}

pub(super) fn output(enabled: bool, json: bool, lang: Lang) -> String {
    if json {
        serde_json::json!({"schema_version":1,"experiments":{"translation_enabled":enabled,"scope":"antigravity_app"},"runtime_checked":false,"executor":"desktop_or_foreground_cli"}).to_string()
    } else {
        let state = match (lang, enabled) {
            (Lang::Zh, true) => "开启",
            (Lang::Zh, false) => "关闭",
            (Lang::En, true) => "On",
            (Lang::En, false) => "Off",
        };
        format!(
            "{}: {state}\n{}",
            if lang == Lang::Zh {
                "界面汉化设置"
            } else {
                "Chinese interface setting"
            },
            description(lang)
        )
    }
}

pub(super) fn show(root: &Path, json: bool) -> Result<String> {
    Ok(output(read(root)?, json, Lang::current(root)))
}

pub(super) fn set(root: &Path, enabled: bool, json: bool) -> Result<String> {
    configure(root, enabled)?;
    show(root, json)
}

pub(super) fn run_foreground(root: &Path, lang: Lang) -> Result<String> {
    if !cfg!(any(target_os = "macos", target_os = "windows")) {
        return Err(CliError::data(
            "App translation is available only on Windows/macOS.",
        ));
    }
    if !read(root)? {
        return Err(CliError::data(
            "Enable translation first: agy-switch experiments translation on.",
        ));
    }
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|_| CliError::data("Cannot start the translation runtime."))?;
    // A foreground process only. No detached child, service, account operation
    // or security-setting change; the runtime's lease handles process exit.
    let _worker = app_experiments::start_foreground(root);
    println!("{}", description(lang));
    println!(
        "{}",
        if lang == Lang::Zh {
            "正在维持汉化；等待 Antigravity App。Ctrl+C 停止前台续期。"
        } else {
            "Maintaining translation; waiting for Antigravity App. Ctrl+C stops foreground renewals."
        }
    );
    runtime
        .block_on(tokio::signal::ctrl_c())
        .map_err(|_| CliError::data("Cannot listen for Ctrl+C."))?;
    Ok(if lang == Lang::Zh { "已停止前台续期；没有其他 Switch 进程续期时，页面将在 15 秒内恢复原文。" } else { "Foreground renewals stopped. Without another Switch process renewing them, pages restore within 15 seconds." }.into())
}
