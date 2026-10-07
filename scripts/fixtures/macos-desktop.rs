//! Native lifecycle acceptance using production desktop/config code and an
//! isolated fixture directory. No accounts, credentials or scheduler are loaded.
#![allow(dead_code)]

#[cfg(target_os = "macos")]
mod models {
    pub mod config { include!("../../src-tauri/src/models/config.rs"); }
    pub use config::AppConfig;
}
#[cfg(target_os = "macos")]
mod utils {
    pub mod fs { include!("../../src-tauri/src/utils/fs.rs"); }
}
#[cfg(target_os = "macos")]
mod modules {
    pub mod account {
        pub fn get_data_dir() -> Result<std::path::PathBuf, String> {
            let root = std::env::var_os("AGY_DESKTOP_FIXTURE_DATA")
                .map(std::path::PathBuf::from).ok_or("Missing isolated fixture")?;
            if !root.join(".desktop-fixture").is_file() { return Err("Not a fixture directory".into()); }
            Ok(root)
        }
    }
    pub mod config { include!("../../src-tauri/src/modules/config.rs"); }
    pub use config::{load_app_config, set_saved_desktop_preferences};
    pub mod i18n { pub fn default_language() -> String { "en".into() } }
    pub mod logger { pub fn log_warn(message: &str) { eprintln!("{message}"); } }
    pub mod native_menu {
        pub fn toggle(_: &tauri::AppHandle, _: Option<tauri::Rect>) -> Result<(), String> {
            Err("Menu interaction is outside this lifecycle fixture".into())
        }
    }
}
#[cfg(target_os = "macos")]
#[path = "../../src-tauri/src/modules/desktop.rs"]
mod desktop;

#[cfg(target_os = "macos")]
fn main() {
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    use objc2_foundation::MainThreadMarker;
    use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
    use tauri::Manager;

    fn inspect(app: &tauri::AppHandle, stage: &str, hidden: bool, visible: bool) -> bool {
        let marker = MainThreadMarker::new().expect("Native check requires the main thread");
        let policy = NSApplication::sharedApplication(marker).activationPolicy();
        let expected = if hidden { NSApplicationActivationPolicy::Accessory } else { NSApplicationActivationPolicy::Regular };
        let actual_visible = app.get_webview_window("main").unwrap().is_visible().unwrap();
        let passed = policy == expected && actual_visible == visible;
        println!("{}", serde_json::json!({"stage":stage,"accessory":policy == NSApplicationActivationPolicy::Accessory,"visible":actual_visible,"passed":passed}));
        passed
    }
    async fn on_main(app: &tauri::AppHandle, action: impl FnOnce(&tauri::AppHandle) -> bool + Send + 'static) -> bool {
        let (send, receive) = tokio::sync::oneshot::channel();
        let handle = app.clone();
        app.run_on_main_thread(move || { let _ = send.send(action(&handle)); }).unwrap();
        receive.await.unwrap()
    }
    async fn pause() { tokio::time::sleep(std::time::Duration::from_millis(200)).await; }

    let failed = Arc::new(AtomicBool::new(false));
    let closing = Arc::new(AtomicBool::new(false));
    let setup_failed = failed.clone();
    let exit_code = tauri::Builder::default()
        .plugin(tauri_plugin_autostart::Builder::new().app_name("agy-switch-desktop-fixture").build())
        .manage(desktop::DesktopRuntime::default())
        .on_window_event(desktop::handle_window_event)
        .setup(move |app| {
            tauri::tray::TrayIconBuilder::with_id("main")
                .tooltip("Switch lifecycle test")
                .icon(tauri::image::Image::new_owned(vec![0, 0, 0, 255], 1, 1))
                .build(app)?;
            desktop::set_tray_available(app.handle(), true);
            desktop::initialize(app.handle())?;
            setup_failed.store(!inspect(app.handle(), "startup", true, false), Ordering::Relaxed);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Cannot build native fixture")
        .run_return(move |app, event| {
            if let tauri::RunEvent::Exit = event {
                println!("{}", serde_json::json!({"stage":"close_without_tray","passed":closing.load(Ordering::Relaxed) && !failed.load(Ordering::Relaxed)}));
            }
            if let tauri::RunEvent::Ready = event {
                let app = app.clone(); let failed = failed.clone(); let closing = closing.clone();
                tauri::async_runtime::spawn(async move {
                    pause().await;
                    let mut passed = on_main(&app, |app| inspect(app, "after_ready", true, false)).await;
                    passed &= on_main(&app, |app| {
                        desktop::show_main(app).unwrap();
                        inspect(app, "reopen", true, true)
                    }).await;
                    on_main(&app, |app| { app.get_webview_window("main").unwrap().close().unwrap(); true }).await;
                    pause().await;
                    passed &= on_main(&app, |app| inspect(app, "close", true, false)).await;
                    // Reproduce the observed drift: a regular native policy with
                    // menu-only still saved. Closing/reopening must reconcile it.
                    on_main(&app, |app| {
                        desktop::show_main(app).unwrap();
                        NSApplication::sharedApplication(MainThreadMarker::new().unwrap())
                            .setActivationPolicy(NSApplicationActivationPolicy::Regular);
                        let status = serde_json::to_value(desktop::get_desktop_settings(app.clone()).unwrap()).unwrap();
                        let detected = status["dock_error"].is_string();
                        println!("{}", serde_json::json!({"stage":"drift_detected","passed":detected}));
                        if !detected { return false; }
                        app.get_webview_window("main").unwrap().close().unwrap(); true
                    }).await.then_some(()).expect("Settings must report the observed drift");
                    pause().await;
                    passed &= on_main(&app, |app| inspect(app, "close_after_drift", true, false)).await;
                    for hide in [false, true, false, true] {
                        let patch = serde_json::from_value(serde_json::json!({"hide_dock_icon":hide,"start_minimized":hide})).unwrap();
                        desktop::set_desktop_preferences(app.clone(), patch).await.unwrap();
                        passed &= on_main(&app, move |app| {
                            desktop::show_main(app).unwrap();
                            inspect(app, if hide { "enable_and_reopen" } else { "disable_and_reopen" }, hide, true)
                        }).await;
                    }
                    passed &= on_main(&app, |app| {
                        desktop::set_tray_available(app, false);
                        desktop::show_main(app).unwrap();
                        inspect(app, "tray_unavailable_recovery", false, true)
                    }).await;
                    failed.fetch_or(!passed, Ordering::Relaxed);
                    closing.store(true, Ordering::Relaxed);
                    on_main(&app, |app| { app.get_webview_window("main").unwrap().close().unwrap(); true }).await;
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    failed.store(true, Ordering::Relaxed);
                    app.exit(1); // A close without a tray must actually terminate.
                });
            }
        });
    std::process::exit(exit_code);
}

#[cfg(not(target_os = "macos"))]
fn main() { eprintln!("This native lifecycle fixture requires macOS"); std::process::exit(1); }
