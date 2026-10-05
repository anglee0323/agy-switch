use super::output::{AccountView, QuotaView, Snapshot};
use super::{CliError, HeadlessIntegration};
use std::io::{self, Write};
use crossterm::{event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers}, terminal, execute, cursor};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Zh,
    En,
}

impl Lang {
    pub fn current(root: &Path) -> Self {
        if let Ok(content) = std::fs::read_to_string(root.join("gui_config.json")) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(l) = v.get("language").and_then(|l| l.as_str()) {
                    if l.starts_with("zh") {
                        return Lang::Zh;
                    } else if l.starts_with("en") {
                        return Lang::En;
                    }
                }
            }
        }
        if let Some(loc) = sys_locale::get_locale() {
            if loc.starts_with("zh") {
                return Lang::Zh;
            }
        }
        Lang::En
    }
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::new();
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn display_width(s: &str) -> usize {
    let clean = strip_ansi(s);
    clean
        .chars()
        .map(|c| {
            let u = c as u32;
            if (0x4E00..=0x9FFF).contains(&u)
                || (0x3400..=0x4DBF).contains(&u)
                || (0x20000..=0x2A6DF).contains(&u)
                || (0xF900..=0xFAFF).contains(&u)
                || (0xFF01..=0xFF60).contains(&u)
                || (0xFFE0..=0xFFE6).contains(&u)
            {
                2
            } else {
                1
            }
        })
        .sum()
}

pub fn pad_right(s: &str, target_width: usize) -> String {
    let w = display_width(s);
    if w >= target_width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(target_width - w))
    }
}

pub fn get_terminal_width() -> usize {
    terminal::size().map(|(width, _)| usize::from(width)).ok().filter(|v| *v > 0)
        .or_else(|| std::env::var("COLUMNS").ok()?.parse().ok()).unwrap_or(80)
}

pub fn get_terminal_height() -> usize {
    terminal::size().map(|(_, height)| usize::from(height)).ok().filter(|v| *v > 0)
        .or_else(|| std::env::var("LINES").ok()?.parse().ok()).unwrap_or(24)
}

pub fn truncate_display_width(s: &str, max_w: usize) -> String {
    let clean = strip_ansi(s);
    if display_width(&clean) <= max_w {
        return s.to_string();
    }
    let budget = if max_w > 1 { max_w - 1 } else { max_w };
    let mut out = String::new();
    let mut cur_w = 0;
    for c in clean.chars() {
        let cw = display_width(&c.to_string());
        if cur_w + cw > budget {
            break;
        }
        out.push(c);
        cur_w += cw;
    }
    if max_w > 1 {
        out.push('…');
    }
    out
}

pub fn pad_left(s: &str, target_width: usize) -> String {
    let w = display_width(s);
    if w >= target_width {
        s.to_string()
    } else {
        format!("{}{}", " ".repeat(target_width - w), s)
    }
}

pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    align_right: Vec<bool>,
}

impl Table {
    pub fn new(headers: Vec<&str>) -> Self {
        let count = headers.len();
        Self {
            headers: headers.into_iter().map(String::from).collect(),
            rows: Vec::new(),
            align_right: vec![false; count],
        }
    }

    pub fn set_align_right(&mut self, col: usize, right: bool) {
        if col < self.align_right.len() {
            self.align_right[col] = right;
        }
    }

    pub fn add_row(&mut self, row: Vec<String>) {
        self.rows.push(row);
    }

    pub fn render(&self) -> String {
        let term_w = get_terminal_width();
        let max_w = term_w.saturating_sub(2).max(40);
        self.render_with_max_width(max_w)
    }

    pub fn render_with_max_width(&self, max_width: usize) -> String {
        let num_cols = self.headers.len();
        if num_cols == 0 {
            return String::new();
        }
        let mut col_widths = vec![0; num_cols];

        for (i, h) in self.headers.iter().enumerate() {
            col_widths[i] = col_widths[i].max(display_width(h));
        }

        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < num_cols {
                    for line in cell.lines() {
                        col_widths[i] = col_widths[i].max(display_width(line));
                    }
                }
            }
        }

        let border_overhead = num_cols * 3 + 1;
        let mut total_content_width: usize = col_widths.iter().sum();
        let total_width = total_content_width + border_overhead;

        if total_width > max_width && max_width > border_overhead {
            let max_content_width = max_width - border_overhead;
            while total_content_width > max_content_width {
                let (widest_idx, &widest_w) = col_widths
                    .iter()
                    .enumerate()
                    .max_by_key(|&(_, w)| *w)
                    .unwrap();
                if widest_w <= 4 {
                    break;
                }
                col_widths[widest_idx] -= 1;
                total_content_width -= 1;
            }
        }

        let mut out = String::new();

        // Top border: ┌───┬───┐
        out.push_str("\r┌");
        for (i, w) in col_widths.iter().enumerate() {
            out.push_str(&"─".repeat(*w + 2));
            if i + 1 < num_cols {
                out.push('┬');
            }
        }
        out.push_str("┐\n");

        // Header: │ Title │ ... │
        out.push_str("\r│");
        for (i, h) in self.headers.iter().enumerate() {
            out.push(' ');
            let w = col_widths[i];
            let clean = strip_ansi(h);
            let truncated = if display_width(&clean) > w {
                truncate_display_width(&clean, w)
            } else {
                h.clone()
            };
            let cell = if self.align_right.get(i).copied().unwrap_or(false) {
                pad_left(&truncated, w)
            } else {
                pad_right(&truncated, w)
            };
            out.push_str(&format!("\x1b[1m{}\x1b[0m", cell));
            out.push(' ');
            out.push('│');
        }
        out.push('\n');

        // Header separator: ├───┼───┤
        out.push_str("\r├");
        for (i, w) in col_widths.iter().enumerate() {
            out.push_str(&"─".repeat(*w + 2));
            if i + 1 < num_cols {
                out.push('┼');
            }
        }
        out.push_str("┤\n");

        let has_multiline = self.rows.iter().any(|r| r.iter().any(|c| c.contains('\n')));

        // Data rows
        for (row_idx, row) in self.rows.iter().enumerate() {
            if has_multiline && row_idx > 0 {
                out.push_str("\r├");
                for (i, w) in col_widths.iter().enumerate() {
                    out.push_str(&"─".repeat(*w + 2));
                    if i + 1 < num_cols {
                        out.push('┼');
                    }
                }
                out.push_str("┤\n");
            }

            let cell_lines: Vec<Vec<&str>> = row.iter().map(|c| {
                let lines: Vec<&str> = c.lines().collect();
                if lines.is_empty() {
                    vec![""]
                } else {
                    lines
                }
            }).collect();
            let max_sublines = cell_lines.iter().map(|l| l.len()).max().unwrap_or(1);

            for sub_idx in 0..max_sublines {
                out.push_str("\r│");
                for i in 0..num_cols {
                    out.push(' ');
                    let sub_line = cell_lines.get(i).and_then(|lines| lines.get(sub_idx)).copied().unwrap_or("");
                    let clean = strip_ansi(sub_line);
                    let w = col_widths[i];
                    let formatted = if display_width(&clean) > w {
                        let truncated = truncate_display_width(&clean, w);
                        if self.align_right.get(i).copied().unwrap_or(false) {
                            pad_left(&truncated, w)
                        } else {
                            pad_right(&truncated, w)
                        }
                    } else {
                        if self.align_right.get(i).copied().unwrap_or(false) {
                            pad_left(sub_line, w)
                        } else {
                            pad_right(sub_line, w)
                        }
                    };
                    out.push_str(&formatted);
                    out.push(' ');
                    out.push('│');
                }
                out.push('\n');
            }
        }

        // Bottom border: └───┴───┘
        out.push_str("\r└");
        for (i, w) in col_widths.iter().enumerate() {
            out.push_str(&"─".repeat(*w + 2));
            if i + 1 < num_cols {
                out.push('┴');
            }
        }
        out.push_str("┘\n");

        out
    }
}

pub(crate) fn quota_brief(quota: Option<&QuotaView>, lang: Lang) -> String {
    if let Some(q) = quota {
        if q.is_forbidden {
            return match lang {
                Lang::Zh => " (额度受限)".into(),
                Lang::En => " (Forbidden)".into(),
            };
        }
        let mut parts = Vec::new();
        if let Some(groups) = &q.quota_groups {
            for g in groups {
                if let Some(b) = g.buckets.first() {
                    let pct = (b.remaining_fraction * 100.0).round() as i32;
                    let short_name = if g.display_name.contains("Gemini") {
                        "Gemini"
                    } else if g.display_name.contains("Claude") || g.display_name.contains("GPT") {
                        "Claude/GPT"
                    } else {
                        &g.display_name
                    };
                    parts.push(format!("{}: {}%", short_name, pct));
                }
            }
        } else {
            for m in q.models.iter().take(2) {
                parts.push(format!("{}: {}%", m.name, m.percentage));
            }
        }
        if !parts.is_empty() {
            return format!(" ({})", parts.join(" | "));
        }
    }
    String::new()
}

#[allow(dead_code)]
pub(crate) fn format_countdown_compact(reset_time_str: &str, lang: Lang) -> String {
    if reset_time_str.is_empty() {
        return String::new();
    }
    if let Ok(reset_dt) = chrono::DateTime::parse_from_rfc3339(reset_time_str) {
        let now = chrono::Utc::now();
        let diff = reset_dt.signed_duration_since(now.with_timezone(&reset_dt.timezone()));
        if diff.num_seconds() <= 0 {
            match lang {
                Lang::Zh => "已重置".to_string(),
                Lang::En => "Ready".to_string(),
            }
        } else {
            let hours = diff.num_hours();
            let mins = diff.num_minutes() % 60;
            if hours > 0 {
                format!("{}h", hours)
            } else {
                format!("{}m", mins.max(1))
            }
        }
    } else {
        String::new()
    }
}

pub(crate) fn format_quota_countdown(reset_time_str: &str, lang: Lang) -> String {
    if reset_time_str.is_empty() {
        return String::new();
    }
    if let Ok(reset_dt) = chrono::DateTime::parse_from_rfc3339(reset_time_str) {
        let now = chrono::Utc::now();
        let diff = reset_dt.signed_duration_since(now.with_timezone(&reset_dt.timezone()));
        if diff.num_seconds() <= 0 {
            match lang {
                Lang::Zh => "已重置".to_string(),
                Lang::En => "Ready".to_string(),
            }
        } else {
            let hours = diff.num_hours();
            let mins = diff.num_minutes() % 60;
            match lang {
                Lang::Zh => {
                    if hours > 0 {
                        format!("{}小时", hours)
                    } else {
                        format!("{}分钟", mins.max(1))
                    }
                }
                Lang::En => {
                    if hours > 0 {
                        format!("{}h", hours)
                    } else {
                        format!("{}m", mins.max(1))
                    }
                }
            }
        }
    } else {
        String::new()
    }
}

pub(crate) fn format_countdown(reset_time_str: &str, lang: Lang) -> String {
    if reset_time_str.is_empty() {
        return String::new();
    }
    if let Ok(reset_dt) = chrono::DateTime::parse_from_rfc3339(reset_time_str) {
        let now = chrono::Utc::now();
        let diff = reset_dt.signed_duration_since(now.with_timezone(&reset_dt.timezone()));
        if diff.num_seconds() <= 0 {
            match lang {
                Lang::Zh => "已重置".to_string(),
                Lang::En => "Reset".to_string(),
            }
        } else {
            let hours = diff.num_hours();
            let mins = diff.num_minutes() % 60;
            match lang {
                Lang::Zh => {
                    if hours > 0 {
                        format!("{}小时{}分", hours, mins)
                    } else {
                        format!("{}分", mins.max(1))
                    }
                }
                Lang::En => {
                    if hours > 0 {
                        format!("{}h {}m", hours, mins)
                    } else {
                        format!("{}m", mins.max(1))
                    }
                }
            }
        }
    } else {
        reset_time_str.to_string()
    }
}

