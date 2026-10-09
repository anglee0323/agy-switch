//! macOS overview is an NSMenu, like CodexBar: AppKit owns the surface and
//! tracking; custom content uses semantic system text and small quota bars.
//! No WebView transparency, root-view replacement, or credential DTOs.
use super::{account_dashboard::{DashboardSnapshot, DashboardEntry}, menu_bar_projection as projection};
use crate::{commands, modules, models::AppConfig};
use objc2::{define_class, msg_send, sel, AnyThread, DefinedClass, MainThreadOnly};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSApplication, NSBezierPath, NSButton, NSBezelStyle, NSCellImagePosition, NSControlSize, NSColor, NSFont, NSImage, NSImageView, NSMenu, NSMenuItem, NSTextField, NSView, NSEvent, NSTrackingArea, NSTrackingAreaOptions};
use crate::models::config::{MenuBarPreferences, MenuBarQuotaScope, MenuBarResetTimeDisplay};
use tauri_plugin_opener::OpenerExt;
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSTimer, NSRunLoop, NSRunLoopCommonModes};
use std::{cell::{Cell, RefCell}, sync::{Mutex, mpsc, atomic::{AtomicBool, AtomicU64, Ordering}}, time::Instant};
use tauri::Emitter;

const WIDTH: f64 = 424.0;
const FAMILY_GAP: f64 = 24.0;
const BRAND: &str = "AntiGravity Switch";
const GITHUB: &str = "https://github.com/anglee0323/agy-switch";
static OPEN: AtomicBool = AtomicBool::new(false);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static BUSY: AtomicBool = AtomicBool::new(false);
static NOTICE: Mutex<Option<String>> = Mutex::new(None);
struct MenuSession {
    zh: bool,
    _targets: Vec<Retained<MenuAction>>,
    controls: Vec<AccountControls>,
    identity_summary: Retained<NSTextField>,
    usage: UsageWidgets,
    quotas: Vec<AccountQuotaWidgets>,
    aggregates: Vec<AggregateWidgets>,
    preferences: MenuBarPreferences,
    freshness_minutes: i32,
    reserve: u8,
    busy: bool,
}
thread_local! {
    static ACTIVE: RefCell<Option<Retained<NSMenu>>> = const { RefCell::new(None) };
    static SESSION: RefCell<Option<MenuSession>> = const { RefCell::new(None) };
}

