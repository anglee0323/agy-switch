//! Desktop lifecycle and the small menu-bar dashboard. OS login registration is
//! changed only by the explicit settings command, never on startup/config load.
use crate::{models::config::DesktopPreferences, modules};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};
#[cfg(not(target_os = "macos"))]
use tauri::{PhysicalPosition, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt;

pub const DASHBOARD_LABEL: &str = "menubar";

#[derive(Default)]
pub struct DesktopRuntime {
    tray_available: AtomicBool,
    preferences_lock: tokio::sync::Mutex<()>,
    dock_error: std::sync::Mutex<Option<String>>,
    #[cfg(target_os = "macos")]
    hide_dock_icon: AtomicBool,
    #[cfg(not(target_os = "macos"))]
    panel_transition: std::sync::Mutex<()>,
    #[cfg(target_os = "macos")]
    appearance_lock: std::sync::Mutex<()>,
    #[cfg(target_os = "macos")]
    material_applied: AtomicBool,
}

pub fn set_tray_available(app: &tauri::AppHandle, available: bool) {
    app.state::<DesktopRuntime>()
        .tray_available
        .store(available, Ordering::Relaxed);
}

pub fn tray_available(app: &tauri::AppHandle) -> bool {
    app.state::<DesktopRuntime>()
        .tray_available
        .load(Ordering::Relaxed)
}

#[derive(Clone, Serialize)]
pub struct MenuBarAppearance {
    platform: &'static str,
    native_material: bool,
    reduced_transparency: bool,
    high_contrast: bool,
    material_kind: &'static str,
}

fn use_native_material(macos: bool, reduced_transparency: bool, high_contrast: bool) -> bool {
    macos && !reduced_transparency && !high_contrast
}

/// Keyboard access to the same tray-anchored overview, including for users who
/// cannot target a small status icon with a pointer.
#[cfg(target_os = "macos")]
pub fn install_dashboard_shortcut(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::{Listener, menu::{Menu, MenuItem, PredefinedMenuItem, WINDOW_SUBMENU_ID}};
    let menu = match app.menu() { Some(menu) => menu, None => Menu::default(app)? };
    let label = || if modules::load_app_config().unwrap_or_default().language.starts_with("zh") { "额度总览" } else { "Quota Overview" };
    let item = MenuItem::with_id(app, "menubar-overview", label(), true, Some("CmdOrCtrl+Shift+M"))?;
    if let Some(submenu) = menu.get(WINDOW_SUBMENU_ID).and_then(|entry| entry.as_submenu().cloned()) {
        submenu.append(&PredefinedMenuItem::separator(app)?)?;
        submenu.append(&item)?;
    }
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| {
        if event.id().as_ref() == "menubar-overview" {
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(error) = toggle_dashboard(&app, None) {
                    modules::logger::log_warn(&format!("Overview unavailable: {error}"));
                }
            });
        }
    });
    app.listen("config://updated", move |_| { let _ = item.set_text(label()); });
    Ok(())
}

/// Read accessibility preferences without changing any system setting. Recheck
/// whenever the panel opens, including after a visit to System Settings.
#[tauri::command]
pub async fn get_menu_bar_appearance(app: tauri::AppHandle) -> Result<MenuBarAppearance, String> {
    // Native effects may dispatch to the UI thread. Never block that same thread
    // on the appearance mutex while another caller is applying a material.
    tauri::async_runtime::spawn_blocking(move || apply_menu_bar_appearance(&app))
        .await
        .map_err(|e| e.to_string())?
}