pub(crate) fn format_number(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut result = String::new();
    let len = bytes.len();
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(b as char);
    }
    result
}

pub(crate) fn progress_bar(percentage: i32, width: usize) -> String {
    let clamped = percentage.clamp(0, 100);
    let mut filled = ((clamped as f64 / 100.0) * width as f64).round() as usize;
    if clamped > 0 && filled == 0 {
        filled = 1;
    }
    if clamped >= 100 {
        filled = width;
    }
    let empty = width.saturating_sub(filled);

    let color = if clamped > 60 {
        "\x1b[32m" // green
    } else if clamped >= 20 {
        "\x1b[33m" // yellow
    } else {
        "\x1b[31m" // red
    };

    let filled_part = if filled > 0 {
        format!("{}{}\x1b[0m", color, "▰".repeat(filled))
    } else {
        String::new()
    };

    let empty_part = if empty > 0 {
        format!("\x1b[38;2;180;175;165m{}\x1b[0m", "▱".repeat(empty))
    } else {
        String::new()
    };

    format!("{}{}", filled_part, empty_part)
}

fn estimate_model_cost(input: u64, output: u64, cached: u64, model: &str) -> Option<f64> {
    let entry = crate::modules::native_token_stats::LocalTokenModel { model: model.into(), input_tokens: input, output_tokens: output, cached_tokens: cached, total_tokens: input + output + cached, request_count: 0 };
    estimate_cost(&[entry]).0
}
fn estimate_cost(models: &[crate::modules::native_token_stats::LocalTokenModel]) -> (Option<f64>, usize) {
    let pricing = crate::modules::api_pricing::cached_pricing();
    crate::modules::menu_bar_usage::estimate(models, pricing.as_ref().map(|p| p.prices.as_slice()).unwrap_or_default())
}
fn period_cost(summary: &crate::modules::native_token_stats::LocalTokenUsageSummary, period: usize, total: u64, lang: Lang) -> String {
    let models = match period { 0 => &summary.by_model_today, 1 => &summary.by_model_yesterday, 2 => &summary.by_model_3_days, 3 => &summary.by_model_7_days, _ => &summary.by_model };
    if models.is_empty() && total > 0 { return format_cost(None, lang); }
    let (cost, missing) = estimate_cost(models);
    let value = format_cost(cost, lang);
    if cost.is_some() && missing > 0 { match lang { Lang::Zh => format!("{}（部分）", value), Lang::En => format!("{} (partial)", value) } } else { value }
}
fn format_cost(usd: Option<f64>, lang: Lang) -> String {
    let Some(usd) = usd else { return match lang { Lang::Zh => "未计价".into(), Lang::En => "Unpriced".into() }; };
    if usd == 0.0 { "$0.00".into() } else if usd < 0.01 { format!("${:.4}", usd) } else { format!("${:.2}", usd) }
}

fn open_browser(url: &str) {
    let _ = tauri_plugin_opener::open_url(url, None::<&str>);
}

struct AlternateScreenGuard;
impl AlternateScreenGuard {
    fn enter() -> Option<Self> {
        if !super::is_tty() { return None; }
        execute!(io::stdout(), terminal::EnterAlternateScreen, terminal::Clear(terminal::ClearType::All), cursor::MoveTo(0, 0)).ok()?;
        Some(Self)
    }
}
impl Drop for AlternateScreenGuard {
    fn drop(&mut self) { let _ = execute!(io::stdout(), terminal::LeaveAlternateScreen, cursor::Show); }
}

pub(super) struct RawTerminal;
impl RawTerminal {
    pub(super) fn enter() -> Option<Self> {
        if !super::is_tty() { return None; }
        terminal::enable_raw_mode().ok()?;
        // The existing renderer uses normal newlines as well as explicit cursor moves.
        #[cfg(unix)]
        unsafe {
            let mut raw = std::mem::zeroed();
            if libc::tcgetattr(libc::STDIN_FILENO, &mut raw) == 0 {
                raw.c_oflag |= libc::OPOST | libc::ONLCR;
                let _ = libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw);
            }
        }
        Some(Self)
    }
}
impl Drop for RawTerminal {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), cursor::Show);
    }
}

#[derive(Debug, PartialEq)]
pub(super) enum KeyAction { Up, Down, Home, End, MoveUp, MoveDown, Redraw, Enter, Cancel, SelectIndex(usize), Char(char), None }
fn key_action(key: KeyEvent) -> KeyAction {
    if key.kind == KeyEventKind::Release { return KeyAction::None; }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') { return KeyAction::Cancel; }
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        match key.code { KeyCode::Up => return KeyAction::MoveUp, KeyCode::Down => return KeyAction::MoveDown, _ => {} }
    }
    match key.code {
        KeyCode::Up | KeyCode::BackTab | KeyCode::Char('k' | 'K') => KeyAction::Up,
        KeyCode::Down | KeyCode::Tab | KeyCode::Char('j' | 'J') => KeyAction::Down,
        KeyCode::Home => KeyAction::Home,
        KeyCode::End => KeyAction::End,
        KeyCode::Enter | KeyCode::Right => KeyAction::Enter,
        KeyCode::Esc | KeyCode::Left | KeyCode::Char('q' | 'Q') => KeyAction::Cancel,
        KeyCode::Char(c @ '1'..='9') => KeyAction::SelectIndex((c as u8 - b'1') as usize),
        KeyCode::Char(c) => KeyAction::Char(c),
        _ => KeyAction::None,
    }
}
pub(super) fn read_key_action() -> KeyAction {
    loop {
        match event::read() {
            Ok(Event::Key(key)) => return key_action(key),
            Ok(Event::Resize(_, _)) => return KeyAction::Redraw,
            Ok(_) => continue,
            Err(_) => return KeyAction::Cancel,
        }
    }
}

pub enum PromptResult {
    Confirmed(String),
    Cancelled,
}

pub fn prompt_line_with_cancel(prompt: &str, max_chars: usize) -> PromptResult {
    prompt_input(prompt, max_chars, false)
}
fn prompt_input(prompt: &str, max_chars: usize, secret: bool) -> PromptResult {
    print!("{}", prompt);
    let _ = io::stdout().flush();

    if let Some(_raw) = RawTerminal::enter() {
        let mut buffer = Vec::<char>::new();
        let mut position = 0usize;
        loop {
            let key = match event::read() {
                Ok(Event::Key(key)) if key.kind != KeyEventKind::Release => key,
                Ok(_) => continue,
                Err(_) => return PromptResult::Cancelled,
            };
            if key.code == KeyCode::Esc || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c')) {
                println!(); return PromptResult::Cancelled;
            }
            match key.code {
                KeyCode::Enter => { println!(); return PromptResult::Confirmed(buffer.iter().collect::<String>().trim().to_string()); }
                KeyCode::Left => position = position.saturating_sub(1),
                KeyCode::Right => position = (position + 1).min(buffer.len()),
                KeyCode::Home => position = 0,
                KeyCode::End => position = buffer.len(),
                KeyCode::Backspace if position > 0 => { position -= 1; buffer.remove(position); }
                KeyCode::Delete if position < buffer.len() => { buffer.remove(position); }
                KeyCode::Char(c) if !c.is_control() && !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                    if max_chars == 0 || buffer.len() < max_chars {
                        buffer.insert(position, c); position += 1;
                    }
                }
                _ => {}
            }
            // Scroll long fields within one line. Never include secret characters
            // in the rendered string, including while moving the input cursor.
            let prefix = truncate_display_width(prompt, get_terminal_width().saturating_sub(8));
            let available = get_terminal_width().saturating_sub(display_width(&prefix) + 1).max(1);
            let width = |chars: &[char]| if secret { chars.len() } else { display_width(&chars.iter().collect::<String>()) };
            let mut start = 0;
            while start < position && width(&buffer[start..position]) >= available { start += 1; }
            let mut end = position;
            while end < buffer.len() && width(&buffer[start..=end]) <= available { end += 1; }
            let shown = if secret { "*".repeat(end - start) } else { buffer[start..end].iter().collect() };
            print!("\x1b[2K\r{prefix}{shown}");
            let suffix = width(&buffer[position..end]);
            if suffix > 0 { let _ = execute!(io::stdout(), cursor::MoveLeft(suffix as u16)); }
            let _ = io::stdout().flush();
        }
    }

    // Refuse credential entry if a terminal with echo disabled is unavailable.
    if secret { return PromptResult::Cancelled; }
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_ok() {
        PromptResult::Confirmed(line.trim().to_string())
    } else {
        PromptResult::Cancelled
    }
}

pub(super) fn wait_for_key(lang: Lang) {
    let msg = match lang {
        Lang::Zh => "按 Esc 或任意键返回...",
        Lang::En => "Press Esc or any key to return...",
    };
    print!("\n\x1b[2m{}\x1b[0m", msg);
    let _ = io::stdout().flush();
    {
        if let Some(_raw) = RawTerminal::enter() {
            while matches!(read_key_action(), KeyAction::Redraw | KeyAction::None) {}
            print!("\x1b[2K\r");
            let _ = io::stdout().flush();
            return;
        }
    }
    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
}

pub fn select_menu_interactive(title: &str, items: &[&str], initial: usize, lang: Lang) -> Option<usize> {
    if items.is_empty() {
        return None;
    }

    {
        if let Some(_raw) = RawTerminal::enter() {
            let mut selected = initial.min(items.len() - 1);
            let mut stdout = io::stdout();

            print!("\x1b[?25l");
            let _ = stdout.flush();

            let hint = match lang {
                Lang::Zh => "↑↓ / Tab 选择  Enter/→ 确认  Esc/←/0 返回",
                Lang::En => "Up/Down/Tab select  Enter/Right open  Esc/Left/0 back",
            };

            let mut rendered_lines = 0;
            let mut render = |sel: usize, first: bool| {
                let mut out = io::stdout();
                if !first {
                    let _ = write!(out, "\x1b[{}A", rendered_lines);
                }
                let _ = write!(out, "\x1b[J");
                let width = get_terminal_width().saturating_sub(2).max(8);
                let _ = writeln!(out, "\x1b[2K\r\x1b[1m{}\x1b[0m", truncate_display_width(title, width));
                let _ = writeln!(out, "\x1b[2K\r\x1b[90m{}\x1b[0m", truncate_display_width(hint, width));
                let visible = get_terminal_height().saturating_sub(10).max(3).min(items.len());
                let start = sel.saturating_sub(visible - 1);
                for (i, item) in items.iter().enumerate().skip(start).take(visible) {
                    let item = truncate_display_width(item, width.saturating_sub(4));
                    if i == sel {
                        let _ = writeln!(out, "\x1b[2K\r  \x1b[1;36m➤\x1b[0m \x1b[1m{}\x1b[0m", item);
                    } else {
                        let _ = writeln!(out, "\x1b[2K\r    {}", item);
                    }
                }
                rendered_lines = visible + 2;
                let _ = out.flush();
            };

            render(selected, true);

            loop {
                match read_key_action() {
                    KeyAction::Up => {
                        selected = if selected > 0 {
                            selected - 1
                        } else {
                            items.len() - 1
                        };
                        render(selected, false);
                    }
                    KeyAction::Down => {
                        selected = if selected + 1 < items.len() {
                            selected + 1
                        } else {
                            0
                        };
                        render(selected, false);
                    }
                    KeyAction::Home => { selected = 0; render(selected, false); }
                    KeyAction::End => { selected = items.len() - 1; render(selected, false); }
                    KeyAction::Redraw => { render(selected, false); }
                    KeyAction::SelectIndex(idx) => {
                        let digit = (idx + 1).to_string();
                        let prefix = format!("{}.", digit);
                        for (i, item) in items.iter().enumerate() {
                            if strip_ansi(item).trim_start().starts_with(&prefix) {
                                print!("\x1b[?25h");
                                let _ = stdout.flush();
                                return Some(i);
                            }
                        }
                        if idx < items.len() {
                            print!("\x1b[?25h");
                            let _ = stdout.flush();
                            return Some(idx);
                        }
                    }
                    KeyAction::Char('0') => {
                        for (i, item) in items.iter().enumerate() {
                            if strip_ansi(item).trim_start().starts_with("0.") {
                                print!("\x1b[?25h");
                                let _ = stdout.flush();
                                return Some(i);
                            }
                        }
                        print!("\x1b[?25h");
                        let _ = stdout.flush();
                        return None;
                    }
                    KeyAction::Char(ch) => {
                        let upper = ch.to_ascii_uppercase();
                        let prefix = format!("{}.", upper);
                        for (i, item) in items.iter().enumerate() {
                            if strip_ansi(item).trim_start().starts_with(&prefix) {
                                print!("\x1b[?25h");
                                let _ = stdout.flush();
                                return Some(i);
                            }
                        }
                    }
                    KeyAction::Enter => {
                        print!("\x1b[?25h");
                        let _ = stdout.flush();
                        return Some(selected);
                    }
                    KeyAction::Cancel => {
                        print!("\x1b[?25h");
                        let _ = stdout.flush();
                        return None;
                    }
                    _ => {}
                }
            }
        }
    }

    println!("{}", title);
    for item in items.iter() {
        println!("  {}", item);
    }
    let prompt_msg = match lang {
        Lang::Zh => format!("请输入选项序号 [1-{}]: ", items.len()),
        Lang::En => format!("Select option [1-{}]: ", items.len()),
    };
    print!("{}", prompt_msg);
    let _ = io::stdout().flush();
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        let trimmed = input.trim();
        if let Ok(num) = trimmed.parse::<usize>() {
            if num >= 1 && num <= items.len() {
                return Some(num - 1);
            }
        }
    }
    None
}

