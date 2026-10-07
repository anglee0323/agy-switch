use crate::modules;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Listener,
};

fn labels(language: &str) -> (&'static str, &'static str) {
    if language.starts_with("zh") {
        ("快速仪表盘", "设置…")
    } else {
        ("Quick Dashboard", "Settings…")
    }
}

fn build_menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let config = modules::load_app_config().unwrap_or_default();
    let texts = modules::i18n::get_tray_texts(&config.language);
    let (dashboard, settings) = labels(&config.language);
    let snapshot = modules::account_dashboard::snapshot().ok();
    let current = snapshot.as_ref().and_then(|snapshot| snapshot.accounts.iter()
        .find(|account| Some(&account.id) == snapshot.current_account_id.as_ref()));
    let user_text = format!(
        "{}: {}",
        texts.saved,
        current
            .as_ref()
            .map(|a| a.email.as_str())
            .unwrap_or(&texts.no_account)
    );
    let now = chrono::Utc::now().timestamp();
    let quota_lines = modules::menu_bar_projection::saved_quota_lines(current, now, config.refresh_interval, &config.language);
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(
        app,
        "dashboard",
        dashboard,
        true,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "info_user",
        user_text,
        false,
        None::<&str>,
    )?)?;
    for (index, text) in quota_lines.iter().enumerate() {
        menu.append(&MenuItem::with_id(
            app,
            format!("quota_{index}"),
            text,
            false,
            None::<&str>,
        )?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    let next = snapshot.as_ref().and_then(|snapshot| modules::menu_bar_projection::next_switchable_account(snapshot, now));
    menu.append(&MenuItem::with_id(
        app,
        "switch_next",
        texts.switch_next,
        next.is_some(),
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        "refresh_curr",
        texts.refresh_saved,
        current.is_some_and(|account| modules::menu_bar_projection::switchable_account(account, now)),
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        "show",
        texts.show_window,
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        "settings",
        settings,
        true,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "quit",
        texts.quit,
        true,
        None::<&str>,
    )?)?;
    Ok(menu)
}

pub fn create_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    let icon_bytes: &[u8] = include_bytes!("../../icons/tray-icon.png");
    #[cfg(not(target_os = "macos"))]
    let icon_bytes: &[u8] = include_bytes!("../../icons/icon.png");
    let img = image::load_from_memory(icon_bytes)
        .map_err(|e| tauri::Error::Io(std::io::Error::other(e.to_string())))?
        .to_rgba8();
    let icon = Image::new_owned(img.clone().into_raw(), img.width(), img.height());
    TrayIconBuilder::with_id("main")
        .menu(&build_menu(app)?)
        // Linux does not deliver TrayIconEvent::Click, so retain its native menu.
        .show_menu_on_left_click(cfg!(target_os = "linux"))
        .tooltip("AntiGravity Switch")
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| match event.id().as_ref() {
            "dashboard" => open_dashboard(app, None),
            "show" => {
                let _ = modules::desktop::show_main(app);
            }
            "settings" => {
                let _ = modules::desktop::open_app_page(app.clone(), "settings".into());
            }
            "quit" => app.exit(0),
            "switch_next" => {
                use std::sync::atomic::{AtomicBool, Ordering};
                static BUSY: AtomicBool = AtomicBool::new(false);
                if BUSY
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
                    .is_err()
                {
                    return;
                }
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    struct Release;
                    impl Drop for Release {
                        fn drop(&mut self) {
                            BUSY.store(false, Ordering::Release);
                        }
                    }
                    let _release = Release;
                    let Ok(snapshot) = modules::account_dashboard::snapshot() else { return; };
                    let Some(next) = modules::menu_bar_projection::next_switchable_account(&snapshot, chrono::Utc::now().timestamp()) else { return; };
                    let next_id = next.id.clone();
                    match crate::commands::switch_account(
                        app.clone(),
                        next_id.clone(),
                        None,
                    )
                    .await
                    {
                        Ok(()) => {
                            let _ = app.emit("tray://account-switched", next_id);
                        }
                        Err(error) => {
                            let _ = app.emit("menubar://error", error);
                        }
                    }
                });
            }
            "refresh_curr" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let selected = modules::account_dashboard::snapshot().ok().and_then(|snapshot| snapshot.accounts.into_iter()
                        .find(|account| Some(&account.id) == snapshot.current_account_id.as_ref()
                            && modules::menu_bar_projection::switchable_account(account, chrono::Utc::now().timestamp())));
                    if let Some(account) = selected {
                        if let Err(error) =
                            crate::commands::fetch_account_quota(app.clone(), account.id).await
                        {
                            modules::logger::log_warn(&format!("Tray refresh failed: {error}"));
                            let _ = app.emit("menubar://error", error.to_string());
                        }
                        let _ = app.emit("accounts://refreshed", ());
                    }
                });
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                open_dashboard(tray.app_handle(), Some(rect));
            }
        })
        .build(app)?;
    modules::menu_bar_usage::warm();
    #[cfg(not(target_os = "macos"))]
    modules::desktop::warm_dashboard(app);
    let handle = app.clone();
    app.listen("config://updated", move |_| {
        update_tray_menus(&handle);
    });
    Ok(())
}

fn open_dashboard(app: &tauri::AppHandle, rect: Option<tauri::Rect>) {
    let app = app.clone();
    // WebView2 creation must not run synchronously in the Windows event loop.
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = modules::desktop::toggle_dashboard(&app, rect) {
            modules::logger::log_warn(&format!("Quick dashboard unavailable: {error}"));
            let _ = modules::desktop::show_main(&app);
        }
    });
}

pub fn update_tray_menus(app: &tauri::AppHandle) {
    let app = app.clone();
    let handle = app.clone();
    let _ = handle.run_on_main_thread(move || {
        if let (Some(tray), Ok(menu)) = (app.tray_by_id("main"), build_menu(&app)) {
            let _ = tray.set_menu(Some(menu));
        }
        // One event updates both windows after add/delete/refresh/switch actions.
        let _ = app.emit("menubar://data-updated", ());
    });
}