fn apply_menu_bar_appearance(app: &tauri::AppHandle) -> Result<MenuBarAppearance, String> {
    #[cfg(target_os = "macos")]
    let (reduced_transparency, high_contrast) = {
        let workspace = objc2_app_kit::NSWorkspace::sharedWorkspace();
        (
            workspace.accessibilityDisplayShouldReduceTransparency(),
            workspace.accessibilityDisplayShouldIncreaseContrast(),
        )
    };
    #[cfg(not(target_os = "macos"))]
    let (reduced_transparency, high_contrast) = (false, false);
    let native_material = use_native_material(
        cfg!(target_os = "macos"),
        reduced_transparency,
        high_contrast,
    );
    #[cfg(target_os = "macos")]
    let (native_material, material_kind) = {
        let state = app.state::<DesktopRuntime>();
        let _guard = state.appearance_lock.lock().map_err(|e| e.to_string())?;
        if let Some(window) = app.get_webview_window(DASHBOARD_LABEL) {
            let applied = state.material_applied.load(Ordering::Relaxed);
            // Use AppKit's menu/popover material through Tauri. Preserve its
            // managed root view; replacing it with NSGlassEffectView can abort
            // during native focus/termination events.
            if native_material && !applied {
                use tauri::window::{Effect, EffectState, EffectsBuilder};
                let success = window
                    .set_effects(
                        EffectsBuilder::new()
                            .effect(Effect::Popover)
                            .state(EffectState::Active)
                            .radius(22.0)
                            .build(),
                    )
                    .is_ok();
                state.material_applied.store(success, Ordering::Relaxed);
                (success, if success { "vibrancy" } else { "opaque" })
            } else if !native_material {
                if applied {
                    let _ = window.set_effects(None);
                }
                state.material_applied.store(false, Ordering::Relaxed);
                (false, "opaque")
            } else {
                (true, "vibrancy")
            }
        } else {
            (false, "opaque")
        }
    };
    #[cfg(not(target_os = "macos"))]
    let _ = app;
    #[cfg(not(target_os = "macos"))]
    let material_kind = "opaque";
    Ok(MenuBarAppearance {
        platform: std::env::consts::OS,
        native_material,
        reduced_transparency,
        high_contrast,
        material_kind,
    })
}

#[derive(Serialize)]
pub struct DesktopStatus {
    platform: &'static str,
    tray_available: bool,
    autostart_supported: bool,
    launch_at_login: Option<bool>,
    autostart_error: Option<String>,
    dock_error: Option<String>,
    hide_dock_icon: bool,
    start_minimized: bool,
}

#[derive(Default, Deserialize)]
pub struct DesktopPatch {
    launch_at_login: Option<bool>,
    hide_dock_icon: Option<bool>,
    start_minimized: Option<bool>,
}

#[tauri::command]
pub fn get_desktop_settings(app: tauri::AppHandle) -> Result<DesktopStatus, String> {
    let preferences = modules::load_app_config()?.desktop;
    // Query the OS rather than trusting a JSON flag after a System Settings edit.
    let (launch_at_login, autostart_error) = match app.autolaunch().is_enabled() {
        Ok(enabled) => (Some(enabled), None),
        Err(error) => (None, Some(error.to_string())),
    };
    Ok(DesktopStatus {
        platform: std::env::consts::OS,
        tray_available: tray_available(&app),
        autostart_supported: !cfg!(debug_assertions),
        launch_at_login,
        autostart_error,
        dock_error: dock_status_error(&app, preferences.hide_dock_icon),
        hide_dock_icon: preferences.hide_dock_icon,
        start_minimized: preferences.start_minimized,
    })
}

#[tauri::command]
pub async fn set_desktop_preferences(
    app: tauri::AppHandle,
    patch: DesktopPatch,
) -> Result<DesktopStatus, String> {
    let state = app.state::<DesktopRuntime>();
    let _guard = state.preferences_lock.lock().await;
    let old = modules::load_app_config()?.desktop;
    let mut next = old.clone();
    if let Some(value) = patch.hide_dock_icon {
        if !cfg!(target_os = "macos") {
            return Err("Dock visibility is only supported on macOS".into());
        }
        if value && !tray_available(&app) {
            return Err("The menu bar is unavailable. Keep the Dock icon visible so the app stays accessible.".into());
        }
        next.hide_dock_icon = value;
    }
    if let Some(value) = patch.start_minimized {
        if value && !tray_available(&app) {
            return Err(
                "A working menu bar or tray is required to start in the background.".into(),
            );
        }
        next.start_minimized = value;
    }
    let old_autostart = if let Some(value) = patch.launch_at_login {
        if cfg!(debug_assertions) {
            return Err(
                "Launch at login is available in installed release builds, not development builds."
                    .into(),
            );
        }
        let actual = app.autolaunch().is_enabled().map_err(|e| e.to_string())?;
        next.launch_at_login = value;
        Some(actual)
    } else {
        None
    };
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || preference_transaction(
        &old,
        &next,
        old_autostart,
        |enabled| {
            let result = if enabled {
                handle.autolaunch().enable()
            } else {
                handle.autolaunch().disable()
            };
            result.map_err(|error| error.to_string())
        },
        |preferences| apply_dock_preference(&handle, preferences),
        || modules::set_saved_desktop_preferences(&next),
    )).await.map_err(|_| "Desktop preference task failed".to_string())??;
    let _ = app.emit("config://updated", ());
    get_desktop_settings(app.clone())
}