fn build_accounts_table(accounts: &[AccountView], lang: Lang, selected_idx: Option<usize>) -> Table {
    let headers = match lang {
        Lang::Zh => vec!["邮箱", "备注", "Gemini", "Claude/GPT", "状态"],
        Lang::En => vec!["Account / Email", "Label", "Gemini", "Claude/GPT", "Status"],
    };
    let mut table = Table::new(headers);

    for (i, acc) in accounts.iter().enumerate() {
        let is_selected = selected_idx == Some(i);
        let prefix = if accounts.len() >= 10 {
            if is_selected {
                format!("➤{:>2}. ", i + 1)
            } else {
                format!(" {:>2}. ", i + 1)
            }
        } else {
            if is_selected {
                format!("➤{}. ", i + 1)
            } else {
                format!(" {}. ", i + 1)
            }
        };

        let cur = if acc.is_current { " *" } else { "" };
        let email_text = format!("{}{}{}", prefix, acc.email, cur);
        let label_text = match &acc.custom_label {
            Some(l) if !l.trim().is_empty() => l.trim().to_string(),
            _ => "-".to_string(),
        };

        let default_none = match lang {
            Lang::Zh => "5小时: 暂无\n周限:  暂无".to_string(),
            Lang::En => "5-Hour: None\nWeekly: None".to_string(),
        };
        let mut gemini_quota = default_none.clone();
        let mut claude_quota = default_none;

        if let Some(q) = &acc.quota {
            if q.is_forbidden {
                gemini_quota = match lang {
                    Lang::Zh => "5小时: 受限\n周限:  需重登".into(),
                    Lang::En => "5-Hour: Forbidden\nWeekly: Relogin".into(),
                };
                claude_quota = gemini_quota.clone();
            } else if let Some(groups) = &q.quota_groups {
                for g in groups {
                    let weekly_b = g.buckets.iter().find(|b| b.bucket_id.contains("week") || b.window.contains("week"));
                    let five_h_b = g.buckets.iter().find(|b| b.bucket_id.contains("5h") || b.window.contains("5h"));

                    let five_h_str = match five_h_b {
                        Some(fb) => {
                            let pct = (fb.remaining_fraction * 100.0).round() as i32;
                            let cd = format_quota_countdown(&fb.reset_time, lang);
                            let prefix = match lang {
                                Lang::Zh => "5小时: ",
                                Lang::En => "5-Hour: ",
                            };
                            if cd.is_empty() {
                                format!("{}{}%", prefix, pct)
                            } else {
                                format!("{}{}% ({})", prefix, pct, cd)
                            }
                        }
                        None => match lang {
                            Lang::Zh => "5小时: 未知".to_string(),
                            Lang::En => "5-Hour: Unknown".to_string(),
                        },
                    };

                    let weekly_str = match weekly_b {
                        Some(wb) => {
                            let pct = (wb.remaining_fraction * 100.0).round() as i32;
                            let cd = format_quota_countdown(&wb.reset_time, lang);
                            let prefix = match lang {
                                Lang::Zh => "周限:  ",
                                Lang::En => "Weekly: ",
                            };
                            if cd.is_empty() {
                                format!("{}{}%", prefix, pct)
                            } else {
                                format!("{}{}% ({})", prefix, pct, cd)
                            }
                        }
                        None => match lang {
                            Lang::Zh => "周限:  未知".to_string(),
                            Lang::En => "Weekly: Unknown".to_string(),
                        },
                    };

                    let text = format!("{}\n{}", five_h_str, weekly_str);
                    if g.display_name.contains("Gemini") {
                        gemini_quota = text;
                    } else if g.display_name.contains("Claude") || g.display_name.contains("GPT") {
                        claude_quota = text;
                    }
                }
            } else {
                for m in &q.models {
                    let text = match lang {
                        Lang::Zh => format!("5小时: {}%\n周限:  -", m.percentage),
                        Lang::En => format!("5-Hour: {}%\nWeekly: -", m.percentage),
                    };
                    if m.name.to_lowercase().contains("gemini") {
                        gemini_quota = text;
                    } else if m.name.to_lowercase().contains("claude") {
                        claude_quota = text;
                    }
                }
            }
        }

        let status = if acc.disabled {
            match lang {
                Lang::Zh => "\x1b[31m已禁用\x1b[0m",
                Lang::En => "\x1b[31mDisabled\x1b[0m",
            }
        } else if acc.validation_blocked {
            match lang {
                Lang::Zh => "\x1b[33m需验证\x1b[0m",
                Lang::En => "\x1b[33mVerify\x1b[0m",
            }
        } else if acc.is_current {
            match lang {
                Lang::Zh => "\x1b[32m当前选择\x1b[0m",
                Lang::En => "\x1b[32mSelected\x1b[0m",
            }
        } else {
            match lang {
                Lang::Zh => "正常",
                Lang::En => "Normal",
            }
        };

        let (row_email, row_label, row_gemini, row_claude) = if is_selected {
            (
                format!("\x1b[1;36m{}\x1b[0m", email_text),
                format!("\x1b[1;36m{}\x1b[0m", label_text),
                gemini_quota.lines().map(|l| format!("\x1b[1;36m{}\x1b[0m", l)).collect::<Vec<_>>().join("\n"),
                claude_quota.lines().map(|l| format!("\x1b[1;36m{}\x1b[0m", l)).collect::<Vec<_>>().join("\n"),
            )
        } else {
            (email_text, label_text, gemini_quota, claude_quota)
        };

        table.add_row(vec![
            row_email,
            row_label,
            row_gemini,
            row_claude,
            status.to_string(),
        ]);
    }
    table
}

pub fn select_account_interactive<'a>(
    accounts: &'a [AccountView],
    lang: Lang,
) -> Option<&'a AccountView> {
    if accounts.is_empty() {
        return None;
    }

    {
        if let Some(_raw) = RawTerminal::enter() {
            let mut selected = accounts.iter().position(|a| a.is_current).unwrap_or(0);
            let mut stdout = io::stdout();

            print!("\x1b[?25l");
            let _ = stdout.flush();

            let prompt_text = match lang {
                Lang::Zh => "? 请选择账号 (↑/↓ 移动  |  回车确认  |  数字键直选  |  Esc/0 取消):",
                Lang::En => "? Select account (↑/↓ Navigate  |  Enter Select  |  Numbers  |  Esc/0 Cancel):",
            };

            let render = |sel: usize, initial: bool| {
                let mut out = io::stdout();
                let table_str = build_accounts_table(accounts, lang, Some(sel)).render();
                let table_lines = table_str.lines().count();
                let total_lines = table_lines + 1;

                if !initial {
                    let _ = write!(out, "\x1b[{}A", total_lines);
                }

                for line in table_str.lines() {
                    let _ = writeln!(out, "\x1b[2K\r{}", line);
                }
                let _ = writeln!(out, "\x1b[2K\r\x1b[1m{}\x1b[0m", prompt_text);
                let _ = write!(out, "\x1b[2K\r");
                let _ = out.flush();
            };

            render(selected, true);

            loop {
                match read_key_action() {
                    KeyAction::Up => {
                        selected = if selected > 0 {
                            selected - 1
                        } else {
                            accounts.len() - 1
                        };
                        render(selected, false);
                    }
                    KeyAction::Down => {
                        selected = if selected + 1 < accounts.len() {
                            selected + 1
                        } else {
                            0
                        };
                        render(selected, false);
                    }
                    KeyAction::SelectIndex(idx) => {
                        if idx < accounts.len() {
                            print!("\x1b[?25h");
                            let _ = stdout.flush();
                            return Some(&accounts[idx]);
                        }
                    }
                    KeyAction::Home => { selected = 0; render(selected, false); }
                    KeyAction::End => { selected = accounts.len() - 1; render(selected, false); }
                    KeyAction::Redraw => { render(selected, false); }
                    KeyAction::Char('0') | KeyAction::Cancel => {
                        print!("\x1b[?25h");
                        let _ = stdout.flush();
                        return None;
                    }
                    KeyAction::Enter => {
                        print!("\x1b[?25h");
                        let _ = stdout.flush();
                        return Some(&accounts[selected]);
                    }
                    _ => {}
                }
            }
        }
    }

    let table_str = build_accounts_table(accounts, lang, None).render();
    print!("{}", table_str);
    let prompt_msg = match lang {
        Lang::Zh => format!("请输入账号序号 [1-{}]: ", accounts.len()),
        Lang::En => format!("Select account number [1-{}]: ", accounts.len()),
    };
    print!("{}", prompt_msg);
    let _ = io::stdout().flush();
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        let trimmed = input.trim();
        if let Ok(num) = trimmed.parse::<usize>() {
            if num >= 1 && num <= accounts.len() {
                return Some(&accounts[num - 1]);
            }
        }
    }
    None
}