#[derive(Clone)]
enum Action { Switch(String), Page(&'static str), Refresh, Cancel(String), Github, Quit }
struct ActionState { app: tauri::AppHandle, menu: Retained<NSMenu>, action: Action, zh: bool }
define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ActionState]
    struct MenuAction;
    unsafe impl NSObjectProtocol for MenuAction {}
    impl MenuAction {
        #[unsafe(method(perform:))]
        fn perform(&self, _sender: &AnyObject) {
            let state = self.ivars();
            let mut root = state.menu.clone();
            // All parent menus are retained during this tracking session.
            while let Some(parent) = unsafe { root.supermenu() } { root = parent; }
            root.cancelTrackingWithoutAnimation();
            let app = state.app.clone();
            let zh = state.zh;
            match &state.action {
                Action::Quit => app.exit(0),
                Action::Github => { let _ = app.opener().open_url(GITHUB, None::<&str>); },
                Action::Page(page) => { let _ = modules::desktop::open_app_page(app, (*page).into()); },
                action => {
                    if BUSY.swap(true, Ordering::AcqRel) { return; }
                    let action = action.clone();
                    tauri::async_runtime::spawn(async move {
                        let result = match action {
                            Action::Switch(id) => commands::switch_account(app.clone(), id.clone(), None).await.map(|()| {
                                let _ = app.emit("tray://account-switched", id);
                            }).map_err(|_| if zh { "切换失败，请在 App 中检查账号状态".into() } else { "Switch failed. Check this account in the app.".into() }),
                            Action::Refresh => commands::refresh_all_quotas(app.clone()).await.map_err(|_| if zh { "刷新失败，请重试".to_string() } else { "Refresh failed. Retry.".into() }).and_then(|stats| {
                                if stats.failed > 0 { Err(if zh { format!("{} 个账号刷新失败，已保留缓存", stats.failed) } else { format!("{} accounts could not refresh", stats.failed) }) }
                                else { Ok(()) }
                            }),
                            Action::Cancel(id) => modules::auto_switch::cancel_auto_switch(app.clone(), id).map(|_| ()).map_err(|_| if zh { "取消失败，请在 App 中检查换号状态".into() } else { "Cancellation failed. Check auto-switch in the app.".into() }),
                            _ => Ok(()),
                        };
                        if let Ok(mut notice) = NOTICE.lock() { *notice = result.err(); }
                        BUSY.store(false, Ordering::Release);
                        modules::tray::update_tray_menus(&app);
                    });
                }
            }
        }
    }
);
impl MenuAction {
    fn new(marker: MainThreadMarker, state: ActionState) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(state);
        // NSObject initializer; targets stay retained throughout menu tracking.
        unsafe { msg_send![super(this), init] }
    }
}

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    struct SectionView;
    unsafe impl NSObjectProtocol for SectionView {}
    impl SectionView { #[unsafe(method(isFlipped))] fn flipped(&self) -> bool { true } }
);
fn section(marker: MainThreadMarker, height: f64) -> Retained<SectionView> {
    unsafe { msg_send![SectionView::alloc(marker), initWithFrame: rect(0.0, 0.0, WIDTH, height)] }
}
struct BarState { value: Cell<Option<f64>>, preferences: MenuBarPreferences, disabled: Cell<bool> }
define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = BarState]
    struct QuotaBar;
    unsafe impl NSObjectProtocol for QuotaBar {}
    impl QuotaBar {
        #[unsafe(method(drawRect:))]
        fn draw(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            if self.ivars().disabled.get() { NSColor::systemRedColor().setFill(); } else { NSColor::quaternaryLabelColor().setFill(); }
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bounds, 3.0, 3.0).fill();
            if let Some(value) = self.ivars().value.get() {
                let color = quota_color(Some(value), &self.ivars().preferences);
                color.setFill();
                let fill = rect(0.0, 0.0, bounds.size.width * (value / 100.0).clamp(0.0, 1.0), bounds.size.height);
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(fill, 3.0, 3.0).fill();
            }
        }
    }
);
impl QuotaBar {
    fn update(&self, value: Option<f64>, disabled: bool) {
        if self.ivars().value.replace(value) != value || self.ivars().disabled.get() != disabled {
            self.ivars().disabled.set(disabled);
            self.setNeedsDisplay(true);
        }
    }
}
// Lay out quota columns according to the family selected in Settings.
struct QuotaRow { cells: Vec<(Retained<QuotaBar>, Retained<NSTextField>)>, y: f64 }
fn quota_column(scope: MenuBarQuotaScope, family: usize) -> (f64, f64) {
    let gap = if scope == MenuBarQuotaScope::All { FAMILY_GAP } else { 0.0 };
    // Each family owns its period label, meter/reset time and percentage.
    let width = (WIDTH - 40.0 - gap) / if scope == MenuBarQuotaScope::All { 2.0 } else { 1.0 } - 50.0;
    (70.0 + if scope == MenuBarQuotaScope::All { family as f64 * (width + 50.0 + gap) } else { 0.0 }, width)
}
impl QuotaRow {
    fn apply(&self, scope: MenuBarQuotaScope) {
        for (family, (progress, text)) in self.cells.iter().enumerate() {
            let visible = scope == MenuBarQuotaScope::All || (scope == MenuBarQuotaScope::Gemini && family == 0) || (scope == MenuBarQuotaScope::Other && family == 1);
            progress.setHidden(!visible); text.setHidden(!visible);
            let (x, width) = quota_column(scope, family);
            progress.setFrame(rect(x, self.y + 3.0, width - 49.0, 4.0)); progress.setNeedsDisplay(true);
            text.setFrame(rect(x + width - 44.0, self.y - 3.0, 46.0, 18.0));
        }
    }
}
#[derive(Default)]
struct AccountRowState { resets: RefCell<Vec<HoverQuota>>, mode: MenuBarResetTimeDisplay, divider: Option<NSRect> }
struct HoverQuota { bar: Retained<QuotaBar>, reset: Retained<SectionView>, full: NSRect, compact: NSRect }
define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = AccountRowState]
    struct AccountRow;
    unsafe impl NSObjectProtocol for AccountRow {}
    impl AccountRow {
        #[unsafe(method(isFlipped))] fn flipped(&self) -> bool { true }
        #[unsafe(method(mouseEntered:))] fn entered(&self, _event: &NSEvent) { if self.ivars().mode == MenuBarResetTimeDisplay::Hover { self.show_resets(true); } }
        #[unsafe(method(mouseExited:))] fn exited(&self, _event: &NSEvent) { if self.ivars().mode == MenuBarResetTimeDisplay::Hover { self.show_resets(false); } }
        #[unsafe(method(drawRect:))]
        fn draw(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            NSColor::separatorColor().colorWithAlphaComponent(0.45).setFill();
            NSBezierPath::bezierPathWithRect(rect(20.0, bounds.size.height - 2.0, bounds.size.width - 40.0, 0.5)).fill();
            if let Some(divider) = self.ivars().divider {
                NSColor::separatorColor().setFill();
                NSBezierPath::bezierPathWithRect(divider).fill();
            }
        }
    }
);
impl AccountRow {
    fn show_resets(&self, visible: bool) {
        for cell in self.ivars().resets.borrow().iter() {
            cell.bar.setFrame(if visible { cell.compact } else { cell.full });
            cell.bar.setNeedsDisplay(true); cell.reset.setHidden(!visible);
        }
    }
}
fn track_hover(view: &NSView) {
    // ActiveAlways is required for status menus when the app is not foreground.
    let area = unsafe { NSTrackingArea::initWithRect_options_owner_userInfo(NSTrackingArea::alloc(), view.bounds(),
        NSTrackingAreaOptions::MouseEnteredAndExited | NSTrackingAreaOptions::ActiveAlways | NSTrackingAreaOptions::InVisibleRect, Some(view), None) };
    view.addTrackingArea(&area);
}
define_class!(
    #[unsafe(super = NSButton)]
    #[thread_kind = MainThreadOnly]
    struct HoverButton;
    unsafe impl NSObjectProtocol for HoverButton {}
    impl HoverButton {
        #[unsafe(method(mouseEntered:))] fn entered(&self, _event: &NSEvent) { if self.isEnabled() { self.highlight(true); } }
        #[unsafe(method(mouseExited:))] fn exited(&self, _event: &NSEvent) { self.highlight(false); }
    }
);
fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect { NSRect::new(NSPoint::new(x, y), NSSize::new(width, height)) }
fn label(view: &NSView, text: &str, x: f64, y: f64, width: f64, size: f64, bold: bool, secondary: bool, marker: MainThreadMarker) -> Retained<NSTextField> {
    let field = NSTextField::labelWithString(&NSString::from_str(text), marker);
    // NSTextField adds two points of horizontal cell padding. Compensate so
    // text, bars and action controls share the same content edges.
    field.setFrame(rect(x - 2.0, y, width + 4.0, 18.0));
    let font = if bold { NSFont::boldSystemFontOfSize(size) } else { NSFont::systemFontOfSize(size) };
    let color = if secondary { NSColor::secondaryLabelColor() } else { NSColor::labelColor() };
    field.setFont(Some(&font)); field.setTextColor(Some(&color));
    field.setSelectable(false);
    view.addSubview(&field);
    field
}
fn quota_color(value: Option<f64>, preferences: &MenuBarPreferences) -> Retained<NSColor> {
    match projection::quota_tone(value, preferences) { projection::QuotaTone::Healthy => NSColor::systemGreenColor(),
        projection::QuotaTone::Warning => NSColor::systemYellowColor(), projection::QuotaTone::Critical => NSColor::systemRedColor(),
        projection::QuotaTone::Unknown => NSColor::secondaryLabelColor() }
}
fn bar(view: &NSView, value: Option<f64>, preferences: &MenuBarPreferences, frame: NSRect, disabled: bool, marker: MainThreadMarker) -> Retained<QuotaBar> {
    let this = QuotaBar::alloc(marker).set_ivars(BarState { value: Cell::new(value), preferences: preferences.clone(), disabled: Cell::new(disabled) });
    let progress: Retained<QuotaBar> = unsafe { msg_send![super(this), initWithFrame: frame] };
    view.addSubview(&progress); progress
}
fn symbol(name: &str) -> Option<Retained<NSImage>> { NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(name), None) }
fn image(view: &NSView, image: Option<Retained<NSImage>>, frame: NSRect, marker: MainThreadMarker) {
    if let Some(image) = image { let field = NSImageView::new(marker); field.setFrame(frame); field.setImage(Some(&image)); view.addSubview(&field); }
}
fn button(view: &NSView, menu: &NSMenu, app: &tauri::AppHandle, title: &str, action: Action, enabled: bool, frame: NSRect, icon: Option<&str>, zh: bool, targets: &mut Vec<Retained<MenuAction>>, marker: MainThreadMarker) -> Retained<NSButton> {
    let target = MenuAction::new(marker, ActionState { app: app.clone(), menu: menu.into(), action, zh });
    let button = if matches!(target.ivars().action, Action::Switch(_)) {
        let button: Retained<HoverButton> = unsafe { msg_send![HoverButton::alloc(marker), initWithFrame: frame] };
        button.setTitle(&NSString::from_str(title));
        unsafe { button.setTarget(Some(&target)); button.setAction(Some(sel!(perform:))); }
        track_hover(&button);
        Retained::into_super(button)
    } else { unsafe { NSButton::buttonWithTitle_target_action(&NSString::from_str(title), Some(&target), Some(sel!(perform:)), marker) } };
    button.setFrame(frame); button.setFont(Some(&NSFont::systemFontOfSize(11.0))); button.setBordered(true);
    button.setBezelStyle(NSBezelStyle::Push); button.setControlSize(NSControlSize::Small); button.setEnabled(enabled);
    if let Some(image) = icon.and_then(symbol) { button.setImage(Some(&image)); button.setImagePosition(NSCellImagePosition::ImageLeading); }
    view.addSubview(&button); targets.push(target); button
}
// Read-only status chips keep their semantic color instead of AppKit's
// disabled-control dimming. They share the switch button's layout footprint.
define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    struct StatusBadge;
    unsafe impl NSObjectProtocol for StatusBadge {}
    impl StatusBadge {
        #[unsafe(method(isFlipped))] fn flipped(&self) -> bool { true }
        #[unsafe(method(drawRect:))]
        fn draw(&self, _dirty: NSRect) {
            let shape = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect(0.5, 3.5, self.bounds().size.width - 1.0, 19.0), 5.0, 5.0);
            NSColor::quaternaryLabelColor().setFill(); shape.fill();
        }
    }
);
fn status_badge(view: &NSView, title: &str, icon: &str, color: &NSColor, frame: NSRect, marker: MainThreadMarker) -> (Retained<StatusBadge>, Retained<NSTextField>) {
    let badge: Retained<StatusBadge> = unsafe { msg_send![StatusBadge::alloc(marker), initWithFrame: frame] };
    let text_width = if title.is_ascii() { title.len() as f64 * 5.5 } else { title.chars().count() as f64 * 11.0 };
    let glyph_x = (frame.size.width - 19.0 - text_width) / 2.0;
    let glyph = NSImageView::new(marker); glyph.setFrame(rect(glyph_x, 6.0, 14.0, 14.0));
    glyph.setImage(symbol(icon).as_deref()); glyph.setContentTintColor(Some(color)); badge.addSubview(&glyph);
    let text = label(&badge, title, glyph_x + 19.0, 5.0, text_width, 11.0, false, false, marker);
    text.setTextColor(Some(color)); view.addSubview(&badge);
    (badge, text)
}
struct UsageRingState { values: Cell<[f64; 3]> }
define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = UsageRingState]
    struct UsageRing;
    unsafe impl NSObjectProtocol for UsageRing {}
    impl UsageRing {
        #[unsafe(method(isFlipped))] fn flipped(&self) -> bool { true }
        #[unsafe(method(drawRect:))]
        fn draw(&self, _dirty: NSRect) {
            let size = self.bounds().size.width;
            let track = NSBezierPath::bezierPathWithOvalInRect(rect(4.0, 4.0, size - 8.0, size - 8.0));
            NSColor::quaternaryLabelColor().setStroke(); track.setLineWidth(6.0); track.stroke();
            let values = self.ivars().values.get();
            let total: f64 = values.iter().sum();
            if total <= 0.0 { return; }
            let mut angle = -90.0;
            for (index, value) in values.iter().enumerate() {
                if *value <= 0.0 { continue; }
                let next = angle + value / total * 360.0;
                let arc = NSBezierPath::bezierPath();
                usage_color(index).setStroke(); arc.setLineWidth(6.0);
                arc.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(NSPoint::new(size / 2.0, size / 2.0), (size - 8.0) / 2.0, angle, next, false);
                arc.stroke(); angle = next;
            }
        }
    }
);
fn usage_color(index: usize) -> Retained<NSColor> {
    let (r, g, b) = match index { 0 => (82.0, 163.0, 216.0), 1 => (234.0, 191.0, 83.0), _ => (98.0, 182.0, 157.0) };
    NSColor::colorWithSRGBRed_green_blue_alpha(r / 255.0, g / 255.0, b / 255.0, 1.0)
}
fn compact_tokens(value: u64) -> String {
    if value >= 1_000_000 { format!("{:.1}M", value as f64 / 1_000_000.0) }
    else if value >= 1_000 { format!("{:.1}K", value as f64 / 1_000.0) }
    else { value.to_string() }
}
struct UsageWidgets {
    ring: Retained<UsageRing>, scope: Retained<NSTextField>, total: Retained<NSTextField>,
    requests: Retained<NSTextField>, cost: Retained<NSTextField>, counts: Vec<Retained<NSTextField>>, status: Retained<NSTextField>,
}
impl UsageWidgets {
    fn apply(&self, usage: Option<&modules::menu_bar_usage::MenuBarUsage>, failed: bool, zh: bool) {
        let values = usage.map(|usage| [usage.today.input_tokens as f64, usage.today.output_tokens as f64, usage.today.cached_tokens as f64]).unwrap_or([0.0; 3]);
        self.ring.ivars().values.set(values); self.ring.setNeedsDisplay(true);
        self.scope.setStringValue(&NSString::from_str(if usage.is_some_and(|usage| usage.incomplete) { if zh { "统计不完整" } else { "Partial records" } } else { if zh { "本机" } else { "Local" } }));
        self.total.setStringValue(&NSString::from_str(&usage.map(|usage| compact_tokens(usage.today.total_tokens)).unwrap_or("—".into())));
        self.requests.setStringValue(&NSString::from_str(&usage.map(|usage| format!("{} {}", usage.today.request_count, if zh { "次请求" } else { "requests" })).unwrap_or("—".into())));
        let amount = usage.and_then(|usage| usage.estimated_usd).map(|usd| if usd > 0.0 && usd < 0.01 { format!("$ {usd:.4}") } else { format!("$ {usd:.2}") }).unwrap_or(if usage.is_some() { if zh { "未计价".into() } else { "Unpriced".into() } } else { "—".into() });
        self.cost.setStringValue(&NSString::from_str(&amount));
        for (index, field) in self.counts.iter().enumerate() { field.setStringValue(&NSString::from_str(&usage.map(|_| compact_tokens(values[index] as u64)).unwrap_or("—".into()))); }
        let status = if failed { if zh { "统计刷新失败" } else { "Usage refresh failed" } }
            else if let Some(usage) = usage { if usage.unpriced_models > 0 { if zh { "部分未计价" } else { "Partly unpriced" } } else if usage.pricing_stale && usage.today.total_tokens > 0 { if zh { "缓存价格" } else { "Cached prices" } } else { "" } }
            else { if zh { "正在读取" } else { "Loading usage" } };
        self.status.setStringValue(&NSString::from_str(status));
    }
}
fn usage_section(menu: &NSMenu, usage: Option<&modules::menu_bar_usage::MenuBarUsage>, zh: bool, marker: MainThreadMarker) -> UsageWidgets {
    let heading = section(marker, 29.0);
    label(&heading, if zh { "今日用量" } else { "Today's usage" }, 20.0, 5.0, 160.0, 13.0, true, false, marker);
    let scope = label(&heading, if usage.is_some_and(|usage| usage.incomplete) { if zh { "统计不完整" } else { "Partial records" } } else { if zh { "本机" } else { "Local" } }, 210.0, 7.0, WIDTH - 230.0, 11.0, false, true, marker);
    scope.setAlignment(objc2_app_kit::NSTextAlignment::Right);
    custom_item(menu, &heading, "Today's usage", marker);
    let view = section(marker, 119.0);
    let ring = UsageRing::alloc(marker).set_ivars(UsageRingState { values: Cell::new([0.0; 3]) });
    let ring: Retained<UsageRing> = unsafe { msg_send![super(ring), initWithFrame: rect(20.0, 5.0, 88.0, 88.0)] };
    view.addSubview(&ring);
    let total = label(&view, "—", 25.0, 33.0, 78.0, 17.0, true, false, marker);
    total.setAlignment(objc2_app_kit::NSTextAlignment::Center);
    let unit = label(&view, "tokens", 25.0, 53.0, 78.0, 9.0, false, true, marker);
    unit.setAlignment(objc2_app_kit::NSTextAlignment::Center);
    let requests = label(&view, "—", 20.0, 100.0, 88.0, 10.0, false, true, marker);
    requests.setAlignment(objc2_app_kit::NSTextAlignment::Center);
    label(&view, if zh { "API 费用估算" } else { "API estimate" }, 130.0, 5.0, 140.0, 11.0, false, true, marker);
    let cost = label(&view, "—", WIDTH - 105.0, 4.0, 85.0, 13.0, true, false, marker);
    cost.setAlignment(objc2_app_kit::NSTextAlignment::Right);
    let mut counts = Vec::new();
    for (index, name) in [if zh { "输入" } else { "Input" }, if zh { "输出" } else { "Output" }, if zh { "缓存" } else { "Cached" }].iter().enumerate() {
        let y = 32.0 + index as f64 * 21.0;
        let key = NSImageView::new(marker);
        key.setFrame(rect(130.0, y + 5.0, 8.0, 8.0)); key.setImage(symbol("minus").as_deref());
        key.setContentTintColor(Some(&usage_color(index))); view.addSubview(&key);
        label(&view, name, 142.0, y, 90.0, 11.0, false, true, marker);
        let value = label(&view, "—", WIDTH - 105.0, y, 85.0, 11.0, false, false, marker);
        value.setAlignment(objc2_app_kit::NSTextAlignment::Right);
        counts.push(value);
    }
    let status = label(&view, "", 130.0, 100.0, WIDTH - 150.0, 10.0, false, true, marker);
    status.setAlignment(objc2_app_kit::NSTextAlignment::Right);
    custom_item(menu, &view, "Local usage breakdown", marker);
    menu.addItem(&NSMenuItem::separatorItem(marker));
    let widgets = UsageWidgets { ring, scope, total, requests, cost, counts, status };
    widgets.apply(usage, false, zh); widgets
}
fn custom_item(menu: &NSMenu, view: &NSView, title: &str, marker: MainThreadMarker) -> Retained<NSMenuItem> {
    let item = unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(marker), &NSString::from_str(title), None, &NSString::from_str("")) };
    item.setView(Some(view)); menu.addItem(&item); item
}
fn standard_item(menu: &NSMenu, app: &tauri::AppHandle, title: &str, action: Action, enabled: bool, key: &str, show_icons: bool, zh: bool, targets: &mut Vec<Retained<MenuAction>>, marker: MainThreadMarker) {
    let icon = if show_icons { match &action {
        Action::Page("dashboard") => Some("chart.bar"), Action::Page("accounts") => Some("person.2"), Action::Page("settings") => Some("gearshape"),
        Action::Refresh => Some("arrow.clockwise"), Action::Github => Some("link"), Action::Quit => Some("power"), Action::Cancel(_) => Some("xmark.circle"), _ => None,
    } } else { None };
    let item = unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(marker), &NSString::from_str(title), Some(sel!(perform:)), &NSString::from_str(key)) };
    if let Some(image) = icon.and_then(symbol) { image.setSize(NSSize::new(16.0, 16.0)); item.setImage(Some(&image)); }
    let target = MenuAction::new(marker, ActionState { app: app.clone(), menu: menu.into(), action, zh });
    unsafe { item.setTarget(Some(&target)); }
    item.setEnabled(enabled); menu.addItem(&item); targets.push(target);
}
fn readonly_item(menu: &NSMenu, title: &str, marker: MainThreadMarker) {
    let item = unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(marker), &NSString::from_str(title), None, &NSString::from_str("")) };
    item.setEnabled(false); menu.addItem(&item);
}
struct AccountControls { id: String, switch: Retained<NSButton>, badge: Retained<StatusBadge>, caption: Retained<NSTextField> }
struct AccountQuotaWidgets {
    id: String,
    rows: Vec<(usize, QuotaRow)>,
    resets: Vec<(usize, usize, Retained<NSTextField>)>,
}
impl AccountQuotaWidgets {
    fn apply(&self, account: Option<&DashboardEntry>, windows: [[Option<f64>; 2]; 2], now: i64, zh: bool) {
        let disabled = account.is_some_and(|account| account.disabled);
        let windows = if disabled { [[Some(0.0); 2]; 2] } else { windows };
        for (period, row) in &self.rows {
            for (family, (progress, text)) in row.cells.iter().enumerate() {
                progress.update(windows[*period][family], disabled);
                text.setStringValue(&NSString::from_str(&projection::percent(windows[*period][family])));
            }
        }
        let labels = account.map(|account| projection::account_reset_labels(account, now, zh));
        for (period, family, text) in &self.resets {
            text.setStringValue(&NSString::from_str(labels.as_ref().map(|labels| labels[*period][*family].as_str())
                .unwrap_or(if zh { "未报告" } else { "Unknown" })));
        }
    }
}
struct AggregateWidgets { period: usize, progress: Retained<QuotaBar>, stats: Retained<NSTextField> }
fn aggregate_text(remaining: Option<f64>, usable: usize, total: usize, zh: bool) -> String {
    format!("{} {usable}/{total}   {} {}", if zh { "可用账号" } else { "Available" }, if zh { "剩余" } else { "Left" }, projection::percent(remaining))
}
impl MenuSession {
    fn apply_quotas(&self, snapshot: Option<&DashboardSnapshot>) {
        let now = chrono::Utc::now().timestamp();
        let ids: Vec<_> = self.quotas.iter().map(|row| row.id.as_str()).collect();
        let windows = projection::open_menu_windows(snapshot, &ids, &self.preferences, now, self.freshness_minutes);
        for (row, &windows) in self.quotas.iter().zip(&windows) {
            let account = snapshot.and_then(|snapshot| snapshot.accounts.iter().find(|account| account.id == row.id));
            row.apply(account, windows, now, self.zh);
        }
        for aggregate in &self.aggregates {
            let (remaining, usable, _) = projection::aggregate(&windows, self.preferences.quota_scope, aggregate.period, self.reserve);
            aggregate.progress.update(remaining, false);
            aggregate.stats.setStringValue(&NSString::from_str(&aggregate_text(remaining, usable, self.quotas.len(), self.zh)));
        }
        for control in &self.controls {
            let switchable = snapshot.and_then(|snapshot| snapshot.accounts.iter().find(|account| account.id == control.id))
                .is_some_and(|account| projection::switchable_account(account, now));
            control.switch.setEnabled(!self.busy && !BUSY.load(Ordering::Acquire) && switchable);
        }
    }
}
fn account_item(menu: &NSMenu, app: &tauri::AppHandle, account: &DashboardEntry, windows: [[Option<f64>; 2]; 2], preferences: &MenuBarPreferences, busy: bool, zh: bool, targets: &mut Vec<Retained<MenuAction>>, marker: MainThreadMarker) -> (Option<AccountControls>, AccountQuotaWidgets) {
    let (primary, secondary) = projection::identity_parts(account, preferences);
    let title = if preferences.display_scope == MenuBarQuotaScope::All || secondary.is_empty() { primary.clone() } else { format!("{primary}   {secondary}") };
    let periods: Vec<_> = (0..2).filter(|period| if *period == 0 { preferences.show_session } else { preferences.show_weekly }).collect();
    let mode = preferences.reset_time_mode();
    let (_, column_width) = quota_column(preferences.display_scope, 0);
    let divider = (preferences.display_scope == MenuBarQuotaScope::All && !periods.is_empty())
        .then(|| rect(70.0 + column_width + FAMILY_GAP / 2.0, 43.0, 0.5, periods.len() as f64 * 18.0 - 2.0));
    let view = AccountRow::alloc(marker).set_ivars(AccountRowState { mode, divider, ..Default::default() });
    let view: Retained<AccountRow> = unsafe { msg_send![super(view), initWithFrame: rect(0.0, 0.0, WIDTH, 48.0 + periods.len() as f64 * 18.0)] };
    let action_width = if zh { 64.0 } else { 72.0 };
    let action_frame = rect(WIDTH - 20.0 - action_width, 8.0, action_width, 26.0);
    label(&view, &primary, 20.0, 7.0, WIDTH - action_width - 48.0, 12.0, true, false, marker);
    if preferences.display_scope == MenuBarQuotaScope::All {
        for (family, name) in ["Gemini", "Claude / GPT"].iter().enumerate() {
            let (x, width) = quota_column(preferences.display_scope, family);
            label(&view, name, x - 50.0, 25.0, width + 50.0, 10.0, false, true, marker);
        }
    } else if !secondary.is_empty() { label(&view, &secondary, 20.0, 25.0, WIDTH - action_width - 48.0, 10.0, false, true, marker); }
    let controls = if account.disabled {
        status_badge(&view, if zh { "禁用" } else { "Disabled" }, "nosign", &NSColor::systemRedColor(), action_frame, marker);
        None
    } else {
        let switch = button(&view, menu, app, if zh { "切换" } else { "Switch" }, Action::Switch(account.id.clone()), !busy && projection::switchable_account(account, chrono::Utc::now().timestamp()), action_frame, Some("arrow.left.arrow.right"), zh, targets, marker);
        let (badge, caption) = status_badge(&view, if zh { "当前" } else { "Current" }, "checkmark.circle", &NSColor::systemBlueColor(), action_frame, marker);
        badge.setHidden(true);
        Some(AccountControls { id: account.id.clone(), switch, badge, caption })
    };
    // Disabled rows display usable quota as zero without changing the cached
    // observations or the aggregate calculation, which still excludes them.
    let windows = if account.disabled { [[Some(0.0); 2]; 2] } else { windows };
    let resets = projection::account_reset_labels(account, chrono::Utc::now().timestamp(), zh);
    let mut widgets = AccountQuotaWidgets { id: account.id.clone(), rows: Vec::new(), resets: Vec::new() };
    for (row, &period) in periods.iter().enumerate() {
        let y = 46.0 + row as f64 * 18.0;
        let cells = (0..2).map(|family| {
            let visible = preferences.display_scope == MenuBarQuotaScope::All || (preferences.display_scope == MenuBarQuotaScope::Gemini && family == 0) || (preferences.display_scope == MenuBarQuotaScope::Other && family == 1);
            let period_name = if period == 0 { if zh { "5 小时" } else { "5 hours" } } else { if zh { "每周" } else { "Weekly" } };
            let family_name = if family == 0 { "Gemini" } else { "Claude / GPT" };
            let tooltip = NSString::from_str(&format!("{family_name} · {period_name}"));
            if visible {
                let (x, _) = quota_column(preferences.display_scope, family);
                let period_label = label(&view, period_name, x - 50.0, y - 3.0, 48.0, 11.0, false, true, marker);
                period_label.setToolTip(Some(&tooltip));
            }
            let progress = bar(&view, windows[period][family], preferences, rect(70.0, y + 3.0, 100.0, 4.0), account.disabled, marker);
            progress.setToolTip(Some(&tooltip));
            let field = label(&view, &projection::percent(windows[period][family]), 180.0, y - 3.0, 42.0, 11.0, false, true, marker);
            field.setToolTip(Some(&tooltip));
            let color = NSColor::labelColor();
            field.setTextColor(Some(&color)); field.setAlignment(objc2_app_kit::NSTextAlignment::Right);
            (progress, field)
        }).collect();
        let row = QuotaRow { cells, y }; row.apply(preferences.display_scope);
        if mode != MenuBarResetTimeDisplay::Hidden {
            for (family, (progress, _)) in row.cells.iter().enumerate().filter(|(_, (progress, _))| !progress.isHidden()) {
                let frame = progress.frame();
                let reset_width = if preferences.display_scope == MenuBarQuotaScope::All { 54.0 } else { 76.0 };
                let compact = rect(frame.origin.x, frame.origin.y, frame.size.width - reset_width - 6.0, frame.size.height);
                let reset = section(marker, 18.0);
                reset.setFrame(rect(compact.origin.x + compact.size.width + 6.0, y - 3.0, reset_width, 18.0));
                image(&reset, symbol("clock"), rect(0.0, 4.0, 9.0, 9.0), marker);
                let text = label(&reset, &resets[period][family], 12.0, 0.0, reset_width - 12.0, 9.0, false, true, marker);
                widgets.resets.push((period, family, text));
                reset.setHidden(true); view.addSubview(&reset);
                view.ivars().resets.borrow_mut().push(HoverQuota { bar: progress.clone(), reset, full: frame, compact });
            }
        }
        widgets.rows.push((period, row));
    }
    if mode == MenuBarResetTimeDisplay::Hover { track_hover(&view); }
    else if mode == MenuBarResetTimeDisplay::Always { view.show_resets(true); }
    custom_item(menu, &view, &title, marker);
    (controls, widgets)
}

