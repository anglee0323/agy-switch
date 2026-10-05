//! Terminal workflows use the same setting/order operations as one-line commands.
use super::{output::{terminal_text, Snapshot}, picker::{self, KeyAction, Lang, PromptResult, RawTerminal}, settings, CliError};
use crate::modules::auto_switch::{Mode, Strategy, Target};
use std::{io::{self, Write}, path::Path};

fn text<'a>(lang: Lang, zh: &'a str, en: &'a str) -> &'a str { if lang == Lang::Zh { zh } else { en } }
fn menu(lang: Lang, title: &str, items: &[String], initial: usize) -> Option<usize> {
    picker::select_menu_interactive(title, &items.iter().map(String::as_str).collect::<Vec<_>>(), initial, lang)
}
fn report(lang: Lang, result: Result<(), CliError>) {
    match result {
        Ok(()) => println!("{}", text(lang, "设置已保存。", "Settings saved.")),
        Err(e) => println!("{}", if lang == Lang::En { e.message } else { match e.code {
            2 => "设置无效：保留额度需为 1–98%，候选最低额度需高于保留额度；启用前请选择候选账号。",
            3 => "账号已不存在，请重新加载账号列表。",
            5 => "设置已被其他端修改，或正在切换账号。请重新加载后再试。",
            _ => "无法读取或保存设置，请检查文件权限并重新加载。",
        } }),
    }
    picker::wait_for_key(lang);
}

pub(super) fn show_settings(root: &Path, lang: Lang) {
    loop {
        print!("\x1b[2J\x1b[H");
        let items = [text(lang, "1. 智能切换策略", "1. Smart switching"), text(lang, "2. 账号列表顺序", "2. Account list order"), text(lang, "0. 返回", "0. Back")];
        match picker::select_menu_interactive(text(lang, "策略与排序", "Settings & Order"), &items, 0, lang) {
            Some(0) => edit_policy(root, lang),
            Some(1) => {
                let result = Snapshot::read(root).and_then(|snapshot| {
                    let expected = snapshot.accounts.iter().map(|a| a.id.clone()).collect::<Vec<_>>();
                    if let Some(ordered) = edit_order(&snapshot, &expected, false, lang) {
                        settings::save_account_order(root, &expected, &ordered)?;
                    } else { return Ok(false); }
                    Ok(true)
                });
                match result { Ok(false) => {}, Ok(true) => report(lang, Ok(())), Err(e) => report(lang, Err(e)) }
            }
            _ => break,
        }
    }
}