pub fn format_token_stats_human(
    summary: &crate::modules::native_token_stats::LocalTokenUsageSummary,
    lang: Lang,
) -> String {
    let mut out = String::new();

    let headers = match lang {
        Lang::Zh => vec!["周期", "总计 Token", "输入", "输出", "缓存率", "预估费用"],
        Lang::En => vec!["Period", "Total Tokens", "Input", "Output", "Cache Hit", "Est. Cost"],
    };

    let mut table = Table::new(headers);
    for col in 1..=5 {
        table.set_align_right(col, true);
    }

    let periods = match lang {
        Lang::Zh => [
            ("今日", &summary.today),
            ("昨日", &summary.yesterday),
            ("近 3 天", &summary.last_3_days),
            ("近 7 天", &summary.last_7_days),
            ("近 30 天", &summary.last_30_days),
        ],
        Lang::En => [
            ("Today", &summary.today),
            ("Yesterday", &summary.yesterday),
            ("Last 3 Days", &summary.last_3_days),
            ("Last 7 Days", &summary.last_7_days),
            ("Last 30 Days", &summary.last_30_days),
        ],
    };

    for (i, (name, row)) in periods.iter().enumerate() {
        let total_in = row.input_tokens + row.cached_tokens;
        let hit_rate = if total_in > 0 {
            format!("{:.1}%", (row.cached_tokens as f64 / total_in as f64) * 100.0)
        } else {
            "0.0%".into()
        };

        let cost = period_cost(&summary, i, row.total_tokens, lang);

        table.add_row(vec![
            name.to_string(),
            format_number(row.total_tokens),
            format_number(row.input_tokens),
            format_number(row.output_tokens),
            hit_rate,
            cost,
        ]);
    }

    out.push_str(&table.render());

    if !summary.by_model_today.is_empty() {
        let title = match lang {
            Lang::Zh => "\n【今日各模型用量明细】\n",
            Lang::En => "\n[Model Breakdown: Today]\n",
        };
        out.push_str(title);

        let m_headers = match lang {
            Lang::Zh => vec!["模型", "总计 Token", "输入", "输出", "缓存", "预期费用"],
            Lang::En => vec!["Model", "Total Tokens", "Input", "Output", "Cached", "Est. Cost"],
        };
        let mut m_table = Table::new(m_headers);
        for col in 1..=5 {
            m_table.set_align_right(col, true);
        }
        for m in &summary.by_model_today {
            let cost = estimate_model_cost(m.input_tokens, m.output_tokens, m.cached_tokens, &m.model);
            m_table.add_row(vec![
                m.model.clone(),
                format_number(m.total_tokens),
                format_number(m.input_tokens),
                format_number(m.output_tokens),
                format_number(m.cached_tokens),
                format_cost(cost, lang),
            ]);
        }
        out.push_str(&m_table.render());
    }

    if !summary.by_model.is_empty() {
        let title = match lang {
            Lang::Zh => "\n【近 30 天主要模型用量排行】\n",
            Lang::En => "\n[Top Models: Last 30 Days]\n",
        };
        out.push_str(title);

        let m_headers = match lang {
            Lang::Zh => vec!["模型", "总计 Token", "请求次数", "预期费用"],
            Lang::En => vec!["Model", "Total Tokens", "Requests", "Est. Cost"],
        };
        let mut m_table = Table::new(m_headers);
        m_table.set_align_right(1, true);
        m_table.set_align_right(2, true);
        m_table.set_align_right(3, true);
        for m in summary.by_model.iter().take(5) {
            let cost = estimate_model_cost(m.input_tokens, m.output_tokens, m.cached_tokens, &m.model);
            m_table.add_row(vec![
                m.model.clone(),
                format_number(m.total_tokens),
                format_number(m.request_count),
                format_cost(cost, lang),
            ]);
        }
        out.push_str(&m_table.render());
    }

    let footer = match lang {
        Lang::Zh => format!(
            "\n数据来源: 扫描 {} 个本地 SQLite 数据库 | 累计对话记录: {} 条\n",
            summary.databases_scanned, summary.generations_scanned
        ),
        Lang::En => format!(
            "\nSource: Scanned {} local SQLite databases | Total records: {}\n",
            summary.databases_scanned, summary.generations_scanned
        ),
    };
    out.push_str(&footer);

    out
}

pub fn run_interactive_dashboard(root: &Path) -> Result<(), CliError> {
    let _alt_screen = AlternateScreenGuard::enter();

    loop {
        let lang = Lang::current(root);
        let snapshot = Snapshot::read(root).unwrap_or(Snapshot {
            accounts: vec![],
            current_target: None,
        });

        // Clear screen and show minimal, professional header
        print!("\x1b[2J\x1b[H");
        match lang {
            Lang::Zh => println!(
                "\x1b[1magy-switch 账号管理\x1b[0m · \x1b[36magy-switch v{}\x1b[0m",
                env!("CARGO_PKG_VERSION")
            ),
            Lang::En => println!(
                "\x1b[1magy-switch\x1b[0m · \x1b[36mv{}\x1b[0m",
                env!("CARGO_PKG_VERSION")
            ),
        }

        if let Ok(curr) = snapshot.current() {
            let target = snapshot.current_target.as_deref().unwrap_or("app");
            let label = match &curr.custom_label {
                Some(l) if !l.trim().is_empty() => format!(" ({})", l),
                _ => String::new(),
            };
            let qb = quota_brief(curr.quota.as_ref(), lang);
            let quota_part = if qb.is_empty() {
                String::new()
            } else {
                format!("  | {}", qb.trim_start_matches(" (").trim_end_matches(')'))
            };
            match lang {
                Lang::Zh => println!(
                    "\x1b[1m当前选择:\x1b[0m \x1b[1;32m{}{}\x1b[0m [{}] {}\n",
                    curr.email, label, target, quota_part
                ),
                Lang::En => println!(
                    "\x1b[1mSelected:\x1b[0m \x1b[1;32m{}{}\x1b[0m [{}] {}\n",
                    curr.email, label, target, quota_part
                ),
            }
        } else {
            match lang {
                Lang::Zh => println!("\x1b[1m当前选择:\x1b[0m \x1b[33m未设置 / 暂无账号\x1b[0m\n"),
                Lang::En => println!("\x1b[1mSelected:\x1b[0m \x1b[33mNone / No accounts\x1b[0m\n"),
            }
        }

        let (title, menu_items) = match lang {
            Lang::Zh => (
                "选择功能:",
                vec![
                    "1. 账号与配额    切换账号、查看各模型配额明细、修改备注与启停管理",
                    "2. 用量统计      本地 Token 消耗、预期费用与模型排行",
                    "3. 刷新配额      联网同步 Google API 最新额度",
                    "4. 添加账号      通过 Google OAuth 授权绑定新账号",
                    "5. 环境状态      关联应用与本地存储状态",
                    "6. 策略与排序    智能切换设置、候选顺序与账号排序",
                    "7. 检查更新      查询最新稳定版",
                    "0. 退出控制台    退出当前工具",
                ],
            ),
            Lang::En => (
                "Select a section:",
                vec![
                    "1. Accounts & Quotas   Switch account, inspect model quotas, edit labels & manage",
                    "2. Statistics          Local token usage, estimated cost & model rankings",
                    "3. Refresh             Fetch live quotas from Google API",
                    "4. Add Account         Authorize new Google account via OAuth",
                    "5. Status              Inspect linked applications and storage",
                    "6. Settings & Order    Smart switching, candidates and account order",
                    "7. Check for Updates   Query the latest stable release",
                    "0. Exit                Quit agy-switch",
                ],
            ),
        };

        let choice = select_menu_interactive(title, &menu_items, 0, lang);

        match choice {
            Some(0) => show_accounts_and_quotas_hub(root, lang),
            Some(1) => show_token_statistics(lang),
            Some(2) => show_refresh_quotas(root, lang),
            Some(3) => show_add_account(root, lang),
            Some(4) => show_system_status(&snapshot, root, lang),
            Some(5) => super::workflows::show_settings(root, lang),
            Some(6) => super::workflows::show_updates(lang),
            Some(7) | None => {
                let exit_msg = match lang {
                    Lang::Zh => "\n已退出 agy-switch 控制台。\n",
                    Lang::En => "\nExited agy-switch.\n",
                };
                println!("{}", exit_msg);
                break;
            }
            _ => {}
        }
    }
    Ok(())
}

enum HubAction {
    Actions(usize),
    Switch(usize),
    ViewDetails(usize),
    EditLabel(usize),
    ToggleStatus(usize),
    Delete(usize),
    Back,
}

fn select_account_hub_action(
    accounts: &[AccountView],
    selected: &mut usize,
    lang: Lang,
) -> HubAction {
    {
        if let Some(_raw) = RawTerminal::enter() {
            let mut stdout = io::stdout();

            let _ = write!(stdout, "\x1b[?25l");
            let _ = stdout.flush();

            let note = match lang {
                Lang::Zh => "* 标注为当前选择账号",
                Lang::En => "* marks the locally selected account",
            };
            let prompt_text = match lang {
                Lang::Zh => "操作: (↑/↓ 移动  |  回车 操作菜单  |  S 切换  |  V 详情  |  R 备注  |  T 启/禁  |  X 删除  |  Esc/0 返回)",
                Lang::En => "Action: (↑/↓ Move  |  Enter Actions  |  S Switch  |  V Details  |  R Label  |  T Toggle  |  X Delete  |  Esc/0 Back)",
            };

            let render = |sel: usize, initial: bool| {
                let mut out = io::stdout();
                let table_str = build_accounts_table(accounts, lang, Some(sel)).render();
                let table_lines = table_str.lines().count();
                let total_lines = table_lines + 3;

                if !initial {
                    let _ = write!(out, "\x1b[{}A", total_lines);
                }

                for line in table_str.lines() {
                    let _ = writeln!(out, "\x1b[2K\r{}", line);
                }
                let _ = writeln!(out, "\x1b[2K\r\x1b[90m{}\x1b[0m\n", note);
                let _ = writeln!(out, "\x1b[2K\r\x1b[1m{}\x1b[0m", prompt_text);
                let _ = write!(out, "\x1b[2K\r");
                let _ = out.flush();
            };

            render(*selected, true);

            loop {
                match read_key_action() {
                    KeyAction::Up => {
                        *selected = if *selected > 0 {
                            *selected - 1
                        } else {
                            accounts.len() - 1
                        };
                        render(*selected, false);
                    }
                    KeyAction::Down => {
                        *selected = if *selected + 1 < accounts.len() {
                            *selected + 1
                        } else {
                            0
                        };
                        render(*selected, false);
                    }
                    KeyAction::SelectIndex(idx) => {
                        if idx < accounts.len() {
                            *selected = idx;
                            render(*selected, false);
                        }
                    }
                    KeyAction::Enter => {
                        print!("\x1b[?25h");
                        let _ = stdout.flush();
                        return HubAction::Actions(*selected);
                    }
                    KeyAction::Home => { *selected = 0; render(*selected, false); }
                    KeyAction::End => { *selected = accounts.len() - 1; render(*selected, false); }
                    KeyAction::Redraw => { render(*selected, false); }
                    KeyAction::Char('s' | 'S') => return HubAction::Switch(*selected),
                    KeyAction::Char('v') | KeyAction::Char('V') | KeyAction::Char(' ') => {
                        print!("\x1b[?25h");
                        let _ = stdout.flush();
                        return HubAction::ViewDetails(*selected);
                    }
                    KeyAction::Char('r') | KeyAction::Char('R') => {
                        print!("\x1b[?25h");
                        let _ = stdout.flush();
                        return HubAction::EditLabel(*selected);
                    }
                    KeyAction::Char('t') | KeyAction::Char('T') => {
                        print!("\x1b[?25h");
                        let _ = stdout.flush();
                        return HubAction::ToggleStatus(*selected);
                    }
                    KeyAction::Char('x') | KeyAction::Char('X') => {
                        print!("\x1b[?25h");
                        let _ = stdout.flush();
                        return HubAction::Delete(*selected);
                    }
                    KeyAction::Char('0') | KeyAction::Cancel => {
                        print!("\x1b[?25h");
                        let _ = stdout.flush();
                        return HubAction::Back;
                    }
                    _ => {}
                }
            }
        }
    }

    print!("\x1b[?25h");
    let _ = io::stdout().flush();
    HubAction::Back
}