/// OS writes can partially succeed before returning an error. Mark each stage
/// before invoking it, then compensate every attempted stage using its snapshot.
fn preference_transaction(
    old: &DesktopPreferences,
    next: &DesktopPreferences,
    old_login: Option<bool>,
    mut login: impl FnMut(bool) -> Result<(), String>,
    mut dock: impl FnMut(&DesktopPreferences) -> Result<(), String>,
    persist: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let mut login_attempted = false;
    let mut dock_attempted = false;
    let result = (|| {
        if old_login.is_some_and(|value| value != next.launch_at_login) {
            login_attempted = true;
            login(next.launch_at_login)?;
        }
        dock_attempted = true;
        dock(next)?;
        persist()
    })();
    if let Err(error) = result {
        let dock_failure = if dock_attempted {
            dock(old).err()
        } else {
            None
        };
        let login_failure = if login_attempted {
            login(old_login.unwrap()).err()
        } else {
            None
        };
        return Err(rollback_failure(error, dock_failure, login_failure));
    }
    Ok(())
}

fn rollback_failure(error: String, dock: Option<String>, login: Option<String>) -> String {
    let mut message = error;
    if let Some(failure) = dock {
        message.push_str(&format!("; Dock setting rollback failed: {failure}"));
    }
    if let Some(failure) = login {
        message.push_str(&format!("; login setting rollback failed: {failure}"));
    }
    message
}

fn apply_dock_preference(
    app: &tauri::AppHandle,
    preferences: &DesktopPreferences,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return apply_dock_state(app, Some(preferences.hide_dock_icon));
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, preferences);
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn with_appkit<T: Send + 'static>(
    app: &tauri::AppHandle,
    action: impl FnOnce(objc2_foundation::MainThreadMarker) -> T + Send + 'static,
) -> Result<T, String> {
    if let Some(marker) = objc2_foundation::MainThreadMarker::new() {
        return Ok(action(marker));
    }
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let marker = objc2_foundation::MainThreadMarker::new().expect("AppKit requires the main thread");
        let _ = send.send(action(marker));
    }).map_err(|error| error.to_string())?;
    receive.recv().map_err(|_| "Dock state is unavailable".into())
}

#[cfg(target_os = "macos")]
fn dock_policy(hidden: bool, tray: bool) -> objc2_app_kit::NSApplicationActivationPolicy {
    if hidden && tray { objc2_app_kit::NSApplicationActivationPolicy::Accessory }
    else { objc2_app_kit::NSApplicationActivationPolicy::Regular }
}

#[cfg(target_os = "macos")]
fn apply_dock_state(app: &tauri::AppHandle, preference: Option<bool>) -> Result<(), String> {
    let handle = app.clone();
    with_appkit(app, move |marker| {
        let runtime = handle.state::<DesktopRuntime>();
        // Stage updates and native writes on the same UI thread as window events.
        // Reopen/close use this latest value; they never replay a disk snapshot.
        if let Some(hidden) = preference { runtime.hide_dock_icon.store(hidden, Ordering::Release); }
        let policy = dock_policy(runtime.hide_dock_icon.load(Ordering::Acquire), tray_available(&handle));
        let application = objc2_app_kit::NSApplication::sharedApplication(marker);
        let result = if application.activationPolicy() == policy
            || (application.setActivationPolicy(policy) && application.activationPolicy() == policy)
        { Ok(()) } else { Err("macOS did not apply the Dock visibility preference".to_string()) };
        // Tauri's dispatch success does not include AppKit's boolean result.
        *runtime.dock_error.lock().unwrap_or_else(|error| error.into_inner()) = result.as_ref().err().cloned();
        result
    })?
}