fn edit_policy(root: &Path, lang: Lang) {
    let mut expected = match settings::read_policy(root) { Ok(c) => c, Err(e) => { report(lang, Err(e)); return; } };
    let mut draft = expected.clone();
    let mut selected = 0;
    loop {
        print!("\x1b[2J\x1b[H");
        println!("{}\n", text(lang, "后台策略由 agy-switch 桌面应用 执行；返回会放弃未保存的修改。", "The desktop app runs this policy. Back discards unsaved changes."));
        let on = if draft.enabled { text(lang, "开启", "On") } else { text(lang, "关闭", "Off") };
        let mode = match draft.mode { Mode::Wait => text(lang, "检测空闲后切换", "Wait for inactivity"), Mode::Stop => text(lang, "达到阈值后切换", "Switch at threshold") };
        let strategy = match draft.strategy { Strategy::Priority => text(lang, "优先顺序", "Priority"), Strategy::RoundRobin => text(lang, "循环轮换", "Round robin") };
        let target = match draft.target { Target::App => text(lang, "全域同步", "Global sync"), Target::AppCli => "APP + agy", Target::Ide => "IDE", Target::Vscode => "VS Code" };
        let scope = match draft.monitored_model.as_str() { "all" | "" => text(lang, "全部模型", "All models"), "gemini" => "Gemini", "claude" => text(lang, "Claude 和 GPT", "Claude & GPT"), value => value };
        let items = vec![
            format!("1. {}: {on}", text(lang, "智能切换", "Smart switching")),
            format!("2. {}: {mode}", text(lang, "切换时机", "Switch timing")),
            format!("3. {}: {strategy}", text(lang, "账号选择顺序", "Account selection order")),
            format!("4. {}: {target}", text(lang, "同步目标", "Sync target")),
            format!("5. {}: {}", text(lang, "监控模型", "Monitored models"), terminal_text(scope)),
            format!("6. {}: {}% / {}%", text(lang, "保留额度 / 候选最低额度", "Reserve / Backup minimum"), draft.reserve_percentage, draft.candidate_min_percentage),
            format!("7. {}: {}", text(lang, "候选账号与排序", "Candidates & Order"), draft.candidate_account_ids.len()),
            text(lang, "8. 保存修改", "8. Save changes").into(),
            text(lang, "9. 重新加载配置", "9. Reload settings").into(),
            text(lang, "0. 返回", "0. Back").into(),
        ];
        let Some(choice) = menu(lang, text(lang, "智能切换策略", "Smart switching"), &items, selected) else { break; };
        selected = choice;
        match choice {
            0 => draft.enabled = !draft.enabled,
            1 => {
                print!("\x1b[2J\x1b[H");
                println!("{}\n", text(lang, "达到阈值后切换可能中断进行中的任务。", "Switching at the threshold may interrupt running work."));
                let items = [text(lang, "1. 检测空闲后切换", "1. Wait for inactivity"), text(lang, "2. 达到阈值后切换", "2. Switch at threshold")];
                if let Some(i) = picker::select_menu_interactive(text(lang, "切换时机", "Switch timing"), &items, usize::from(draft.mode == Mode::Stop), lang) { draft.mode = if i == 0 { Mode::Wait } else { Mode::Stop }; }
            }
            2 => {
                print!("\x1b[2J\x1b[H");
                let items = [text(lang, "1. 优先顺序：从候选列表首位选择", "1. Priority: start at the top of the candidate list"), text(lang, "2. 循环轮换：从当前账号的下一位选择", "2. Round robin: start after the current account")];
                if let Some(i) = picker::select_menu_interactive(text(lang, "账号选择顺序", "Account selection order"), &items, usize::from(draft.strategy == Strategy::RoundRobin), lang) { draft.strategy = if i == 0 { Strategy::Priority } else { Strategy::RoundRobin }; }
            }
            3 => {
                let items = [text(lang, "1. 全域同步", "1. Global sync"), "2. APP + agy", "3. IDE", "4. VS Code"];
                let targets = [Target::App, Target::AppCli, Target::Ide, Target::Vscode];
                print!("\x1b[2J\x1b[H");
                if let Some(i) = picker::select_menu_interactive(text(lang, "同步目标", "Sync target"), &items, targets.iter().position(|t| *t == draft.target).unwrap_or(0), lang) { draft.target = targets[i]; }
            }
            4 => {
                let items = [text(lang, "1. 全部模型", "1. All models"), "2. Gemini", text(lang, "3. Claude 和 GPT", "3. Claude & GPT"), text(lang, "4. 指定模型 ID", "4. Exact model ID")];
                print!("\x1b[2J\x1b[H");
                match picker::select_menu_interactive(text(lang, "监控模型", "Monitored models"), &items, 0, lang) {
                    Some(i @ 0..=2) => draft.monitored_model = ["all", "gemini", "claude"][i].into(),
                    Some(3) => if let PromptResult::Confirmed(value) = picker::prompt_line_with_cancel(text(lang, "模型 ID: ", "Model ID: "), 200) { if !value.is_empty() { draft.monitored_model = value; } },
                    _ => {},
                }
            }
            5 => {
                print!("\x1b[2J\x1b[H");
                let reserve = prompt_percentage(text(lang, "保留额度（1–98）: ", "Reserve (1–98): "), 1, 98, lang);
                if let Some(reserve) = reserve { if let Some(minimum) = prompt_percentage(text(lang, "候选最低额度（需高于保留额度，最高 100）: ", "Backup minimum (above reserve, up to 100): "), reserve + 1, 100, lang) { draft.reserve_percentage = reserve; draft.candidate_min_percentage = minimum; } }
            }
            6 => match Snapshot::read(root) {
                Ok(snapshot) => if let Some(ids) = edit_order(&snapshot, &draft.candidate_account_ids, true, lang) { draft.candidate_account_ids = ids; },
                Err(e) => report(lang, Err(e)),
            },
            7 => { let result = settings::save_policy(root, &expected, &draft); let saved = result.is_ok(); report(lang, result); if saved { break; } }
            8 => match settings::read_policy(root) { Ok(c) => { expected = c; draft = expected.clone(); }, Err(e) => report(lang, Err(e)) },
            _ => break,
        }
    }
}

fn prompt_percentage(prompt: &str, minimum: u8, maximum: u8, lang: Lang) -> Option<u8> {
    loop {
        match picker::prompt_line_with_cancel(prompt, 3) {
            PromptResult::Confirmed(value) => if let Some(value) = value.parse::<u8>().ok().filter(|v| (minimum..=maximum).contains(v)) { return Some(value); },
            PromptResult::Cancelled => return None,
        }
        println!("{} {minimum}–{maximum}", text(lang, "请输入范围内的整数：", "Enter a whole number in this range:"));
    }
}

