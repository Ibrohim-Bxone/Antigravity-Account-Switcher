/**
 * Tauri Application Library Entrypoint.
 * Initializes background workers, local TCP server, system tray, and maps command handlers.
 * Main exports: run
 */
pub mod commands;
pub mod http;
pub mod limits;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;
use tauri_plugin_notification::NotificationExt;

use switcher_windows::SwitcherService;

#[cfg(target_os = "windows")]
extern "system" {
    fn GetUserDefaultUILanguage() -> u16;
}

#[cfg(target_os = "windows")]
fn get_system_lang() -> String {
    unsafe {
        let lang_id = GetUserDefaultUILanguage();
        let primary_lang = lang_id & 0x3ff;
        if primary_lang == 0x15 {
            // LANG_POLISH
            "pl".to_string()
        } else {
            "en".to_string()
        }
    }
}

fn log_trace_lib(step: &str) {
    if let Some(user_profile) = std::env::var_os("USERPROFILE") {
        let log_dir = std::path::Path::new(&user_profile)
            .join("AppData")
            .join("Local")
            .join("AntigravitySwitcher")
            .join("logs");
        let _ = std::fs::create_dir_all(&log_dir);
        let log_file = log_dir.join("startup_trace.log");
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_file) {
            let _ = writeln!(f, "[{}] [lib] {}", chrono::Local::now().to_rfc3339(), step);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    log_trace_lib("pub fn run() entered");
    #[cfg(target_os = "windows")]
    {
        log_trace_lib("Before check_single_instance");
        switcher_windows::check_single_instance();
        log_trace_lib("After check_single_instance");

        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        log_trace_lib("Checking WebView2");
        if let Err(e) = rt.block_on(switcher_windows::check_and_install_webview2()) {
            eprintln!("Failed to check or install WebView2: {}", e);
            log_trace_lib(&format!("WebView2 error: {}", e));
            std::process::exit(1);
        }
        log_trace_lib("WebView2 check passed");
    }

    log_trace_lib("Building Tauri application");
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            log_trace_lib("Inside tauri setup hook");
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            log_trace_lib("Initializing SwitcherService");
            let service = SwitcherService::initialize().map_err(|e| {
                eprintln!("Failed to initialize SwitcherService: {:?}", e);
                log_trace_lib(&format!("Failed to initialize SwitcherService: {:?}", e));
                e
            })?;
            log_trace_lib("SwitcherService initialized successfully");

            // Prefetch quotas in the background on startup
            let service_clone = service.clone();
            tauri::async_runtime::spawn(async move {
                let _ = service_clone.fetch_all_quotas_on_startup().await;
            });

            // Thread checking automatic switching (Smart Switch) in the background
            let service_smart = service.clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
                // Skip the first immediate tick
                interval.tick().await;
                loop {
                    interval.tick().await;
                    if let Err(e) = service_smart.check_and_perform_smart_switch().await {
                        service_smart.logger().error(
                            None,
                            "smart_switch",
                            format!("Smart Switch error: {}", e),
                        );
                    }
                }
            });



            app.manage(service.clone());

            http::start_http_server(service.clone(), app.handle().clone());

            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let icon = app.default_window_icon().cloned().unwrap_or_else(|| {
                tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png")).unwrap()
            });

            let _tray = TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        app.exit(0);
                    }
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_size(tauri::LogicalSize::new(800.0, 600.0));
                            let _ = window.center();
                            let _ = window.set_always_on_top(true);
                            let _ = window.set_focus();
                            let _ = window.set_always_on_top(false);
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_size(tauri::LogicalSize::new(800.0, 600.0));
                            let _ = window.center();
                            let _ = window.set_always_on_top(true);
                            let _ = window.set_focus();
                            let _ = window.set_always_on_top(false);
                        }
                    }
                })
                .build(app)?;

            if let Some(main_win) = app.get_webview_window("main") {
                let _ = main_win.show();
                let _ = main_win.unminimize();
                let _ = main_win.set_size(tauri::LogicalSize::new(800.0, 600.0));
                let _ = main_win.center();
                let _ = main_win.set_always_on_top(true);
                let _ = main_win.set_focus();
                let _ = main_win.set_always_on_top(false);
                #[cfg(target_os = "windows")]
                {
                    if let Ok(hwnd) = main_win.hwnd() {
                        switcher_windows::enable_window_resize(hwnd.0 as isize);
                        switcher_windows::force_window_foreground(hwnd.0 as isize);
                    }
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            log_trace_lib(&format!("Window event on {}: {:?}", window.label(), event));
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                log_trace_lib("WindowEvent::CloseRequested detected!");
                let service = window.state::<std::sync::Arc<SwitcherService>>();
                let minimize = service.minimize_to_tray();
                log_trace_lib(&format!("minimize_to_tray = {}", minimize));
                if minimize {
                    api.prevent_close();
                    let _ = window.hide();

                    #[cfg(target_os = "windows")]
                    {
                        let lang = get_system_lang();
                        let title = "Antigravity Account Switcher";
                        let body = if lang == "pl" {
                            "Aplikacja została zminimalizowana do zasobnika systemowego."
                        } else {
                            "The application has been minimized to the system tray."
                        };

                        let app = window.app_handle();
                        let _ = app.notification().builder().title(title).body(body).show();
                    }
                } else {
                    window.app_handle().exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_state,
            commands::request_switch,
            commands::confirm_switch,
            commands::cancel_switch,
            commands::add_current_profile,
            commands::delete_profile,
            commands::update_settings,
            commands::copy_diagnostics,
            commands::recovery_resume,
            commands::recovery_rollback,
            commands::start_oauth_login,
            commands::cancel_oauth_login,
            commands::show_mini_window,
            commands::hide_mini_window,
            commands::resize_mini_window,
            commands::toggle_maximize,
            commands::is_window_maximized,
            commands::minimize_window,
            commands::close_window,
            commands::enter_mini_mode,
            commands::exit_mini_mode,
            commands::wipe_app_data,
            commands::uninstall_app,
            commands::force_smart_switch,
            commands::lock_profile,
            commands::unlock_profile,
            commands::remove_profile_lock,
            commands::close_app_lock,
            commands::open_browser_url,
            commands::send_email_report,
            limits::get_external_ai_quotas
        ]);

    log_trace_lib("Calling builder.build(tauri::generate_context!())");
    let app = match builder.build(tauri::generate_context!()) {
        Ok(a) => {
            log_trace_lib("Tauri app build succeeded!");
            a
        }
        Err(e) => {
            log_trace_lib(&format!("Tauri app build FAILED with error: {:?}", e));
            eprintln!("Tauri build error: {:?}", e);
            return;
        }
    };

    log_trace_lib("Calling app.run()");
    app.run(|_app_handle, event| {
        match event {
            tauri::RunEvent::ExitRequested { code, .. } => {
                log_trace_lib(&format!("RunEvent::ExitRequested with code: {:?}", code));
            }
            tauri::RunEvent::Exit => {
                log_trace_lib("RunEvent::Exit");
            }
            _ => {}
        }
    });
    log_trace_lib("app.run() completed");
}
