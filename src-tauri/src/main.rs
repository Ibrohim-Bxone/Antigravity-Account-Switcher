// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let backtrace = std::backtrace::Backtrace::capture();
        let timestamp = chrono::Local::now().to_rfc3339();
        let payload = match info.payload().downcast_ref::<&str>() {
            Some(s) => (*s).to_string(),
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => s.clone(),
                None => "Unknown panic payload".to_string(),
            },
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "Unknown location".to_string());

        let msg = format!(
            "[{timestamp}] CRITICAL PANIC DETECTED:\nLocation: {location}\nMessage: {payload}\n\nBacktrace:\n{backtrace:?}\n\n"
        );
        eprintln!("{msg}");

        if let Some(user_profile) = std::env::var_os("USERPROFILE") {
            let log_dir = std::path::Path::new(&user_profile)
                .join("AppData")
                .join("Local")
                .join("AntigravitySwitcher")
                .join("logs");
            let _ = std::fs::create_dir_all(&log_dir);
            let crash_file = log_dir.join("crash.log");
            use std::io::Write;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&crash_file)
            {
                let _ = writeln!(file, "{msg}");
            }
        }
    }));

    app_lib::run();
}