fn show_accounts_and_quotas_hub(root: &Path, lang: Lang) {
    let mut snapshot = match Snapshot::read(root) {
        Ok(s) => s,
        Err(_) => Snapshot {
            accounts: Vec::new(),
            current_target: None,
        },
    };

    if snapshot.accounts.is_empty() {
        let msg = match lang {
            Lang::Zh => "\n\x1b[33m暂无已保存账号，请使用主菜单 [4] 添加 Google 账号。\x1b[0m",
            Lang::En => "\n\x1b[33mNo saved accounts. Use [4] in main menu to add a Google account.\x1b[0m",
        };
        println!("{}", msg);
        wait_for_key(lang);
        return;
    }

    let mut selected = snapshot.accounts.iter().position(|a| a.is_current).unwrap_or(0);

    loop {
        if snapshot.accounts.is_empty() {
            let msg = match lang {
                Lang::Zh => "\n\x1b[33m暂无已保存账号。\x1b[0m",
                Lang::En => "\n\x1b[33mNo saved accounts.\x1b[0m",
            };
            println!("{}", msg);
            wait_for_key(lang);
            break;
        }

        if selected >= snapshot.accounts.len() {
            selected = snapshot.accounts.len().saturating_sub(1);
        }

        print!("\x1b[2J\x1b[H");
        let header = match lang {
            Lang::Zh => "账号与配额中心",
            Lang::En => "Accounts & Quotas Hub",
        };
        println!("\x1b[1m{}\x1b[0m\n", header);

        let action = match select_account_hub_action(&snapshot.accounts, &mut selected, lang) {
            HubAction::Actions(idx) => {
                print!("\x1b[2J\x1b[H");
                println!("\x1b[1m{}\x1b[0m\n", super::output::terminal_text(&snapshot.accounts[idx].email));
                let (title, items) = match lang {
                    Lang::Zh => ("账号操作", vec!["1. 查看配额", "2. 切换账号", "3. 修改备注", "4. 启用或禁用", "5. 删除账号", "0. 返回"]),
                    Lang::En => ("Account actions", vec!["1. View quotas", "2. Switch account", "3. Edit label", "4. Enable or disable", "5. Delete account", "0. Back"]),
                };
                match select_menu_interactive(title, &items, 0, lang) {
                    Some(0) => HubAction::ViewDetails(idx), Some(1) => HubAction::Switch(idx),
                    Some(2) => HubAction::EditLabel(idx), Some(3) => HubAction::ToggleStatus(idx),
                    Some(4) => HubAction::Delete(idx), _ => continue,
                }
            }
            action => action,
        };

        match action {
            HubAction::Actions(_) => unreachable!(),
            HubAction::Switch(idx) => {
                let target_acc = &snapshot.accounts[idx];
                print!("\x1b[2J\x1b[H");
                let target_header = match lang {
                    Lang::Zh => format!("已选择账号: \x1b[1;32m{}\x1b[0m\n", target_acc.email),
                    Lang::En => format!("Selected account: \x1b[1;32m{}\x1b[0m\n", target_acc.email),
                };
                println!("{}", target_header);

                let wait_msg = match lang {
                    Lang::Zh => format!("\n正在切换至 \x1b[1;32m{}\x1b[0m 并全域同步会话...", target_acc.email),
                    Lang::En => format!("\nSwitching to \x1b[1;32m{}\x1b[0m and synchronizing globally...", target_acc.email),
                };
                println!("{}", wait_msg);

                let runtime = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = e;
                        let err_msg = match lang {
                            Lang::Zh => format!("\x1b[31m启动异步运行时失败: {}\x1b[0m", e),
                            Lang::En => format!("\x1b[31mFailed to start async runtime: {}\x1b[0m", e),
                        };
                        println!("{}", err_msg);
                        wait_for_key(lang);
                        continue;
                    }
                };

                let target_ide = None;

                match runtime.block_on(crate::modules::account::switch_account(
                    &target_acc.id,
                    target_ide,
                    &HeadlessIntegration,
                )) {
                    Ok(_) => {
                        let succ_msg = match lang {
                            Lang::Zh => format!(
                                "\x1b[1;32m✓ 账号切换成功: {}\x1b[0m (全域智能同步)\n提示: 已打开的桌面客户端已自动刷新；若使用已打开的 VS Code 请在命令面板执行 Reload Window。\n",
                                target_acc.email
                            ),
                            Lang::En => format!(
                                "\x1b[1;32m✓ Successfully switched to: {}\x1b[0m (Global smart sync)\nHint: Active clients were refreshed. In open VS Code windows, run 'Reload Window'.\n",
                                target_acc.email
                            ),
                        };
                        println!("{}", succ_msg);
                        wait_for_key(lang);
                        if let Ok(reloaded) = Snapshot::read(root) {
                            snapshot = reloaded;
                        }
                    }
                    Err(err) => {
                        let fail_msg = match lang {
                            Lang::Zh => format!("\x1b[31m账号切换失败: {}\x1b[0m", err),
                            Lang::En => format!("\x1b[31mAccount switch failed: {}\x1b[0m", err),
                        };
                        println!("{}", fail_msg);
                        wait_for_key(lang);
                    }
                }
            }
            HubAction::ViewDetails(idx) => {
                show_single_account_quota(&snapshot.accounts[idx], lang);
            }
            HubAction::EditLabel(idx) => {
                let target_acc = &snapshot.accounts[idx];
                print!("\x1b[2J\x1b[H");
                let edit_header = match lang {
                    Lang::Zh => format!("修改账号备注 · \x1b[1;36m{}\x1b[0m\n", target_acc.email),
                    Lang::En => format!("Edit Account Label · \x1b[1;36m{}\x1b[0m\n", target_acc.email),
                };
                println!("{}", edit_header);

                let cur_label = target_acc.custom_label.as_deref().unwrap_or(match lang {
                    Lang::Zh => "无",
                    Lang::En => "None",
                });
                let cur_info = match lang {
                    Lang::Zh => format!("当前备注: {}\n", cur_label),
                    Lang::En => format!("Current Label: {}\n", cur_label),
                };
                println!("{}", cur_info);

                let tip = match lang {
                    Lang::Zh => "\x1b[90m(操作: 直接输入后按 [回车] 确定  |  输入为空按回车清空备注  |  按 [Esc] 取消返回)\x1b[0m\n",
                    Lang::En => "\x1b[90m(Action: Type and press [Enter] to save  |  Empty to clear  |  Press [Esc] to cancel)\x1b[0m\n",
                };
                println!("{}", tip);

                let prompt = match lang {
                    Lang::Zh => "请输入新备注 (最多15字): ",
                    Lang::En => "Enter new label (max 15 chars): ",
                };

                match prompt_line_with_cancel(prompt, 15) {
                    PromptResult::Confirmed(new_label) => {
                        let label_opt = if new_label.is_empty() {
                            None
                        } else {
                            Some(new_label)
                        };
                        if let Ok(mut acc) = crate::modules::account::load_account(&target_acc.id) {
                            acc.custom_label = label_opt;
                            let _ = crate::modules::account::save_account(&acc);
                            if let Ok(reloaded) = Snapshot::read(root) {
                                snapshot = reloaded;
                            }
                        }
                    }
                    PromptResult::Cancelled => {
                        // User pressed Esc or Ctrl+C, discard changes and return cleanly to table
                    }
                }
            }
            HubAction::ToggleStatus(idx) => {
                let target_acc = &snapshot.accounts[idx];
                if let Ok(mut acc) = crate::modules::account::load_account(&target_acc.id) {
                    acc.disabled = !acc.disabled;
                    if acc.disabled {
                        acc.disabled_at = Some(chrono::Utc::now().timestamp());
                        acc.disabled_reason = Some("Manually disabled via CLI".to_string());
                    } else {
                        acc.disabled_at = None;
                        acc.disabled_reason = None;
                    }
                    let _ = crate::modules::account::save_account(&acc);
                    if let Ok(reloaded) = Snapshot::read(root) {
                        snapshot = reloaded;
                    }
                }
            }
            HubAction::Delete(idx) => {
                let target_acc = &snapshot.accounts[idx];
                print!("\x1b[2J\x1b[H");
                let del_header = match lang {
                    Lang::Zh => format!("删除账号 · \x1b[1;31m{}\x1b[0m\n", target_acc.email),
                    Lang::En => format!("Delete Account · \x1b[1;31m{}\x1b[0m\n", target_acc.email),
                };
                println!("{}", del_header);

                let warn_text = match lang {
                    Lang::Zh => "警告: 此操作将永久移除该账号的本地凭证与配置！\n",
                    Lang::En => "Warning: This will permanently remove local credentials & settings!\n",
                };
                println!("{}", warn_text);

                let confirm_prompt = match lang {
                    Lang::Zh => "确定要永久删除该账号吗？(输入 y 后按回车确认，按 Esc 取消): ",
                    Lang::En => "Permanently delete this account? (Type y and press Enter to confirm; Esc cancels): ",
                };
                match prompt_line_with_cancel(confirm_prompt, 10) {
                    PromptResult::Confirmed(ans) if ans.eq_ignore_ascii_case("y") || ans.eq_ignore_ascii_case("yes") => {
                        match crate::modules::account::delete_account(&target_acc.id) {
                            Ok(_) => {
                                let del_msg = match lang {
                                    Lang::Zh => "\x1b[1;32m✓ 账号已成功删除\x1b[0m",
                                    Lang::En => "\x1b[1;32m✓ Account deleted successfully\x1b[0m",
                                };
                                println!("{}", del_msg);
                                wait_for_key(lang);
                                if let Ok(reloaded) = Snapshot::read(root) {
                                    snapshot = reloaded;
                                }
                                if snapshot.accounts.is_empty() {
                                    break;
                                }
                            }
                            Err(e) => {
                                println!("\x1b[31mDelete error: {}\x1b[0m", e);
                                wait_for_key(lang);
                            }
                        }
                    }
                    _ => {
                        // Cancelled!
                    }
                }
            }
            HubAction::Back => break,
        }
    }
}