#[derive(Debug)]
struct OrderDraft { rows: Vec<(String, String, bool)>, selected: usize }
impl OrderDraft {
    fn move_selected(&mut self, up: bool) {
        if self.rows.is_empty() { return; }
        let next = if up { self.selected.saturating_sub(1) } else { (self.selected + 1).min(self.rows.len() - 1) };
        self.rows.swap(self.selected, next); self.selected = next;
    }
    fn ids(&self) -> Vec<String> { self.rows.iter().filter(|r| r.2).map(|r| r.0.clone()).collect() }
}

fn edit_order(snapshot: &Snapshot, initial: &[String], candidates: bool, lang: Lang) -> Option<Vec<String>> {
    let all = snapshot.accounts.iter().map(|a| &a.id).collect::<Vec<_>>();
    let ids = initial.iter().filter(|id| all.contains(id)).chain(all.iter().copied().filter(|id| !initial.contains(id)));
    let mut draft = OrderDraft { rows: ids.map(|id| {
        let a = snapshot.select(id).unwrap();
        let state = if a.disabled || a.validation_blocked || a.quota.as_ref().is_some_and(|q| q.is_forbidden) { text(lang, "（不可用）", " (unavailable)") } else { "" };
        (id.clone(), format!("{}{}", terminal_text(&a.email), state), !candidates || initial.contains(id))
    }).collect(), selected: 0 };
    let Some(_raw) = RawTerminal::enter() else { return None; };
    loop {
        print!("\x1b[2J\x1b[H\x1b[?25l");
        println!("\x1b[1m{}\x1b[0m", if candidates { text(lang, "候选账号与排序", "Candidates & Order") } else { text(lang, "账号列表顺序", "Account list order") });
        println!("{}", text(lang, "↑↓ 选择  Shift+↑↓ / U/D 调整顺序  Enter/→ 确认  Esc/← 取消", "Up/Down select  Shift+Up/Down or U/D reorder  Enter/Right apply  Esc/Left cancel"));
        if candidates { println!("{}", text(lang, "空格选择候选账号；顺序仅对选中的账号生效。", "Space selects a candidate. Only checked accounts enter the policy.")); }
        let visible = picker::get_terminal_height().saturating_sub(6).max(1);
        let start = draft.selected.saturating_sub(visible - 1);
        for (i, (_, label, included)) in draft.rows.iter().enumerate().skip(start).take(visible) {
            let label = picker::truncate_display_width(label, picker::get_terminal_width().saturating_sub(14));
            println!("{} {:>3}. {} {}", if i == draft.selected { "➤" } else { " " }, i + 1, if candidates { if *included { "[x]" } else { "[ ]" } } else { "" }, label);
        }
        if draft.rows.is_empty() { println!("{}", text(lang, "暂无账号。", "No saved accounts.")); }
        let _ = io::stdout().flush();
        match picker::read_key_action() {
            KeyAction::Cancel | KeyAction::Char('0') => return None,
            KeyAction::Enter => return if draft.rows.is_empty() && !candidates { None } else { Some(draft.ids()) },
            KeyAction::Up if !draft.rows.is_empty() => draft.selected = if draft.selected == 0 { draft.rows.len() - 1 } else { draft.selected - 1 },
            KeyAction::Down if !draft.rows.is_empty() => draft.selected = (draft.selected + 1) % draft.rows.len(),
            KeyAction::Home => draft.selected = 0,
            KeyAction::End => draft.selected = draft.rows.len().saturating_sub(1),
            KeyAction::MoveUp | KeyAction::Char('u' | 'U') => draft.move_selected(true),
            KeyAction::MoveDown | KeyAction::Char('d' | 'D') => draft.move_selected(false),
            KeyAction::Char(' ') if candidates && !draft.rows.is_empty() => draft.rows[draft.selected].2 = !draft.rows[draft.selected].2,
            KeyAction::SelectIndex(i) if i < draft.rows.len() => draft.selected = i,
            _ => {},
        }
    }
}

pub(super) fn show_updates(lang: Lang) {
    print!("\x1b[2J\x1b[H");
    println!("{}", text(lang, "正在检查更新…", "Checking for updates…"));
    match settings::check_update(false, lang) {
        Ok(result) => println!("\n{result}"),
        Err(_) => println!("{}", text(lang, "更新检查失败，请检查网络或稍后重试。", "Update check failed. Check the network and retry later.")),
    }
    picker::wait_for_key(lang);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moving_candidates_preserves_selection_and_stops_at_boundaries() {
        let mut draft = OrderDraft { rows: vec![("A".into(), "".into(), true), ("B".into(), "".into(), false), ("C".into(), "".into(), true)], selected: 2 };
        draft.move_selected(true); assert_eq!(draft.ids(), vec!["A", "C"]);
        draft.move_selected(true); assert_eq!(draft.ids(), vec!["C", "A"]);
        draft.move_selected(true); assert_eq!(draft.selected, 0);
        draft.move_selected(false); assert_eq!(draft.ids(), vec!["A", "C"]);
    }
}