fn reconcile_dock_preference(app: &tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return apply_dock_state(app, None);
    #[cfg(not(target_os = "macos"))]
    { let _ = app; Ok(()) }
}

fn dock_status_error(app: &tauri::AppHandle, hidden: bool) -> Option<String> {
    let saved_error = app.state::<DesktopRuntime>().dock_error.lock()
        .unwrap_or_else(|error| error.into_inner()).clone();
    #[cfg(target_os = "macos")]
    {
        let policy = dock_policy(hidden, tray_available(app));
        let observed = with_appkit(app, move |marker| {
            objc2_app_kit::NSApplication::sharedApplication(marker).activationPolicy() == policy
        });
        saved_error.or_else(|| match observed {
            Ok(true) => None,
            Ok(false) => Some("Dock visibility does not match the saved preference".into()),
            Err(error) => Some(error),
        })
    }
    #[cfg(not(target_os = "macos"))]
    { let _ = hidden; saved_error }
}

pub fn initialize(app: &tauri::AppHandle) -> Result<(), String> {
    let config = modules::load_app_config().unwrap_or_default();
    apply_dock_preference(app, &config.desktop)?;
    let autostart = std::env::args().any(|arg| arg == "--autostart");
    if start_hidden(
        autostart,
        config.desktop.start_minimized,
        tray_available(app),
    ) {
        // Creating the native WebView can map a window despite its initial
        // visibility flag (notably WebKitGTK). Apply the requested state after
        // creation rather than relying on that flag alone.
        if let Some(window) = app.get_webview_window("main") {
            window.hide().map_err(|error| error.to_string())?;
        }
    } else {
        show_main(app)?;
    }
    Ok(())
}

fn start_hidden(autostart: bool, minimized: bool, available: bool) -> bool {
    (autostart || minimized) && available
}

pub fn show_main(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(popover) = app.get_webview_window(DASHBOARD_LABEL) {
        let _ = popover.hide();
    }
    let window = app
        .get_webview_window("main")
        .ok_or("Main window is unavailable")?;
    window.unminimize().map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    reconcile_dock_preference(app)
}

