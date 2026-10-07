//! Hosted Windows/Linux lifecycle checks against the production desktop runtime.
//! No schedulers, account operations, credential stores or IPC commands are started.
#[cfg(not(target_os = "macos"))]
fn main() {
    use antigravity_tools_lib::desktop_lifecycle::{desktop, tray};
    use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
    use tauri::{Manager, WebviewWindow};

    fn record(stage: &str, passed: bool) -> bool {
        println!("{}", serde_json::json!({"stage":stage,"passed":passed})); passed
    }
    fn visible(app: &tauri::AppHandle, label: &str) -> bool {
        app.get_webview_window(label).is_some_and(|window| window.is_visible().unwrap_or(false))
    }
    fn same_window(first: &WebviewWindow, second: &WebviewWindow) -> bool {
        #[cfg(target_os = "windows")]
        { first.hwnd().unwrap() == second.hwnd().unwrap() }
        #[cfg(target_os = "linux")]
        { first.gtk_window().unwrap() == second.gtk_window().unwrap() }
    }
    fn panel_fits(app: &tauri::AppHandle) -> bool {
        let panel = app.get_webview_window(desktop::DASHBOARD_LABEL).unwrap();
        let position = panel.outer_position().unwrap();
        let size = panel.outer_size().unwrap();
        let content = panel.inner_size().unwrap();
        let monitor = panel.current_monitor().unwrap().unwrap();
        let area = monitor.work_area();
        let scale = monitor.scale_factor();
        let frame_width = f64::from(size.width.saturating_sub(content.width));
        let frame_height = f64::from(size.height.saturating_sub(content.height));
        let expected_width = (424.0 * scale).min((f64::from(area.size.width) - 16.0 * scale - frame_width).max(1.0)) as u32;
        let expected_height = (680.0 * scale).min((f64::from(area.size.height) - 16.0 * scale - frame_height).max(1.0)) as u32;
        eprintln!("Panel geometry: {}", serde_json::json!({"position":position,"outer":size,"content":content,
            "work_area":[area.position.x,area.position.y,area.size.width,area.size.height],"scale":scale,"expected_content":[expected_width,expected_height]}));
        // Windows shadows contribute to the outer rectangle. Check content
        // dimensions independently and still require the full frame to fit.
        content.width.abs_diff(expected_width) <= 1 && content.height.abs_diff(expected_height) <= 1
            && position.x >= area.position.x && position.y >= area.position.y
            && i64::from(position.x) + i64::from(size.width) <= i64::from(area.position.x) + i64::from(area.size.width) + 1
            && i64::from(position.y) + i64::from(size.height) <= i64::from(area.position.y) + i64::from(area.size.height) + 1
            && !panel.is_decorated().unwrap() && !panel.is_resizable().unwrap()
    }
    async fn on_main(app: &tauri::AppHandle, action: impl FnOnce(&tauri::AppHandle) -> bool + Send + 'static) -> bool {
        let (send, receive) = tokio::sync::oneshot::channel();
        let handle = app.clone();
        app.run_on_main_thread(move || { let _ = send.send(action(&handle)); }).unwrap();
        receive.await.unwrap()
    }
    async fn pause() { tokio::time::sleep(std::time::Duration::from_millis(350)).await; }
    async fn toggle(app: &tauri::AppHandle) {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || desktop::toggle_dashboard(&app, None)).await.unwrap().unwrap();
        pause().await;
    }

    assert_eq!(std::env::var("GITHUB_ACTIONS").as_deref(), Ok("true"), "Hosted fixture only");
    assert_eq!(std::env::var("RUNNER_ENVIRONMENT").as_deref(), Ok("github-hosted"), "Never run on user machines");
    let root = std::path::PathBuf::from(std::env::var_os("ABV_DATA_DIR").expect("Missing isolated fixture"));
    assert!(root.is_absolute() && root.join(".desktop-fixture").is_file());
    tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG)
        .with_writer(std::io::stderr).with_ansi(false).init();
    let no_tray = std::env::args().any(|arg| arg == "--no-tray");
    let failed = Arc::new(AtomicBool::new(false));
    let run_failed = failed.clone();
    let closing = Arc::new(AtomicBool::new(false));
    let close_timeout = Arc::new(AtomicBool::new(false));
    let mut context = tauri::generate_context!();
    // This example's identity is separate from the installed app's WebView data.
    context.config_mut().identifier = "com.agy-switch.desktop-lifecycle-fixture".into();
    eprintln!("Native startup: building the application");
    let application = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::Builder::new().app_name("agy-switch-desktop-fixture").build())
        .manage(desktop::DesktopRuntime::default())
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed | tauri::WindowEvent::Focused(_)) {
                eprintln!("Native window event: {} {event:?}", window.label());
            }
            desktop::handle_window_event(window, event);
        })
        .setup(move |app| {
            eprintln!("Native startup: setup entered");
            if !no_tray { tray::create_tray(app.handle())?; }
            eprintln!("Native startup: tray initialized");
            desktop::set_tray_available(app.handle(), !no_tray);
            desktop::initialize(app.handle())?;
            eprintln!("Native startup: desktop initialized");
            if !no_tray { desktop::warm_dashboard(app.handle()); }
            Ok(())
        })
        .build(context).expect("Cannot build native lifecycle fixture");
    eprintln!("Native startup: running the event loop");
    let code = application.run_return(move |app, event| match event {
            tauri::RunEvent::Ready => {
                eprintln!("Native startup: ready");
                let app = app.clone(); let failed = failed.clone(); let closing = closing.clone(); let close_timeout = close_timeout.clone();
                tauri::async_runtime::spawn(async move {
                    pause().await;
                    let mut passed = on_main(&app, move |app| record("startup", visible(app, "main") == no_tray)).await;
                    if !no_tray {
                        for _ in 0..40 {
                            if app.get_webview_window(desktop::DASHBOARD_LABEL).is_some() { break; }
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                        pause().await;
                        let first = app.get_webview_window(desktop::DASHBOARD_LABEL).expect("Prewarm must create the panel");
                        passed &= on_main(&app, |app| record("prewarm_hidden", !visible(app, desktop::DASHBOARD_LABEL) && app.tray_by_id("main").is_some())).await;
                        toggle(&app).await;
                        passed &= on_main(&app, |app| record("panel_open_and_bounds", visible(app, desktop::DASHBOARD_LABEL) && panel_fits(app))).await;
                        toggle(&app).await;
                        passed &= on_main(&app, |app| record("second_click_hides", !visible(app, desktop::DASHBOARD_LABEL))).await;
                        toggle(&app).await;
                        // Exercise repeated GDK monitor reads; the old worker
                        // path could lose its X connection on any open.
                        for _ in 0..4 { toggle(&app).await; toggle(&app).await; }
                        let reused = first.clone();
                        passed &= on_main(&app, move |app| record("panel_reused", visible(app, desktop::DASHBOARD_LABEL) && same_window(&reused, &app.get_webview_window(desktop::DASHBOARD_LABEL).unwrap()) && app.webview_windows().len() == 2)).await;
                        on_main(&app, |app| { app.get_webview_window(desktop::DASHBOARD_LABEL).unwrap().close().unwrap(); true }).await;
                        pause().await;
                        passed &= on_main(&app, |app| record("panel_close_hides", app.get_webview_window(desktop::DASHBOARD_LABEL).is_some() && !visible(app, desktop::DASHBOARD_LABEL))).await;
                        eprintln!("Native action: reopen panel before focus check");
                        toggle(&app).await;
                        // Focus the main window directly: show_main's explicit panel
                        // hide must not stand in for the delayed blur dismissal.
                        eprintln!("Native action: show and focus main");
                        on_main(&app, |app| { let main = app.get_webview_window("main").unwrap(); main.show().unwrap(); main.set_focus().unwrap(); true }).await;
                        pause().await;
                        passed &= on_main(&app, |app| record("blur_hides_panel", visible(app, "main") && !visible(app, desktop::DASHBOARD_LABEL))).await;
                        on_main(&app, |app| { app.get_webview_window("main").unwrap().close().unwrap(); true }).await;
                        pause().await;
                        passed &= on_main(&app, |app| record("main_close_keeps_runtime", app.get_webview_window("main").is_some() && !visible(app, "main"))).await;
                        toggle(&app).await;
                        on_main(&app, |app| { desktop::show_main(app).unwrap(); true }).await;
                        pause().await;
                        passed &= on_main(&app, |app| record("main_reopen_hides_panel", visible(app, "main") && !visible(app, desktop::DASHBOARD_LABEL))).await;
                    }
                    passed &= on_main(&app, |app| {
                        desktop::set_tray_available(app, false);
                        desktop::show_main(app).unwrap();
                        record("tray_unavailable_recovery", visible(app, "main"))
                    }).await;
                    let patch = serde_json::from_value(serde_json::json!({"start_minimized":true})).unwrap();
                    passed &= record("background_start_rejected_without_tray", desktop::set_desktop_preferences(app.clone(), patch).await.is_err());
                    failed.fetch_or(!passed, Ordering::Relaxed);
                    closing.store(true, Ordering::Relaxed);
                    on_main(&app, |app| { app.get_webview_window("main").unwrap().close().unwrap(); true }).await;
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    failed.store(true, Ordering::Relaxed);
                    close_timeout.store(true, Ordering::Relaxed);
                    // A retained hidden panel must not leave an inaccessible process.
                    app.exit(1);
                });
            }
            tauri::RunEvent::ExitRequested { code, .. } => { eprintln!("Native exit requested: {code:?}"); }
            tauri::RunEvent::Exit => { record("close_without_tray", closing.load(Ordering::Relaxed) && !close_timeout.load(Ordering::Relaxed)); }
            _ => {}
        });
    eprintln!("Native run returned: {code}");
    std::process::exit(if code == 0 && run_failed.load(Ordering::Relaxed) { 1 } else { code });
}

#[cfg(target_os = "macos")]
fn main() { eprintln!("Use test-macos-desktop.mjs for the AppKit lifecycle fixture"); std::process::exit(1); }