enum MenuUpdate { Quotas(Result<DashboardSnapshot, String>), Identity(Option<String>, &'static str), Usage(Result<modules::menu_bar_usage::MenuBarUsage, String>) }
struct MenuRefreshState { updates: mpsc::Receiver<MenuUpdate>, ticket: u64, started: Instant }
define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = MenuRefreshState]
    struct MenuRefresh;
    unsafe impl NSObjectProtocol for MenuRefresh {}
    impl MenuRefresh {
        #[unsafe(method(poll:))]
        fn poll(&self, _timer: &NSTimer) {
            if GENERATION.load(Ordering::Acquire) != self.ivars().ticket { return; }
            for update in self.ivars().updates.try_iter() {
                SESSION.with(|session| {
                    let mut session = session.borrow_mut();
                    let Some(session) = session.as_mut() else { return; };
                    match update {
                        MenuUpdate::Identity(current, source) => { for control in &session.controls {
                            let selected = current.as_ref() == Some(&control.id);
                            let verified = projection::verified_current(&control.id, current.as_deref(), source);
                            control.switch.setHidden(verified); control.badge.setHidden(!verified);
                            control.switch.setTitle(&NSString::from_str(if selected && !verified { if session.zh { "记录" } else { "Saved" } } else { if session.zh { "切换" } else { "Switch" } }));
                            control.caption.setStringValue(&NSString::from_str(if session.zh { "当前" } else { "Current" }));
                        }
                            session.identity_summary.setStringValue(&NSString::from_str(&projection::account_heading_summary(session.quotas.len(), current.as_deref(), source, session.zh)));
                            modules::logger::log_info(&format!("Native menu identity applied in {} ms ({source})", self.ivars().started.elapsed().as_millis()));
                        },
                        MenuUpdate::Quotas(snapshot) => session.apply_quotas(snapshot.as_ref().ok()),
                        MenuUpdate::Usage(Ok(usage)) => session.usage.apply(Some(&usage), false, session.zh),
                        MenuUpdate::Usage(Err(_)) => session.usage.apply(modules::menu_bar_usage::cached().as_ref(), true, session.zh),
                    }
                });
            }
        }
    }
);
fn show(app: tauri::AppHandle, config: AppConfig, snapshot: Option<DashboardSnapshot>, usage: Option<modules::menu_bar_usage::MenuBarUsage>, status: Option<modules::auto_switch::Status>, reserve: u8, ticket: u64, updates: mpsc::Receiver<MenuUpdate>, started: Instant) {
    let Some(marker) = MainThreadMarker::new() else { OPEN.store(false, Ordering::Release); return; };
    if !OPEN.load(Ordering::Acquire) || GENERATION.load(Ordering::Acquire) != ticket { return; }
    let zh = config.language.starts_with("zh");
    let menu = NSMenu::new(marker); menu.setAutoenablesItems(false); menu.setMinimumWidth(WIDTH);
    let mut targets = Vec::new();
    let now = chrono::Utc::now().timestamp();
    let preferences = &config.menu_bar;
    let accounts: Vec<_> = snapshot.as_ref().map(|snapshot| snapshot.accounts.iter().filter(|account| projection::visible_account(account, preferences, now)).collect()).unwrap_or_default();
    let windows: Vec<_> = accounts.iter().map(|account| projection::account_windows(account, now, config.refresh_interval)).collect();
    let scope = config.menu_bar.quota_scope;
    let scope_name = match scope { crate::models::config::MenuBarQuotaScope::All => if zh { "Gemini 与 Claude/GPT" } else { "Gemini / Claude & GPT" }, crate::models::config::MenuBarQuotaScope::Gemini => if zh { "Gemini 系列" } else { "Gemini" }, crate::models::config::MenuBarQuotaScope::Other => if zh { "Claude 和 GPT 系列" } else { "Claude & GPT" } };
    let header = section(marker, 34.0);
    if preferences.show_icons { image(&header, NSApplication::sharedApplication(marker).applicationIconImage(), rect(20.0, 5.0, 24.0, 24.0), marker); }
    let title_x = if preferences.show_icons { 53.0 } else { 20.0 };
    label(&header, BRAND, title_x, 8.0, WIDTH - title_x - 20.0, 13.0, true, false, marker);
    custom_item(&menu, &header, BRAND, marker);
    menu.addItem(&NSMenuItem::separatorItem(marker));
    let usage = usage_section(&menu, usage.as_ref(), zh, marker);
    let mut aggregates = Vec::new();
    if preferences.show_aggregate {
      let heading = section(marker, 29.0);
      label(&heading, if zh { "剩余额度" } else { "Remaining quota" }, 20.0, 5.0, 140.0, 13.0, true, false, marker);
      let summary = label(&heading, &format!("{scope_name}  {}", if zh { "平均剩余" } else { "Mean remaining" }), 80.0, 7.0, WIDTH - 100.0, 11.0, false, true, marker);
      summary.setAlignment(objc2_app_kit::NSTextAlignment::Right);
      custom_item(&menu, &heading, "Overall quotas", marker);
    for period in (0..2).filter(|period| if *period == 0 { preferences.show_session } else { preferences.show_weekly }) {
        let (remaining, usable, _) = projection::aggregate(&windows, scope, period, reserve);
        let view = section(marker, 48.0);
        label(&view, if period == 0 { if zh { "5 小时" } else { "5 hours" } } else { if zh { "每周" } else { "Weekly" } }, 20.0, 5.0, 85.0, 13.0, true, false, marker);
        let stats = label(&view, &aggregate_text(remaining, usable, accounts.len(), zh), 118.0, 7.0, WIDTH - 138.0, 11.0, false, true, marker);
        stats.setAlignment(objc2_app_kit::NSTextAlignment::Right);
        let progress = bar(&view, remaining, preferences, rect(20.0, 30.0, WIDTH - 40.0, 6.0), false, marker);
        aggregates.push(AggregateWidgets { period, progress, stats });
        custom_item(&menu, &view, if period == 0 { "5 hours" } else { "Weekly" }, marker);
    }
    menu.addItem(&NSMenuItem::separatorItem(marker));
    }
    let account_header = section(marker, 29.0);
    label(&account_header, if zh { "账号列表" } else { "Accounts" }, 20.0, 4.0, 160.0, 13.0, true, false, marker);
    let count = label(&account_header, &projection::account_heading_summary(accounts.len(), None, "checking", zh), 180.0, 5.0, WIDTH - 200.0, 11.0, false, true, marker);
    count.setAlignment(objc2_app_kit::NSTextAlignment::Right);
    custom_item(&menu, &account_header, "Accounts", marker);
    let busy = BUSY.load(Ordering::Acquire) || status.as_ref().is_none_or(|status| status.phase == "switching");
    if snapshot.is_none() { readonly_item(&menu, if zh { "账号读取失败，请重试" } else { "Could not read accounts. Retry." }, marker); }
    else if accounts.is_empty() { readonly_item(&menu, if zh { "尚未添加账号" } else { "No saved accounts" }, marker); }
    let mut controls = Vec::new();
    let mut quotas = Vec::new();
    for (account, windows) in accounts.iter().zip(windows) {
        let (control, widgets) = account_item(&menu, &app, account, windows, preferences, busy, zh, &mut targets, marker);
        if let Some(control) = control { controls.push(control); }
        quotas.push(widgets);
    }
    menu.addItem(&NSMenuItem::separatorItem(marker));
    if let Ok(mut notice) = NOTICE.lock() { if let Some(notice) = notice.take() { readonly_item(&menu, &notice, marker); } }
    if BUSY.load(Ordering::Acquire) { readonly_item(&menu, if zh { "正在执行，请稍候…" } else { "Working…" }, marker); }
    if let Some(status) = status.filter(|status| status.pending_id.is_some()) {
        let message = match status.reason.as_deref() {
            Some("waiting_task_finish") => if zh { "智能换号：等待所有任务结束" } else { "Auto switch: waiting for all tasks" },
            Some("task_state_unknown") => if zh { "智能换号：正在确认任务状态" } else { "Auto switch: verifying task state" },
            Some("client_close_failed") => if zh { "智能换号：自动关闭失败，将重试" } else { "Auto switch: close failed; retrying" },
            Some("closing_clients" | "clients_running") => if zh { "智能换号：正在自动关闭客户端" } else { "Auto switch: closing clients automatically" },
            _ => if zh { "智能换号：正在准备切换" } else { "Auto switch: preparing to switch" },
        };
        readonly_item(&menu, message, marker);
        standard_item(&menu, &app, if zh { "取消待切换操作" } else { "Cancel pending switch" }, Action::Cancel(status.pending_id.unwrap()), !busy, "", preferences.show_icons, zh, &mut targets, marker);
    }
    standard_item(&menu, &app, if zh { "刷新全部额度" } else { "Refresh All Quotas" }, Action::Refresh, !busy, "r", preferences.show_icons, zh, &mut targets, marker);
    standard_item(&menu, &app, if zh { "用量看板" } else { "Usage Dashboard" }, Action::Page("dashboard"), true, "", preferences.show_icons, zh, &mut targets, marker);
    standard_item(&menu, &app, if zh { "管理账号" } else { "Manage Accounts" }, Action::Page("accounts"), true, "", preferences.show_icons, zh, &mut targets, marker);
    menu.addItem(&NSMenuItem::separatorItem(marker));
    standard_item(&menu, &app, if zh { "设置" } else { "Settings" }, Action::Page("settings"), true, ",", preferences.show_icons, zh, &mut targets, marker);
    standard_item(&menu, &app, "GitHub ↗", Action::Github, true, "", preferences.show_icons, zh, &mut targets, marker);
    standard_item(&menu, &app, if zh { "退出" } else { "Quit" }, Action::Quit, true, "q", preferences.show_icons, zh, &mut targets, marker);
    SESSION.with(|session| *session.borrow_mut() = Some(MenuSession { zh, _targets: targets, controls, identity_summary: count, usage, quotas, aggregates,
        preferences: preferences.clone(), freshness_minutes: config.refresh_interval, reserve, busy }));
    ACTIVE.with(|active| *active.borrow_mut() = Some(menu.clone()));
    let refresh = MenuRefresh::alloc(marker).set_ivars(MenuRefreshState { updates, ticket, started });
    let refresh: Retained<MenuRefresh> = unsafe { msg_send![super(refresh), init] };
    let timer = unsafe { NSTimer::timerWithTimeInterval_target_selector_userInfo_repeats(0.05, &refresh, sel!(poll:), None, true) };
    unsafe {
        NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes);
        NSRunLoop::mainRunLoop().addTimer_forMode(&timer, objc2_app_kit::NSEventTrackingRunLoopMode);
        let _: () = msg_send![&*refresh, poll: &*timer];
    }
    modules::logger::log_info(&format!("Native menu ready in {} ms ({} account rows)", started.elapsed().as_millis(), accounts.len()));
    // Associate the NSMenu with the real status item. AppKit then owns the
    // anchor, active screen, accessibility hierarchy and native menu tracking.
    // This synchronous closure runs on the main thread while `menu` is retained
    // here; only its address crosses Tauri's Send bound, never another thread.
    let menu_address = Retained::as_ptr(&menu) as usize;
    let shown = app.tray_by_id("main").and_then(|tray| tray.with_inner_tray_icon(move |icon| {
        let marker = MainThreadMarker::new().expect("tray callback is on main thread");
        let menu = unsafe { &*(menu_address as *const NSMenu) };
        let Some(status_item) = icon.ns_status_item() else { return false; };
        let previous = status_item.menu(marker);
        status_item.setMenu(Some(menu));
        // tray-icon overlays its own mouse view on the status button. Open
        // from the status item itself instead of synthesizing a button click.
        #[allow(deprecated)]
        status_item.popUpStatusItemMenu(menu);
        status_item.setMenu(previous.as_deref());
        true
    }).ok()).unwrap_or(false);
    if !shown { menu.popUpMenuPositioningItem_atLocation_inView(None, objc2_app_kit::NSEvent::mouseLocation(), None); }
    timer.invalidate();
    ACTIVE.with(|active| active.borrow_mut().take());
    SESSION.with(|session| session.borrow_mut().take());
    if GENERATION.load(Ordering::Acquire) == ticket { OPEN.store(false, Ordering::Release); }
}