#[tauri::command]
pub fn open_app_page(app: tauri::AppHandle, page: String) -> Result<(), String> {
    let route = match page.as_str() {
        "dashboard" => "/",
        "accounts" => "/accounts",
        "settings" => "/settings",
        _ => return Err("Unknown application page".into()),
    };
    show_main(&app)?;
    app.emit_to("main", "app://navigate", route)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn hide_menu_bar_dashboard(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(DASHBOARD_LABEL) {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// Clamp in physical pixels; negative monitor origins and mixed DPI are valid.
#[cfg(any(not(target_os = "macos"), test))]
fn panel_bounds(
    anchor: (f64, f64, f64, f64),
    area: (f64, f64, f64, f64),
    scale: f64,
) -> (f64, f64, f64, f64) {
    let (ax, ay, aw, ah) = anchor;
    let (x, y, w, h) = area;
    let margin = 8.0 * scale;
    let width = (424.0 * scale).min((w - margin * 2.0).max(1.0));
    let height = (680.0 * scale).min((h - margin * 2.0).max(1.0));
    let px =
        (ax + aw / 2.0 - width / 2.0).clamp(x + margin, (x + w - width - margin).max(x + margin));
    let below = if ah > 0.0 { ay + ah + 2.0 * scale } else { (ay + ah).max(y) };
    let py = if below + height <= y + h - margin {
        below
    } else {
        ay - height
    };
    (
        px,
        py.clamp(y, (y + h - height).max(y)),
        width,
        height,
    )
}

pub fn toggle_dashboard(app: &tauri::AppHandle, rect: Option<tauri::Rect>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return modules::native_menu::toggle(app, rect);
    #[cfg(not(target_os = "macos"))]
    toggle_web_dashboard(app, rect)
}

/// Prepare the hidden WebView once; tray clicks reuse an already rendered panel.
#[cfg(not(target_os = "macos"))]
pub fn warm_dashboard(app: &tauri::AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let runtime = app.state::<DesktopRuntime>();
        let Ok(_transition) = runtime.panel_transition.lock() else { return; };
        if let Err(error) = web_dashboard(&app) { modules::logger::log_warn(&format!("Quick dashboard prewarm failed: {error}")); }
    });
}

#[cfg(not(target_os = "macos"))]
fn web_dashboard(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    match app.get_webview_window(DASHBOARD_LABEL) {
        Some(window) => Ok(window),
        None => {
            let builder =
                WebviewWindowBuilder::new(app, DASHBOARD_LABEL, WebviewUrl::App("menubar".into()))
                    .title("Antigravity Quick Dashboard")
                    .inner_size(424.0, 680.0)
                    .resizable(false)
                    .decorations(false)
                    .visible(false)
                    .skip_taskbar(true)
                    .always_on_top(true)
                    .shadow(true);
            builder.build().map_err(|e| e.to_string())
        }
    }

}

#[tauri::command]
pub fn open_project_page(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url("https://github.com/anglee0323/agy-switch", None::<&str>).map_err(|e| e.to_string())
}

#[cfg(not(target_os = "macos"))]
fn toggle_web_dashboard(app: &tauri::AppHandle, rect: Option<tauri::Rect>) -> Result<(), String> {
    let runtime = app.state::<DesktopRuntime>();
    let _transition = runtime.panel_transition.lock().map_err(|e| e.to_string())?;
    if let Some(window) = app.get_webview_window(DASHBOARD_LABEL) {
        if window.is_visible().unwrap_or(false) {
            return window.hide().map_err(|e| e.to_string());
        }
    }
    let window = web_dashboard(app)?;
    let anchor = rect.or_else(|| {
        app.tray_by_id("main")
            .and_then(|tray| tray.rect().ok().flatten())
    });
    let point = anchor
        .map(|r| r.position.to_physical::<f64>(1.0))
        .or_else(|| app.cursor_position().ok());
    let monitor = point
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let area = monitor.work_area();
        let anchor = anchor
            .map(|r| {
                let pos = r.position.to_physical::<f64>(scale);
                let size = r.size.to_physical::<f64>(scale);
                (pos.x, pos.y, size.width, size.height)
            })
            .unwrap_or((
                f64::from(area.position.x + area.size.width as i32),
                f64::from(area.position.y),
                0.0,
                0.0,
            ));
        let (x, y, width, height) = panel_bounds(
            anchor,
            (
                f64::from(area.position.x),
                f64::from(area.position.y),
                f64::from(area.size.width),
                f64::from(area.size.height),
            ),
            scale,
        );
        window
            .set_size(tauri::PhysicalSize::new(width as u32, height as u32))
            .map_err(|e| e.to_string())?;
        window
            .set_position(PhysicalPosition::new(x as i32, y as i32))
            .map_err(|e| e.to_string())?;
    } else {
        window.center().map_err(|e| e.to_string())?;
    }
    let appearance = apply_menu_bar_appearance(app)?;
    tracing::debug!(material = appearance.material_kind, "Menu bar appearance applied");
    let _ = window.emit("menubar://appearance", &appearance);
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    let _ = window.emit("menubar://opened", ());
    Ok(())
}