fn show_single_account_quota(acc: &AccountView, lang: Lang) {
    print!("\x1b[2J\x1b[H");
    let title = match lang {
        Lang::Zh => format!("账号配额详情 · {}", acc.email),
        Lang::En => format!("Quota Details · {}", acc.email),
    };
    println!("\x1b[1m{}\x1b[0m\n", title);

    let status = if acc.disabled {
        match lang {
            Lang::Zh => "\x1b[31m已禁用\x1b[0m",
            Lang::En => "\x1b[31mDisabled\x1b[0m",
        }
    } else if acc.validation_blocked {
        match lang {
            Lang::Zh => "\x1b[33m需安全验证\x1b[0m",
            Lang::En => "\x1b[33mVerification Required\x1b[0m",
        }
    } else if acc.is_current {
        match lang {
            Lang::Zh => "\x1b[32m当前选择\x1b[0m",
            Lang::En => "\x1b[32mSelected\x1b[0m",
        }
    } else {
        match lang {
            Lang::Zh => "正常",
            Lang::En => "Normal",
        }
    };

    let label_str = acc.custom_label.as_deref().unwrap_or("-");
    let tier_str = acc
        .quota
        .as_ref()
        .and_then(|q| q.subscription_tier.as_deref())
        .unwrap_or("-");

    let meta_headers = match lang {
        Lang::Zh => vec!["属性", "内容值"],
        Lang::En => vec!["Property", "Value"],
    };
    let mut meta_table = Table::new(meta_headers);
    let meta_rows = match lang {
        Lang::Zh => vec![
            vec!["邮箱地址".into(), acc.email.clone()],
            vec!["账号标识".into(), acc.id.clone()],
            vec!["备注标签".into(), label_str.into()],
            vec!["当前状态".into(), status.into()],
            vec!["订阅级别".into(), tier_str.into()],
        ],
        Lang::En => vec![
            vec!["Email".into(), acc.email.clone()],
            vec!["Account ID".into(), acc.id.clone()],
            vec!["Label".into(), label_str.into()],
            vec!["Status".into(), status.into()],
            vec!["Plan Tier".into(), tier_str.into()],
        ],
    };
    for r in meta_rows {
        meta_table.add_row(r);
    }
    print!("{}", meta_table.render());

    if let Some(q) = &acc.quota {
        if q.is_forbidden {
            let warn = match lang {
                Lang::Zh => "\n\x1b[31m[警告] 账号配额访问受限 (Forbidden)，可能需重新授权登录。\x1b[0m",
                Lang::En => "\n\x1b[31m[Warning] Quota access is forbidden; please re-authorize account.\x1b[0m",
            };
            println!("{}", warn);
        }

        let q_headers = match lang {
            Lang::Zh => vec!["配额窗口 / 模型", "余量", "进度", "重置倒计时"],
            Lang::En => vec!["Quota Window / Model", "Remaining", "Progress", "Resets In"],
        };
        let mut q_table = Table::new(q_headers);
        q_table.set_align_right(1, true);

        if let Some(groups) = &q.quota_groups {
            for g in groups {
                for b in &g.buckets {
                    let pct = (b.remaining_fraction * 100.0).round() as i32;
                    let bar = progress_bar(pct, 10);
                    let cd = format_countdown(&b.reset_time, lang);

                    let window_desc = if b.bucket_id.contains("week") || b.window.contains("week") {
                        match lang {
                            Lang::Zh => "周配额 (7天重置)",
                            Lang::En => "Weekly (7-Day)",
                        }
                    } else if b.bucket_id.contains("5h") || b.window.contains("5h") {
                        match lang {
                            Lang::Zh => "5小时滚动配额",
                            Lang::En => "5-Hour Rolling",
                        }
                    } else {
                        &b.bucket_id
                    };

                    let group_title = if g.display_name.contains("Gemini") {
                        "Gemini"
                    } else if g.display_name.contains("Claude") || g.display_name.contains("GPT") {
                        "Claude/GPT"
                    } else {
                        &g.display_name
                    };

                    let name = format!("{} ({})", group_title, window_desc);
                    q_table.add_row(vec![
                        name,
                        format!("{}%", pct),
                        bar,
                        if cd.is_empty() { "-".into() } else { cd },
                    ]);
                }
            }
        } else if !q.models.is_empty() {
            for m in &q.models {
                let bar = progress_bar(m.percentage, 10);
                let cd = format_countdown(&m.reset_time, lang);
                q_table.add_row(vec![
                    m.name.clone(),
                    format!("{}%", m.percentage),
                    bar,
                    if cd.is_empty() { "-".into() } else { cd },
                ]);
            }
        }

        println!("\n{}", q_table.render());
    } else {
        let msg = match lang {
            Lang::Zh => "\n\x1b[33m暂无本地缓存额度，请使用主菜单 [4] 刷新配额。\x1b[0m\n",
            Lang::En => "\n\x1b[33mNo cached quota data. Use [4] to refresh live quotas.\x1b[0m\n",
        };
        println!("{}", msg);
    }

    wait_for_key(lang);
}

fn show_token_statistics(lang: Lang) {
    print!("\x1b[2J\x1b[H");
    let wait_msg = match lang {
        Lang::Zh => "正在扫描本地 Antigravity 对话数据库...\n",
        Lang::En => "Scanning local Antigravity conversation databases...\n",
    };
    print!("{}", wait_msg);
    let _ = io::stdout().flush();

    let summary = match crate::modules::native_token_stats::get_local_token_usage() {
        Ok(s) => s,
        Err(e) => {
            let _ = e;
            let err_msg = match lang {
                Lang::Zh => format!("\x1b[31m读取本地 Token 统计失败: {}\x1b[0m", e),
                Lang::En => format!("\x1b[31mFailed to read token statistics: {}\x1b[0m", e),
            };
            println!("{}", err_msg);
            wait_for_key(lang);
            return;
        }
    };

    let mut show_today_detail = false;

    loop {
        print!("\x1b[2J\x1b[H");
        if !show_today_detail {
            let title = match lang {
                Lang::Zh => "本地 Token 用量与预期费用统计",
                Lang::En => "Local Token Usage & Estimated Cost",
            };
            println!("\x1b[1m{}\x1b[0m\n", title);

            let headers = match lang {
                Lang::Zh => vec!["周期", "总计 Token", "输入", "输出", "缓存率", "预期费用"],
                Lang::En => vec!["Period", "Total Tokens", "Input", "Output", "Cache Hit", "Est. Cost"],
            };
            let mut table = Table::new(headers);
            for col in 1..=5 {
                table.set_align_right(col, true);
            }
            let periods = match lang {
                Lang::Zh => [
                    ("今日", &summary.today),
                    ("昨日", &summary.yesterday),
                    ("近 3 天", &summary.last_3_days),
                    ("近 7 天", &summary.last_7_days),
                    ("近 30 天", &summary.last_30_days),
                ],
                Lang::En => [
                    ("Today", &summary.today),
                    ("Yesterday", &summary.yesterday),
                    ("Last 3 Days", &summary.last_3_days),
                    ("Last 7 Days", &summary.last_7_days),
                    ("Last 30 Days", &summary.last_30_days),
                ],
            };
            for (i, (name, row)) in periods.iter().enumerate() {
                let total_in = row.input_tokens + row.cached_tokens;
                let hit_rate = if total_in > 0 {
                    format!("{:.1}%", (row.cached_tokens as f64 / total_in as f64) * 100.0)
                } else {
                    "0.0%".into()
                };
                let cost = period_cost(&summary, i, row.total_tokens, lang);
                table.add_row(vec![
                    name.to_string(),
                    format_number(row.total_tokens),
                    format_number(row.input_tokens),
                    format_number(row.output_tokens),
                    hit_rate,
                    cost,
                ]);
            }
            print!("{}", table.render());

            if !summary.by_model.is_empty() {
                let top_title = match lang {
                    Lang::Zh => "【近 30 天主要模型用量排行】",
                    Lang::En => "[Top Models: Last 30 Days]",
                };
                println!("\n{}", top_title);

                let m_headers = match lang {
                    Lang::Zh => vec!["模型", "总计 Token", "请求次数", "预期费用"],
                    Lang::En => vec!["Model", "Total Tokens", "Requests", "Est. Cost"],
                };
                let mut m_table = Table::new(m_headers);
                m_table.set_align_right(1, true);
                m_table.set_align_right(2, true);
                m_table.set_align_right(3, true);

                let term_h = get_terminal_height();
                let limit = if term_h >= 28 { 5 } else { 3 };
                for m in summary.by_model.iter().take(limit) {
                    let cost = estimate_model_cost(m.input_tokens, m.output_tokens, m.cached_tokens, &m.model);
                    m_table.add_row(vec![
                        m.model.clone(),
                        format_number(m.total_tokens),
                        format_number(m.request_count),
                        format_cost(cost, lang),
                    ]);
                }
                print!("{}", m_table.render());
            }

            let footer = match lang {
                Lang::Zh => format!(
                    "\n\x1b[90m数据来源: 扫描 {} 个本地 SQLite 数据库 | 累计对话: {} 条\x1b[0m\n",
                    summary.databases_scanned, summary.generations_scanned
                ),
                Lang::En => format!(
                    "\n\x1b[90mSource: Scanned {} local SQLite databases | {} conversations\x1b[0m\n",
                    summary.databases_scanned, summary.generations_scanned
                ),
            };
            print!("{}", footer);

            let prompt = match lang {
                Lang::Zh => "\x1b[1m操作: (M 查看今日模型明细  |  Esc/0 返回主菜单)\x1b[0m ",
                Lang::En => "\x1b[1mAction: (M Today's Models  |  Esc/0 Back)\x1b[0m ",
            };
            print!("{}", prompt);
            let _ = io::stdout().flush();
        } else {
            let title = match lang {
                Lang::Zh => "今日各模型用量明细",
                Lang::En => "Model Breakdown: Today",
            };
            println!("\x1b[1m【{}】\x1b[0m\n", title);

            if summary.by_model_today.is_empty() {
                let msg = match lang {
                    Lang::Zh => "\x1b[33m今日暂无模型用量记录。\x1b[0m\n",
                    Lang::En => "\x1b[33mNo model usage recorded today.\x1b[0m\n",
                };
                println!("{}", msg);
            } else {
                let m_headers = match lang {
                    Lang::Zh => vec!["模型", "总计 Token", "输入", "输出", "缓存", "预期费用"],
                    Lang::En => vec!["Model", "Total Tokens", "Input", "Output", "Cached", "Est. Cost"],
                };
                let mut m_table = Table::new(m_headers);
                for col in 1..=5 {
                    m_table.set_align_right(col, true);
                }
                for m in &summary.by_model_today {
                    let cost = estimate_model_cost(m.input_tokens, m.output_tokens, m.cached_tokens, &m.model);
                    m_table.add_row(vec![
                        m.model.clone(),
                        format_number(m.total_tokens),
                        format_number(m.input_tokens),
                        format_number(m.output_tokens),
                        format_number(m.cached_tokens),
                        format_cost(cost, lang),
                    ]);
                }
                print!("{}", m_table.render());
            }

            let footer = match lang {
                Lang::Zh => format!(
                    "\n\x1b[90m数据来源: 扫描 {} 个本地 SQLite 数据库 | 累计对话: {} 条\x1b[0m\n",
                    summary.databases_scanned, summary.generations_scanned
                ),
                Lang::En => format!(
                    "\n\x1b[90mSource: Scanned {} local SQLite databases | {} conversations\x1b[0m\n",
                    summary.databases_scanned, summary.generations_scanned
                ),
            };
            print!("{}", footer);

            let prompt = match lang {
                Lang::Zh => "\x1b[1m操作: (Esc/0 返回用量总览)\x1b[0m ",
                Lang::En => "\x1b[1mAction: (Esc/0 Back to Overview)\x1b[0m ",
            };
            print!("{}", prompt);
            let _ = io::stdout().flush();
        }

            {
            if let Some(_raw) = RawTerminal::enter() {
                match read_key_action() {
                    KeyAction::Char('m') | KeyAction::Char('M') | KeyAction::Char('\t') => {
                        show_today_detail = !show_today_detail;
                    }
                    KeyAction::Enter | KeyAction::Char('0') | KeyAction::Char('q') | KeyAction::Char('Q') | KeyAction::Cancel => {
                        if show_today_detail {
                            show_today_detail = false;
                        } else {
                            break;
                        }
                    }
                    _ => {}
                }
            } else {
                break;
            }
        }
    }
}

