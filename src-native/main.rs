#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod capture;
mod config;
mod hotkey;
mod translator;
mod tray;

use app::QuickTranApp;
use eframe::egui;

#[cfg(windows)]
fn check_single_instance() -> bool {
    extern "system" {
        fn CreateMutexW(lpMutexAttributes: *const core::ffi::c_void, bInitialOwner: i32, lpName: *const u16) -> isize;
        fn GetLastError() -> u32;
        fn FindWindowW(lpClassName: *const u16, lpWindowName: *const u16) -> isize;
        fn ShowWindow(hWnd: isize, nCmdShow: i32) -> i32;
        fn SetForegroundWindow(hWnd: isize) -> i32;
    }
    const ERROR_ALREADY_EXISTS: u32 = 183;
    const SW_RESTORE: i32 = 9;

    let mutex_name: Vec<u16> = "Global\\QuickTranNativeMutex\0".encode_utf16().collect();
    let win_name: Vec<u16> = "QuickTran Native\0".encode_utf16().collect();

    unsafe {
        let handle = CreateMutexW(core::ptr::null(), 1, mutex_name.as_ptr());
        if handle != 0 && GetLastError() == ERROR_ALREADY_EXISTS {
            let hwnd = FindWindowW(core::ptr::null(), win_name.as_ptr());
            if hwnd != 0 {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
            }
            return false;
        }
    }
    true
}

fn main() -> eframe::Result<()> {
    #[cfg(windows)]
    if !check_single_instance() {
        return Ok(());
    }

    let icon_data = match image::load_from_memory(include_bytes!("../src-tauri/icons/32x32.png")) {
        Ok(img) => {
            let rgba = img.into_rgba8();
            let (width, height) = rgba.dimensions();
            Some(egui::IconData {
                rgba: rgba.into_raw(),
                width,
                height,
            })
        }
        Err(_) => None,
    };

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("QuickTran Native")
        .with_inner_size([440.0, 390.0])
        .with_min_inner_size([380.0, 360.0]);

    if let Some(ic) = icon_data {
        viewport = viewport.with_icon(ic);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "QuickTran Native",
        options,
        Box::new(|cc| Ok(Box::new(QuickTranApp::new(cc)))),
    )
}