pub fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if window.label() == DASHBOARD_LABEL {
        match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            tauri::WindowEvent::Focused(false) => {
                let window = window.clone();
                // Let a second tray click toggle the still-visible panel before
                // handling blur, avoiding the familiar hide-then-reopen race.
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                    if !window.is_focused().unwrap_or(false) {
                        let _ = window.hide();
                    }
                });
            }
            _ => {}
        }
    } else if window.label() == "main" {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            if tray_available(window.app_handle()) {
                // Never hide the only recovery path when the tray failed.
                if window.hide().is_ok() {
                    api.prevent_close();
                    if let Err(error) = reconcile_dock_preference(window.app_handle()) {
                        modules::logger::log_warn(&format!("Dock state could not be restored after closing: {error}"));
                    }
                }
            } else {
                // Prewarming may have left a hidden secondary window alive.
                // Closing only main would leave an inaccessible runtime.
                window.app_handle().exit(0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        panel_bounds, preference_transaction, rollback_failure, start_hidden, use_native_material,
    };
    use crate::models::config::DesktopPreferences;
    use std::cell::{Cell, RefCell};

    #[test]
    fn partially_failed_login_write_restores_actual_snapshot() {
        let old = DesktopPreferences::default();
        let next = DesktopPreferences {
            launch_at_login: true,
            ..old.clone()
        };
        let actual_login = Cell::new(false);
        let calls = RefCell::new(Vec::new());
        let result = preference_transaction(
            &old,
            &next,
            Some(false),
            |enabled| {
                calls.borrow_mut().push(enabled);
                actual_login.set(enabled); // model the first write succeeding
                if enabled {
                    Err("second login write failed".into())
                } else {
                    Ok(())
                }
            },
            |_| panic!("Dock must not run after initial login failure"),
            || panic!("Config must not persist"),
        );
        assert_eq!(result.unwrap_err(), "second login write failed");
        assert!(!actual_login.get());
        assert_eq!(*calls.borrow(), [true, false]);
    }

    #[test]
    fn persistence_failure_rolls_back_both_os_settings_and_reports_both_failures() {
        let old = DesktopPreferences::default();
        let next = DesktopPreferences {
            launch_at_login: true,
            hide_dock_icon: true,
            ..old.clone()
        };
        let login_calls = RefCell::new(Vec::new());
        let dock_calls = RefCell::new(Vec::new());
        let result = preference_transaction(
            &old,
            &next,
            Some(false),
            |enabled| {
                login_calls.borrow_mut().push(enabled);
                if enabled {
                    Ok(())
                } else {
                    Err("login recovery denied".into())
                }
            },
            |preferences| {
                dock_calls.borrow_mut().push(preferences.hide_dock_icon);
                if preferences.hide_dock_icon {
                    Ok(())
                } else {
                    Err("dock recovery denied".into())
                }
            },
            || Err("config save failed".into()),
        );
        let error = result.unwrap_err();
        assert_eq!(*login_calls.borrow(), [true, false]);
        assert_eq!(*dock_calls.borrow(), [true, false]);
        assert!(error.contains("config save failed"));
        assert!(error.contains("dock recovery denied"));
        assert!(error.contains("login recovery denied"));
    }

    #[test]
    fn unchanged_actual_login_is_not_written_or_compensated() {
        let old = DesktopPreferences::default();
        let next = DesktopPreferences {
            launch_at_login: true,
            ..old.clone()
        };
        assert!(preference_transaction(
            &old,
            &next,
            Some(true),
            |_| panic!("Actual login already matches"),
            |_| Ok(()),
            || Ok(())
        )
        .is_ok());
    }
    #[test]
    fn rollback_failures_report_both_os_operations() {
        assert_eq!(
            rollback_failure("save failed".into(), None, None),
            "save failed"
        );
        let message = rollback_failure(
            "save failed".into(),
            Some("dock failed".into()),
            Some("login failed".into()),
        );
        assert!(message.contains("save failed"));
        assert!(message.contains("Dock setting rollback failed: dock failed"));
        assert!(message.contains("login setting rollback failed: login failed"));
    }
    #[test]
    fn material_requires_macos_and_accessibility_opt_in() {
        assert!(use_native_material(true, false, false));
        assert!(!use_native_material(false, false, false));
        assert!(!use_native_material(true, true, false));
        assert!(!use_native_material(true, false, true));
    }
    #[test]
    fn background_start_requires_every_safety_condition() {
        for autostart in [false, true] {
            for minimized in [false, true] {
                for tray in [false, true] {
                    assert_eq!(
                        start_hidden(autostart, minimized, tray),
                        (autostart || minimized) && tray
                    );
                }
            }
        }
    }
    #[test]
    fn popover_fits_small_negative_origin_monitor() {
        let (x, y, w, h) = panel_bounds(
            (-12.0, -32.0, 20.0, 24.0),
            (-800.0, -10.0, 800.0, 540.0),
            1.0,
        );
        assert!(x >= -792.0 && x + w <= -8.0);
        assert!(y >= -10.0 && y + h <= 530.0);
    }
    #[test]
    fn popover_scales_and_opens_above_bottom_tray() {
        let (x, y, w, h) = panel_bounds(
            (2800.0, 1760.0, 40.0, 40.0),
            (0.0, 0.0, 2880.0, 1760.0),
            2.0,
        );
        assert_eq!(w, 848.0);
        assert_eq!(h, 1360.0);
        assert!(x + w <= 2864.0 && y + h <= 1760.0);
    }
}
