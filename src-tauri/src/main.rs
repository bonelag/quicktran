// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
fn run_cleaner(pid: u32, appdata_dir: String, temp_dir: String) {
    extern "system" {
        fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> isize;
        fn WaitForSingleObject(hHandle: isize, dwMilliseconds: u32) -> u32;
        fn CloseHandle(hObject: isize) -> i32;
    }
    const SYNCHRONIZE: u32 = 0x00100000;
    const INFINITE: u32 = 0xFFFFFFFF;

    unsafe {
        let handle = OpenProcess(SYNCHRONIZE, 0, pid);
        if handle != 0 {
            let _ = WaitForSingleObject(handle, INFINITE);
            let _ = CloseHandle(handle);
        }
    }

    // Main process exited. Allow Windows a brief moment to release file handles.
    std::thread::sleep(std::time::Duration::from_millis(600));

    // Retry cleanup to ensure complete wipe even if file handles were closing
    for _ in 0..10 {
        let _ = std::fs::remove_dir_all(&appdata_dir);
        let _ = std::fs::remove_dir_all(&temp_dir);
        let appdata_exists = std::path::Path::new(&appdata_dir).exists();
        let temp_exists = std::path::Path::new(&temp_dir).exists();
        if !appdata_exists && !temp_exists {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
}

#[cfg(windows)]
fn spawn_cleaner(real_local: &str, temp_wv: &std::path::Path) {
    if let Ok(exe) = std::env::current_exe() {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let pid = std::process::id().to_string();
        let appdata_dir = format!("{}\\com.quicktran.app", real_local);
        let temp_str = temp_wv.to_string_lossy().to_string();

        let _ = std::process::Command::new(exe)
            .args(["--clean-on-exit", &pid, &appdata_dir, &temp_str])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 5 && args[1] == "--clean-on-exit" {
        #[cfg(windows)]
        if let Ok(pid) = args[2].parse::<u32>() {
            run_cleaner(pid, args[3].clone(), args[4].clone());
        }
        return;
    }

    let real_local = std::env::var("LOCALAPPDATA").unwrap_or_default();

    // 1. Clean up any leftover com.quicktran.app in LocalAppData immediately on start
    #[cfg(windows)]
    if !real_local.is_empty() {
        let appdata_dir = std::path::PathBuf::from(&real_local).join("com.quicktran.app");
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

    // 4. Spawn watchdog cleaner to monitor this process and wipe everything on exit
    #[cfg(windows)]
    if !real_local.is_empty() {
        spawn_cleaner(&real_local, &temp_wv);
    }

    std::env::set_var("LOCALAPPDATA", &temp_wv);
    std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &temp_wv);

    // 5. Run application
    quicktran_lib::run();

    // 6. Clean up immediately upon normal return
    let _ = std::fs::remove_dir_all(&temp_wv);
    #[cfg(windows)]
    if !real_local.is_empty() {
        let appdata_dir = std::path::PathBuf::from(&real_local).join("com.quicktran.app");
        let _ = std::fs::remove_dir_all(&appdata_dir);
    }
}
