// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
fn spawn_delayed_cleanup(temp_wv: std::path::PathBuf) {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let appdata_dir = format!("{}\\com.quicktran.app", local);
        let temp_str = temp_wv.to_string_lossy().to_string();

        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let cmd = format!(
            "ping 127.0.0.1 -n 2 >nul & if exist \"{appdata_dir}\" rmdir /s /q \"{appdata_dir}\" & if exist \"{temp_str}\" rmdir /s /q \"{temp_str}\""
        );

        let _ = std::process::Command::new("cmd")
            .args(["/C", &cmd])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
}

fn main() {
    // 1. Clean up any leftover com.quicktran.app in LocalAppData immediately on start
    #[cfg(windows)]
    if let Ok(real_local) = std::env::var("LOCALAPPDATA") {
        let appdata_dir = std::path::PathBuf::from(real_local).join("com.quicktran.app");
        let _ = std::fs::remove_dir_all(&appdata_dir);
    }

    // 2. Clean up any previous temp folders in %TEMP%
    if let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) {
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                if name.starts_with("quicktran_wv_") {
                    let _ = std::fs::remove_dir_all(entry.path());
                }
            }
        }
    }

    // 3. Set WebView2 user data folder to an isolated temporary location
    let temp_wv = std::env::temp_dir().join(format!("quicktran_wv_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_wv);
    std::env::set_var("LOCALAPPDATA", &temp_wv);
    std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &temp_wv);

    // 4. Run application
    quicktran_lib::run();

    // 5. Clean up temporary directory and trigger detached cleaner for zero traces
    let _ = std::fs::remove_dir_all(&temp_wv);
    #[cfg(windows)]
    spawn_delayed_cleanup(temp_wv);
}
