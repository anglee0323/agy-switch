pub mod cli;
mod commands;
pub mod constants;
pub mod error;
#[cfg(target_os = "linux")]
mod linux_graphics;
mod models;
mod modules;
mod utils;

use tauri::Manager;
use tracing::{info, warn};

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn is_wayland_session() -> bool {
    std::env::var("WAYLAND_DISPLAY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
        || std::env::var("XDG_SESSION_TYPE")
            .map(|v| v.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
}

fn should_enable_tray() -> bool {
    if env_flag_enabled("ANTIGRAVITY_DISABLE_TRAY") {
        info!("Tray disabled by ANTIGRAVITY_DISABLE_TRAY");
        return false;
    }

    #[cfg(target_os = "linux")]
    {
        if is_wayland_session() && !env_flag_enabled("ANTIGRAVITY_FORCE_TRAY") {
            // 智能自适应检测：检查系统中是否存在有效的 AppIndicator / StatusNotifier 动态链接库
            let has_appindicator = [
                "/usr/lib/x86_64-linux-gnu/libayatana-appindicator3.so.1",
                "/usr/lib/x86_64-linux-gnu/libappindicator3.so.1",
                "/usr/lib/aarch64-linux-gnu/libayatana-appindicator3.so.1",
                "/usr/lib/aarch64-linux-gnu/libappindicator3.so.1",
                "/usr/lib64/libayatana-appindicator3.so.1",
                "/usr/lib64/libappindicator3.so.1",
                "/usr/lib/libayatana-appindicator3.so.1",
                "/usr/lib/libappindicator3.so.1",
            ]
            .iter()
            .any(|path| std::path::Path::new(path).exists());

            if has_appindicator {
                info!("Linux Wayland session detected with valid AppIndicator libraries. Enabling tray automatically.");
                return true;
            }

            warn!(
                "Linux Wayland session detected without AppIndicator libraries; disabling tray by default to avoid GTK crashes. Install libayatana-appindicator3 or set ANTIGRAVITY_FORCE_TRAY=1 to force-enable."
            );
            return false;
        }
    }

    true
}

#[cfg(target_os = "linux")]
fn nvidia_proprietary_loaded() -> bool {
    std::path::Path::new("/dev/nvidia0").exists()
        || std::path::Path::new("/proc/driver/nvidia/version").exists()
}

#[cfg(target_os = "linux")]
fn configure_linux_graphics() {
    use linux_graphics::{
        desktop_is_wlroots_family, should_disable_webkit_dmabuf, should_force_x11_backend,
    };

    let is_wayland = is_wayland_session();
    let has_x11_display = std::env::var("DISPLAY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_else(|_| std::env::var("XDG_SESSION_DESKTOP").unwrap_or_default());
    let force_wayland = env_flag_enabled("ANTIGRAVITY_FORCE_WAYLAND");
    let force_x11 = env_flag_enabled("ANTIGRAVITY_FORCE_X11");
    let gdk_already_set = std::env::var("GDK_BACKEND").is_ok();

    if should_force_x11_backend(
        gdk_already_set,
        force_x11,
        force_wayland,
        is_wayland,
        has_x11_display,
        &desktop,
    ) {
        // Force X11 backend under GNOME/KDE Wayland to avoid a GTK shm crash.
        std::env::set_var("GDK_BACKEND", "x11");
        warn!(
            "Forcing GDK_BACKEND=x11 for stability on Wayland. Set ANTIGRAVITY_FORCE_WAYLAND=1 to keep Wayland backend."
        );
    } else if is_wayland && !gdk_already_set && desktop_is_wlroots_family(&desktop) {
        info!(
            "Keeping native Wayland GDK backend on {} (Xwayland DISPLAY is not a reason to force X11).",
            desktop
        );
    }

    let webkit_already_set = std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_ok();
    if should_disable_webkit_dmabuf(
        webkit_already_set,
        is_wayland,
        nvidia_proprietary_loaded(),
        &desktop,
    ) {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        info!(
            "WEBKIT_DISABLE_DMABUF_RENDERER=1 (WebKit DMA-BUF workaround on this Wayland setup). Set it yourself to override."
        );
    }
}

/// Increase file descriptor limit for macOS to prevent "Too many open files" errors
#[cfg(target_os = "macos")]
fn increase_nofile_limit() {
    unsafe {
        let mut rl = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };

        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut rl) == 0 {
            info!(
                "Current open file limit: soft={}, hard={}",
                rl.rlim_cur, rl.rlim_max
            );

            // Attempt to increase to 4096 or maximum hard limit
            let target = 4096.min(rl.rlim_max);
            if rl.rlim_cur < target {
                rl.rlim_cur = target;
                if libc::setrlimit(libc::RLIMIT_NOFILE, &rl) == 0 {
                    info!("Successfully increased hard file limit to {}", target);
                } else {
                    warn!("Failed to increase file descriptor limit");
                }
            }
        }
    }
}

/// Windows FFI calls to disable Efficiency Mode (EcoQoS / Power Throttling)
/// to prevent background freezes when minimized/hidden.
#[cfg(target_os = "windows")]
mod windows_api {
    type Bool = i32;
    type Handle = *mut std::ffi::c_void;

    #[repr(C)]
    struct ProcessPowerThrottlingState {
        version: u32,
        control_mask: u32,
        state_mask: u32,
    }

    #[link(name = "Kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> Handle;
        fn SetProcessInformation(
            h_process: Handle,
            process_information_class: u32,
            process_information: *mut std::ffi::c_void,
            process_information_size: u32,
        ) -> Bool;
    }

    pub fn disable_efficiency_mode() {
        unsafe {
            let mut state = ProcessPowerThrottlingState {
                version: 1,        // PROCESS_POWER_THROTTLING_STATE::VERSION
                control_mask: 0x1, // PROCESS_POWER_THROTTLING_CURRENT_EXECUTION_SPEED
                state_mask: 0,
            };
            let process_handle = GetCurrentProcess();
            // ProcessPowerThrottling = 4
            let res = SetProcessInformation(
                process_handle,
                4,
                &mut state as *mut _ as *mut std::ffi::c_void,
                std::mem::size_of::<ProcessPowerThrottlingState>() as u32,
            );
            if res == 0 {
                let err = std::io::Error::last_os_error();
                tracing::warn!(
                    "Failed to disable Windows Power Throttling / EcoQoS: {}",
                    err
                );
            } else {
                tracing::info!(
                    "Successfully disabled Windows Power Throttling / EcoQoS for the process."
                );
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    modules::logger::init_logger();

    #[cfg(target_os = "linux")]
    configure_linux_graphics();

    // Increase the file descriptor limit on macOS.
    #[cfg(target_os = "macos")]
    increase_nofile_limit();

    // Disable Windows background throttling/EcoQoS
    #[cfg(target_os = "windows")]
    windows_api::disable_efficiency_mode();

    let tray_enabled = should_enable_tray();
    let context = application_context();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(modules::updater::UpdateRuntime::default())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .arg("--autostart")
                .app_name("agy-switch")
                .build(),
        )
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_denylist(&[modules::desktop::DASHBOARD_LABEL])
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::all()
                        .difference(tauri_plugin_window_state::StateFlags::VISIBLE),
                )
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !args.iter().any(|arg| arg == "--autostart") {
                let _ = modules::desktop::show_main(app);
            }
        }))
        .manage(modules::desktop::DesktopRuntime::default())
        .manage(modules::auto_switch::Runtime::default())
        .setup(move |app| {
            info!("Setup starting...");
            #[cfg(target_os = "macos")]
            modules::desktop::install_dashboard_shortcut(app.handle())?;
            modules::auto_switch::start(app.handle().clone());
            modules::quota_refresh::start(app.handle().clone());
            modules::account_sync::start(app.handle().clone());
            if tray_enabled {
                match modules::tray::create_tray(app.handle()) {
                    Ok(()) => {
                        modules::desktop::set_tray_available(app.handle(), true);
                        info!("Tray created");
                    }
                    Err(error) => warn!(
                        "Tray unavailable; preserving main window and Dock recovery: {}",
                        error
                    ),
                }
            }
            if let Err(error) = modules::desktop::initialize(app.handle()) {
                warn!("Desktop preferences could not be applied: {}", error);
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                }
            }
            Ok(())
        })
        .on_window_event(modules::desktop::handle_window_event)
        .invoke_handler(tauri::generate_handler![
            modules::auto_switch::get_auto_switch_config,
            modules::auto_switch::set_auto_switch_config,
            modules::auto_switch::get_auto_switch_status,
            modules::auto_switch::check_auto_switch_now,
            modules::auto_switch::cancel_auto_switch,
            modules::desktop::get_desktop_settings,
            modules::desktop::get_menu_bar_appearance,
            modules::desktop::set_desktop_preferences,
            modules::desktop::open_app_page,
            modules::desktop::hide_menu_bar_dashboard,
            modules::desktop::quit_app,
            commands::list_accounts,
            commands::get_account_dashboard_snapshot,
            commands::get_menu_bar_snapshot,
            commands::set_menu_bar_preferences,
            commands::set_dashboard_cards,
            commands::add_account,
            commands::delete_account,
            commands::delete_accounts,
            commands::reorder_accounts,
            commands::switch_account,
            commands::export_accounts,
            commands::get_current_account,
            commands::fetch_account_quota,
            commands::refresh_all_quotas,
            commands::load_config,
            commands::save_config,
            commands::prepare_oauth_url,
            commands::start_oauth_login,
            commands::complete_oauth_login,
            commands::cancel_oauth_login,
            commands::submit_oauth_code,
            commands::list_oauth_clients,
            commands::get_active_oauth_client,
            commands::set_active_oauth_client,
            commands::import_from_db,
            commands::import_custom_db,
            commands::sync_account_from_db,
            commands::open_data_folder,
            commands::get_data_dir_path,
            commands::show_main_window,
            modules::desktop::open_project_page,
            commands::set_window_theme,
            commands::update_account_label,
            commands::get_local_token_usage,
            commands::get_menu_bar_usage,
            commands::get_api_pricing,
            commands::check_for_updates,
            commands::download_and_install_update,
            commands::get_running_version,
        ])
        .build(context)
        .expect("error while building tauri application")
        .run(|_app_handle, _event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { has_visible_windows: false, .. } = _event {
                let _ = modules::desktop::show_main(_app_handle);
            }
        });
}

// WebView2 150+ ignores environment debug arguments on elevated hosts.
// Pass only the WebDriver port through its supported API in opt-in debug CI builds.
fn application_context() -> tauri::Context<tauri::Wry> {
    #[allow(unused_mut)]
    let mut context = tauri::generate_context!();
    #[cfg(all(target_os = "windows", debug_assertions, feature = "native-gui-test"))]
    if std::env::var("ANTIGRAVITY_NATIVE_GUI_TEST").as_deref() == Ok("1") {
        if let Some(port) = std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").ok()
            .and_then(|args| args.split_whitespace().find_map(|arg|
                arg.strip_prefix("--remote-debugging-port=").and_then(|port| port.parse::<u16>().ok()))) {
            for window in &mut context.config_mut().app.windows {
                window.additional_browser_args = Some(format!("--remote-debugging-port={port}"));
            }
        }
    }
    context
}