fn show_refresh_quotas(root: &Path, lang: Lang) {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let _ = e;
            println!("{}", match lang { Lang::Zh => "无法启动登录服务", Lang::En => "Could not start the sign-in service" });
            wait_for_key(lang);
            return;
        }
    };

    loop {
        let snapshot = Snapshot::read(root).unwrap_or(Snapshot {
            accounts: vec![],
            current_target: None,
        });

        print!("\x1b[2J\x1b[H");
        if snapshot.accounts.is_empty() {
            let msg = match lang {
                Lang::Zh => "\n\x1b[33m暂无已保存账号。\x1b[0m",
                Lang::En => "\n\x1b[33mNo saved accounts.\x1b[0m",
            };
            println!("{}", msg);
            wait_for_key(lang);
            return;
        }

        let header = match lang {
            Lang::Zh => "配额刷新中心",
            Lang::En => "Quota Refresh Hub",
        };
        println!("\x1b[1m{}\x1b[0m\n", header);

        let (title, items) = match lang {
            Lang::Zh => (
                "选择刷新方式:",
                vec![
                    "1. 刷新当前选择账号配额",
                    "2. 批量刷新全部账号 (并发执行)",
                    "3. 选择指定账号刷新",
                    "0. 返回主菜单",
                ],
            ),
            Lang::En => (
                "Select Refresh Mode:",
                vec![
                    "1. Refresh selected account quota",
                    "2. Batch refresh all accounts (concurrent)",
                    "3. Select specific account to refresh",
                    "0. Back to main menu",
                ],
            ),
        };

        let choice = select_menu_interactive(title, &items, 0, lang);

        match choice {
            Some(0) => {
            print!("\x1b[2J\x1b[H");
            let sub_title = match lang {
                Lang::Zh => "刷新当前选择账号配额",
                Lang::En => "Refresh Selected Account Quota",
            };
            println!("\x1b[1m{}\x1b[0m\n", sub_title);

            let curr = match snapshot.current() {
                Ok(c) => c,
                Err(_) => {
                    let msg = match lang {
                        Lang::Zh => "\x1b[33m当前尚未选择账号，请使用 [3] 指定账号刷新。\x1b[0m",
                        Lang::En => "\x1b[33mNo account selected. Use [3] to select an account.\x1b[0m",
                    };
                    println!("{}", msg);
                    wait_for_key(lang);
                    continue;
                }
            };
            let wait_msg = match lang {
                Lang::Zh => format!("\x1b[2m正在请求 Google API 刷新账号 ({}) 配额...\x1b[0m\n", curr.email),
                Lang::En => format!("\x1b[2mRefreshing quota for ({}) via Google API...\x1b[0m\n", curr.email),
            };
            println!("{}", wait_msg);
            match runtime.block_on(async {
                let mut account = crate::modules::account::load_account(&curr.id)?;
                let quota = crate::modules::account::fetch_quota_with_retry(&mut account)
                    .await
                    .map_err(|e| e.to_string())?;
                crate::modules::account::update_account_quota(&curr.id, quota.clone())?;
                Ok::<crate::models::QuotaData, String>(quota)
            }) {
                Ok(quota) => {
                    let succ_msg = match lang {
                        Lang::Zh => "\x1b[1;32m✓ 配额刷新成功！最新数据:\x1b[0m",
                        Lang::En => "\x1b[1;32m✓ Quota refreshed successfully! Latest:\x1b[0m",
                    };
                    println!("{}", succ_msg);

                    let q_headers = match lang {
                        Lang::Zh => vec!["模型 / 分组", "剩余比例", "重置倒计时"],
                        Lang::En => vec!["Model / Group", "Remaining", "Resets In"],
                    };
                    let mut q_table = Table::new(q_headers);
                    q_table.set_align_right(1, true);

                    if let Some(groups) = &quota.quota_groups {
                        for g in groups {
                            if let Some(b) = g.buckets.first() {
                                let pct = (b.remaining_fraction * 100.0).round() as i32;
                                let cd = format_countdown(&b.reset_time, lang);
                                q_table.add_row(vec![
                                    g.display_name.clone(),
                                    format!("{}%", pct),
                                    if cd.is_empty() { "-".into() } else { cd },
                                ]);
                            }
                        }
                    } else {
                        for m in &quota.models {
                            let cd = format_countdown(&m.reset_time, lang);
                            q_table.add_row(vec![
                                m.name.clone(),
                                format!("{}%", m.percentage),
                                if cd.is_empty() { "-".into() } else { cd },
                            ]);
                        }
                    }
                    print!("{}", q_table.render());
                }
                Err(e) => {
                    let _ = e;
                    let fail_msg = match lang {
                        Lang::Zh => format!("\x1b[31m刷新失败: {}\x1b[0m", e),
                        Lang::En => format!("\x1b[31mRefresh failed: {}\x1b[0m", e),
                    };
                    println!("{}", fail_msg);
                }
            }
            wait_for_key(lang);
        }
        Some(1) => {
            print!("\x1b[2J\x1b[H");
            let sub_title = match lang {
                Lang::Zh => "批量刷新全部账号配额",
                Lang::En => "Batch Refresh All Accounts Quota",
            };
            println!("\x1b[1m{}\x1b[0m\n", sub_title);

            let wait_msg = match lang {
                Lang::Zh => format!("\x1b[2m正在并发批量刷新所有 {} 个账号配额...\x1b[0m\n", snapshot.accounts.len()),
                Lang::En => format!("\x1b[2mBatch refreshing all {} accounts concurrently...\x1b[0m\n", snapshot.accounts.len()),
            };
            println!("{}", wait_msg);
            match runtime.block_on(crate::modules::account::refresh_all_quotas_logic()) {
                Ok(stats) => {
                    let result_msg = match lang {
                        Lang::Zh => format!(
                            "\x1b[1;32m✓ 批量刷新完成: 成功 {} 个, 失败 {} 个 (总计: {})\x1b[0m",
                            stats.success, stats.failed, stats.total
                        ),
                        Lang::En => format!(
                            "\x1b[1;32m✓ Batch refresh complete: {} succeeded, {} failed (total: {})\x1b[0m",
                            stats.success, stats.failed, stats.total
                        ),
                    };
                    println!("{}", result_msg);
                    for detail in stats.details {
                        println!("  {}", detail);
                    }
                }
                Err(e) => {
                    let _ = e;
                    let fail_msg = match lang {
                        Lang::Zh => format!("\x1b[31m批量刷新失败: {}\x1b[0m", e),
                        Lang::En => format!("\x1b[31mBatch refresh failed: {}\x1b[0m", e),
                    };
                    println!("{}", fail_msg);
                }
            }
            wait_for_key(lang);
        }
        Some(2) => {
            print!("\x1b[2J\x1b[H");
            let sub_title = match lang {
                Lang::Zh => "选择指定账号刷新配额",
                Lang::En => "Select Account to Refresh Quota",
            };
            println!("\x1b[1m{}\x1b[0m\n", sub_title);

            let selected = match select_account_interactive(&snapshot.accounts, lang) {
                Some(acc) => acc,
                None => continue,
            };
            print!("\x1b[2J\x1b[H");
            println!("\x1b[1m{}\x1b[0m\n", sub_title);
            let wait_msg = match lang {
                Lang::Zh => format!("\x1b[2m正在刷新账号 ({}) 配额...\x1b[0m\n", selected.email),
                Lang::En => format!("\x1b[2mRefreshing quota for ({}) ...\x1b[0m\n", selected.email),
            };
            println!("{}", wait_msg);
            match runtime.block_on(async {
                let mut account = crate::modules::account::load_account(&selected.id)?;
                let quota = crate::modules::account::fetch_quota_with_retry(&mut account)
                    .await
                    .map_err(|e| e.to_string())?;
                crate::modules::account::update_account_quota(&selected.id, quota.clone())?;
                Ok::<crate::models::QuotaData, String>(quota)
            }) {
                Ok(quota) => {
                    let succ_msg = match lang {
                        Lang::Zh => "\x1b[1;32m✓ 配额刷新成功！\x1b[0m",
                        Lang::En => "\x1b[1;32m✓ Quota refreshed successfully!\x1b[0m",
                    };
                    println!("{}", succ_msg);

                    let q_headers = match lang {
                        Lang::Zh => vec!["模型 / 分组", "剩余比例", "重置倒计时"],
                        Lang::En => vec!["Model / Group", "Remaining", "Resets In"],
                    };
                    let mut q_table = Table::new(q_headers);
                    q_table.set_align_right(1, true);

                    if let Some(groups) = &quota.quota_groups {
                        for g in groups {
                            if let Some(b) = g.buckets.first() {
                                let pct = (b.remaining_fraction * 100.0).round() as i32;
                                let cd = format_countdown(&b.reset_time, lang);
                                q_table.add_row(vec![
                                    g.display_name.clone(),
                                    format!("{}%", pct),
                                    if cd.is_empty() { "-".into() } else { cd },
                                ]);
                            }
                        }
                    } else {
                        for m in &quota.models {
                            let cd = format_countdown(&m.reset_time, lang);
                            q_table.add_row(vec![
                                m.name.clone(),
                                format!("{}%", m.percentage),
                                if cd.is_empty() { "-".into() } else { cd },
                            ]);
                        }
                    }
                    print!("{}", q_table.render());
                }
                Err(e) => {
                    let _ = e;
                    let fail_msg = match lang {
                        Lang::Zh => format!("\x1b[31m刷新失败: {}\x1b[0m", e),
                        Lang::En => format!("\x1b[31mRefresh failed: {}\x1b[0m", e),
                    };
                    println!("{}", fail_msg);
                }
            }
            wait_for_key(lang);
        }
        Some(3) | None => break,
        _ => break,
    }
}
}