pub fn toggle(app: &tauri::AppHandle, _anchor: Option<tauri::Rect>) -> Result<(), String> {
    let started = Instant::now();
    let ticket = GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    if OPEN.swap(true, Ordering::AcqRel) {
        OPEN.store(false, Ordering::Release);
        return app.run_on_main_thread(|| ACTIVE.with(|active| { if let Some(menu) = active.borrow().as_ref() { menu.cancelTrackingWithoutAnimation(); } })).map_err(|error| error.to_string());
    }
    let (send, updates) = mpsc::channel();
    let quota_sender = send.clone();
    tauri::async_runtime::spawn(async move {
        // Read local observations while tracking, including scheduler/CLI writes.
        // Keep disk I/O off AppKit's tracking loop; opening never refreshes credentials.
        while OPEN.load(Ordering::Acquire) && GENERATION.load(Ordering::Acquire) == ticket {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            if !OPEN.load(Ordering::Acquire) || GENERATION.load(Ordering::Acquire) != ticket { break; }
            let snapshot = commands::get_account_dashboard_snapshot().await;
            if quota_sender.send(MenuUpdate::Quotas(snapshot)).is_err() { break; }
        }
    });
    let identity_sender = send.clone();
    tauri::async_runtime::spawn(async move {
        let identity = commands::get_menu_bar_snapshot().await.ok();
        let current = identity.as_ref().and_then(|snapshot| snapshot.current_account_id.clone());
        let source = identity.as_ref().map(|snapshot| snapshot.current_identity_source).unwrap_or("unavailable");
        let _ = identity_sender.send(MenuUpdate::Identity(current, source));
    });
    tauri::async_runtime::spawn(async move { let _ = send.send(MenuUpdate::Usage(modules::menu_bar_usage::load().await)); });
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Only small local DTO/config reads are on the opening path. Never mark
        // the saved index as a verified live App before its background check.
        let snapshot = modules::account_dashboard::snapshot().ok();
        let usage = modules::menu_bar_usage::cached();
        if !OPEN.load(Ordering::Acquire) || GENERATION.load(Ordering::Acquire) != ticket { return; }
        let config = modules::load_app_config().unwrap_or_default();
        let status = modules::auto_switch::get_auto_switch_status(app.clone()).ok();
        let reserve = modules::auto_switch::get_auto_switch_config(app.clone()).map(|config| config.reserve_percentage).unwrap_or(10);
        let handle = app.clone();
        if handle.run_on_main_thread(move || show(app, config, snapshot, usage, status, reserve, ticket, updates, started)).is_err() { OPEN.store(false, Ordering::Release); }
    });
    Ok(())
}