fn show_add_account(_root: &Path, lang: Lang) {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            println!("\x1b[31mRuntime error: {}\x1b[0m", e);
            wait_for_key(lang);
            return;
        }
    };

    loop {
        print!("\x1b[2J\x1b[H");
        let header = match lang {
            Lang::Zh => "添加 Google 账号",
            Lang::En => "Add Google Account",
        };
        println!("\x1b[1m{}\x1b[0m\n", header);

        let (title, items) = match lang {
            Lang::Zh => (
                "选择添加方式:",
                vec![
                    "1. 浏览器一键授权 (Google OAuth 自动登录)",
                    "2. 手动输入刷新凭据",
                    "0. 返回主菜单",
                ],
            ),
            Lang::En => (
                "Select Method:",
                vec![
                    "1. Browser authorization (Google OAuth auto-login)",
                    "2. Manually enter Refresh Token",
                    "0. Back to main menu",
                ],
            ),
        };

        let choice = select_menu_interactive(title, &items, 0, lang);

        match choice {
            Some(0) => {
            print!("\x1b[2J\x1b[H");
            let sub_title = match lang {
                Lang::Zh => "浏览器一键授权 · Google OAuth",
                Lang::En => "Browser Authorization · Google OAuth",
            };
            println!("\x1b[1m{}\x1b[0m\n", sub_title);

            let wait_msg = match lang {
                Lang::Zh => "\x1b[2m正在准备 Google OAuth 登录服务...\x1b[0m\n",
                Lang::En => "\x1b[2mPreparing Google OAuth login service...\x1b[0m\n",
            };
            println!("{}", wait_msg);

            let auth_url = match runtime.block_on(crate::modules::oauth_server::prepare_oauth_url(None, None)) {
                Ok(url) => url,
                Err(e) => {
                    let _ = e;
                    let err_msg = match lang {
                        Lang::Zh => "\x1b[31m无法启动授权服务，请检查网络后重试。\x1b[0m".to_string(),
                        Lang::En => "\x1b[31mCould not start authorization. Check your connection and try again.\x1b[0m".to_string(),
                    };
                    println!("{}", err_msg);
                    wait_for_key(lang);
                    continue;
                }
            };

            open_browser(&auth_url);
            let open_msg = match lang {
                Lang::Zh => format!(
                    "\x1b[1;32m✓ 已启动本地回调服务并尝试打开系统浏览器。\x1b[0m\n若浏览器未自动打开，请手动复制并在浏览器中访问以下授权链接:\n\x1b[4;34m{}\x1b[0m\n\n\x1b[2m等待浏览器授权完成... (可按 Ctrl+C 取消)\x1b[0m",
                    auth_url
                ),
                Lang::En => format!(
                    "\x1b[1;32m✓ Local callback service started. Opening browser.\x1b[0m\nIf browser does not open automatically, copy and visit this URL:\n\x1b[4;34m{}\x1b[0m\n\n\x1b[2mWaiting for browser authorization... (Press Ctrl+C to cancel)\x1b[0m",
                    auth_url
                ),
            };
            println!("{}", open_msg);

            let token_res = match runtime.block_on(crate::modules::oauth_server::complete_oauth_flow(None)) {
                Ok(t) => t,
                Err(e) => {
                    let _ = e;
                    let fail_msg = match lang {
                        Lang::Zh => "\x1b[31m授权未完成，请重新发起登录。\x1b[0m".to_string(),
                        Lang::En => "\x1b[31mAuthorization did not complete. Start sign-in again.\x1b[0m".to_string(),
                    };
                    println!("{}", fail_msg);
                    wait_for_key(lang);
                    continue;
                }
            };

            let refresh_token = match token_res.refresh_token {
                Some(rt) => rt,
                None => {
                    let no_token_msg = match lang {
                        Lang::Zh => "\x1b[31m未能获取到 Refresh Token，请撤销旧授权后重试。\x1b[0m",
                        Lang::En => "\x1b[31mFailed to obtain Refresh Token. Revoke prior access and retry.\x1b[0m",
                    };
                    println!("{}", no_token_msg);
                    wait_for_key(lang);
                    continue;
                }
            };

            match runtime.block_on(async {
                let user_info = crate::modules::oauth::get_user_info(&token_res.access_token).await?;
                let project_id = crate::modules::project_resolver::fetch_project_id(&token_res.access_token)
                    .await
                    .ok();
                let token_data = crate::models::TokenData::new(
                    token_res.access_token.clone(),
                    refresh_token,
                    token_res.expires_in,
                    Some(user_info.email.clone()),
                    project_id,
                    None,
                    false,
                    token_res.id_token.clone(),
                )
                .with_oauth_client_key(token_res.oauth_client_key.clone());

                let mut account = crate::modules::upsert_account(
                    user_info.email.clone(),
                    user_info.get_display_name(),
                    token_data,
                )?;

                let _ = crate::modules::account::fetch_quota_with_retry(&mut account).await;
                Ok::<String, String>(account.email)
            }) {
                Ok(email) => {
                    let succ_msg = match lang {
                        Lang::Zh => format!("\x1b[1;32m✓ 账号添加成功: {}\x1b[0m", email),
                        Lang::En => format!("\x1b[1;32m✓ Account added successfully: {}\x1b[0m", email),
                    };
                    println!("{}", succ_msg);
                }
                Err(e) => {
                    let _ = e;
                    let fail_msg = match lang {
                        Lang::Zh => "\x1b[31m无法添加账号，请检查授权和网络后重试。\x1b[0m".to_string(),
                        Lang::En => "\x1b[31mCould not add the account. Check authorization and your connection.\x1b[0m".to_string(),
                    };
                    println!("{}", fail_msg);
                }
            }
            wait_for_key(lang);
        }
        Some(1) => {
            print!("\x1b[2J\x1b[H");
            let sub_title = match lang {
                Lang::Zh => "手动导入凭据 · Refresh Token",
                Lang::En => "Manual Import · Refresh Token",
            };
            println!("\x1b[1m{}\x1b[0m\n", sub_title);

            let tip = match lang {
                Lang::Zh => "\x1b[90m(操作: 粘贴或输入 Token 后按 [回车] 确定  |  按 [Esc] 取消返回)\x1b[0m\n",
                Lang::En => "\x1b[90m(Action: Paste or type Token and press [Enter]  |  Press [Esc] to cancel)\x1b[0m\n",
            };
            println!("{}", tip);

            let prompt_text = match lang {
                Lang::Zh => "请输入 Google Refresh Token: ",
                Lang::En => "Enter Google Refresh Token: ",
            };
            let refresh_token = match prompt_input(prompt_text, 0, true) {
                PromptResult::Confirmed(token) => token,
                PromptResult::Cancelled => {
                    continue;
                }
            };
            if refresh_token.is_empty() {
                continue;
            }

            let wait_msg = match lang {
                Lang::Zh => "\x1b[2m正在验证 Token 并拉取账号详情...\x1b[0m",
                Lang::En => "\x1b[2mVerifying Token and fetching profile...\x1b[0m",
            };
            println!("{}", wait_msg);

            match runtime.block_on(async {
                let token_res = crate::modules::oauth::refresh_access_token(&refresh_token, None).await?;
                let user_info = crate::modules::oauth::get_user_info(&token_res.access_token).await?;
                let project_id = crate::modules::project_resolver::fetch_project_id(&token_res.access_token)
                    .await
                    .ok();
                let token_data = crate::models::TokenData::new(
                    token_res.access_token.clone(),
                    refresh_token,
                    token_res.expires_in,
                    Some(user_info.email.clone()),
                    project_id,
                    None,
                    false,
                    token_res.id_token.clone(),
                )
                .with_oauth_client_key(token_res.oauth_client_key.clone());

                let mut account = crate::modules::upsert_account(
                    user_info.email.clone(),
                    user_info.get_display_name(),
                    token_data,
                )?;

                let _ = crate::modules::account::fetch_quota_with_retry(&mut account).await;
                Ok::<String, String>(account.email)
            }) {
                Ok(email) => {
                    let succ_msg = match lang {
                        Lang::Zh => format!("\x1b[1;32m✓ 账号导入成功: {}\x1b[0m", email),
                        Lang::En => format!("\x1b[1;32m✓ Account imported successfully: {}\x1b[0m", email),
                    };
                    println!("{}", succ_msg);
                }
                Err(e) => {
                    let _ = e;
                    let fail_msg = match lang {
                        Lang::Zh => "\x1b[31m无法验证凭据，请检查凭据和网络后重试。\x1b[0m".to_string(),
                        Lang::En => "\x1b[31mCould not verify the credentials. Check them and your connection.\x1b[0m".to_string(),
                    };
                    println!("{}", fail_msg);
                }
            }
            wait_for_key(lang);
        }
        Some(2) | None => break,
        _ => break,
    }
}
}

fn show_system_status(snapshot: &Snapshot, root: &Path, lang: Lang) {
    print!("\x1b[2J\x1b[H");
    let title = match lang {
        Lang::Zh => "关联应用与系统状态",
        Lang::En => "Linked Applications & System Status",
    };
    println!("\x1b[1m{}\x1b[0m\n", title);

    let home = std::env::var("HOME").unwrap_or_default();
    let app_path = crate::modules::process::get_antigravity_executable_path(None);
    let app_running = crate::modules::process::is_antigravity_running(None);
    let ide_running = crate::modules::process::is_antigravity_running(Some("ide"));

    let running_text = match lang {
        Lang::Zh => "\x1b[32m运行中\x1b[0m",
        Lang::En => "\x1b[32mRunning\x1b[0m",
    };
    let stopped_text = match lang {
        Lang::Zh => "\x1b[90m未运行\x1b[0m",
        Lang::En => "\x1b[90mStopped\x1b[0m",
    };

    let app_status = if app_running { running_text } else { stopped_text };
    let ide_status = if ide_running { running_text } else { stopped_text };

    let app_path_str = if let Some(p) = app_path {
        if !home.is_empty() && p.starts_with(&home) {
            let rel = p.strip_prefix(&home).unwrap_or(&p);
            format!("~/{}", rel.display())
        } else {
            p.display().to_string()
        }
    } else {
        match lang {
            Lang::Zh => "未自动识别 (可在设置中手动指定)".into(),
            Lang::En => "Not detected (can specify in Settings)".into(),
        }
    };

    let ide_details = match lang {
        Lang::Zh => "支持独立 IDE 或 VS Code 插件凭据关联",
        Lang::En => "Independent IDE or VS Code plugin supported",
    };

    let active_info = if let Ok(curr) = snapshot.current() {
        format!(
            "{} [{}]",
            curr.email,
            snapshot.current_target.as_deref().unwrap_or("app")
        )
    } else {
        match lang {
            Lang::Zh => "未设置".into(),
            Lang::En => "None".into(),
        }
    };

    let display_root = if !home.is_empty() && root.starts_with(&home) {
        let rel = root.strip_prefix(&home).unwrap_or(root);
        format!("~/{}", rel.display())
    } else {
        root.display().to_string()
    };

    let storage_status = match lang {
        Lang::Zh => format!("{} (共 {} 个账号)", display_root, snapshot.accounts.len()),
        Lang::En => format!("{} ({} accounts)", display_root, snapshot.accounts.len()),
    };

    let headers = match lang {
        Lang::Zh => vec!["组件名称", "状态", "路径 / 详细信息"],
        Lang::En => vec!["Component", "Status", "Path / Details"],
    };
    let mut table = Table::new(headers);

    let rows = match lang {
        Lang::Zh => vec![
            vec!["AntiGravity 桌面应用".into(), app_status.into(), app_path_str],
            vec!["AntiGravity IDE / VS Code 插件".into(), ide_status.into(), ide_details.into()],
            vec!["本地数据存储".into(), "正常".into(), storage_status],
            vec!["当前选择账号".into(), "本地记录".into(), active_info],
            vec![
                "命令行工具".into(),
                format!("v{}", env!("CARGO_PKG_VERSION")),
                "agy-switch".into(),
            ],
        ],
        Lang::En => vec![
            vec!["AntiGravity Desktop App".into(), app_status.into(), app_path_str],
            vec!["AntiGravity IDE / VS Code Plugin".into(), ide_status.into(), ide_details.into()],
            vec!["Local Data Storage".into(), "Normal".into(), storage_status],
            vec!["Selected Account".into(), "Local record".into(), active_info],
            vec![
                "CLI Binary".into(),
                format!("v{}", env!("CARGO_PKG_VERSION")),
                "agy-switch".into(),
            ],
        ],
    };

    for r in rows {
        table.add_row(r);
    }
    print!("{}", table.render());

    wait_for_key(lang);
}

#[cfg(test)]
mod presentation_tests {
    use super::*;
    #[test]
    fn keyboard_navigation_is_identical_across_platforms() {
        for (code, expected) in [(KeyCode::Up, KeyAction::Up), (KeyCode::Down, KeyAction::Down),
            (KeyCode::Left, KeyAction::Cancel), (KeyCode::Right, KeyAction::Enter),
            (KeyCode::Esc, KeyAction::Cancel), (KeyCode::Enter, KeyAction::Enter),
            (KeyCode::Char('2'), KeyAction::SelectIndex(1)), (KeyCode::Tab, KeyAction::Down),
            (KeyCode::BackTab, KeyAction::Up), (KeyCode::Home, KeyAction::Home), (KeyCode::End, KeyAction::End)] {
            assert_eq!(key_action(KeyEvent::new(code, KeyModifiers::NONE)), expected);
        }
        assert_eq!(key_action(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)), KeyAction::Cancel);
        assert_eq!(key_action(KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT)), KeyAction::MoveUp);
        assert_eq!(key_action(KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT)), KeyAction::MoveDown);
        assert_eq!(key_action(KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Release)), KeyAction::None);
    }
    #[test]
    fn unpriced_costs_are_localized_and_never_reported_as_free() {
        assert_eq!(format_cost(None, Lang::Zh), "未计价");
        assert_eq!(format_cost(None, Lang::En), "Unpriced");
        assert_eq!(format_cost(Some(0.0), Lang::En), "$0.00");
        assert_eq!(format_cost(Some(0.003), Lang::En), "$0.0030");
    }
    #[test]
    fn absent_quota_windows_are_unknown_in_both_languages() {
        let account: AccountView = serde_json::from_value(serde_json::json!({"id":"fixture","email":"example@example.invalid","is_current":false,"disabled":false,"validation_blocked":false,"quota":{"last_updated":1,"models":[],"quota_groups":[{"display_name":"Gemini Models","buckets":[{"bucket_id":"gemini-weekly","window":"weekly","remaining_fraction":0.7,"reset_time":""}]}]}})).unwrap();
        for lang in [Lang::Zh, Lang::En] {
            let rendered = build_accounts_table(&[serde_json::from_value(serde_json::to_value(&account).unwrap()).unwrap()], lang, None).render();
            assert!(rendered.contains("70%"));
            assert!(!rendered.contains("100%"));
            assert!(rendered.contains(match lang { Lang::Zh => "未知", Lang::En => "Unknown" }));
        }
    }
}
