use std::sync::mpsc::Sender;
use eframe::egui;
use muda::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub enum TrayAction {
    OpenTranslator,
    OpenSettings,
}

#[cfg(windows)]
static LAST_WINDOW_POS: std::sync::Mutex<Option<(i32, i32, i32, i32)>> = std::sync::Mutex::new(None);
#[cfg(windows)]
static IS_TRAY_HIDDEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(windows)]
extern "system" {
    fn FindWindowW(lpClassName: *const u16, lpWindowName: *const u16) -> isize;
    fn GetWindowRect(hWnd: isize, lpRect: *mut RECT) -> i32;
    fn SetWindowPos(hWnd: isize, hWndInsertAfter: isize, X: i32, Y: i32, cx: i32, cy: i32, uFlags: u32) -> i32;
    fn GetWindowLongW(hWnd: isize, nIndex: i32) -> i32;
    fn SetWindowLongW(hWnd: isize, nIndex: i32, dwNewLong: i32) -> i32;
    fn ShowWindow(hWnd: isize, nCmdShow: i32) -> i32;
    fn SetForegroundWindow(hWnd: isize) -> i32;
    fn GetSystemMetrics(nIndex: i32) -> i32;
}

#[cfg(windows)]
pub fn hide_window_to_tray() {
    let title: Vec<u16> = "QuickTran Native\0".encode_utf16().collect();
    unsafe {
        let hwnd = FindWindowW(core::ptr::null(), title.as_ptr());
        if hwnd != 0 {
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect) != 0 && rect.left > -10000 {
                let w = rect.right - rect.left;
                let h = rect.bottom - rect.top;
                if w > 100 && h > 100 {
                    *LAST_WINDOW_POS.lock().unwrap() = Some((rect.left, rect.top, w, h));
                }
            }

            // Set WS_EX_TOOLWINDOW and clear WS_EX_APPWINDOW to remove from taskbar & Alt-Tab
            let ex_style = GetWindowLongW(hwnd, -20);
            let new_style = (ex_style | 0x00000080) & !0x00040000;
            SetWindowLongW(hwnd, -20, new_style);

            // Move off-screen without SW_HIDE so winit event loop and repaint requests remain active
            SetWindowPos(
                hwnd,
                0,
                -32000,
                -32000,
                0,
                0,
                0x0001 | 0x0004 | 0x0010 | 0x0020, // SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED
            );
            IS_TRAY_HIDDEN.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

#[cfg(windows)]
pub fn show_and_focus_window() {
    let title: Vec<u16> = "QuickTran Native\0".encode_utf16().collect();
    unsafe {
        let hwnd = FindWindowW(core::ptr::null(), title.as_ptr());
        if hwnd != 0 {
            let ex_style = GetWindowLongW(hwnd, -20);
            let new_style = (ex_style | 0x00040000) & !0x00000080; // +APPWINDOW, -TOOLWINDOW
            SetWindowLongW(hwnd, -20, new_style);

            if IS_TRAY_HIDDEN.load(std::sync::atomic::Ordering::SeqCst) {
                let saved = LAST_WINDOW_POS.lock().unwrap().clone();
                if let Some((x, y, w, h)) = saved {
                    SetWindowPos(hwnd, 0, x, y, w, h, 0x0004 | 0x0040 | 0x0020);
                } else {
                    let sw = GetSystemMetrics(0);
                    let sh = GetSystemMetrics(1);
                    let w = 440;
                    let h = 390;
                    let x = (sw - w) / 2;
                    let y = (sh - h) / 2;
                    SetWindowPos(hwnd, 0, x, y, w, h, 0x0004 | 0x0040 | 0x0020);
                }
                IS_TRAY_HIDDEN.store(false, std::sync::atomic::Ordering::SeqCst);
            }

            let _ = ShowWindow(hwnd, 5); // SW_SHOW
            let _ = ShowWindow(hwnd, 9); // SW_RESTORE
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

pub fn is_window_hidden_to_tray() -> bool {
    #[cfg(windows)]
    {
        IS_TRAY_HIDDEN.load(std::sync::atomic::Ordering::SeqCst)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(windows)]
pub fn make_tooltip_toolwindow() {
    let title: Vec<u16> = "QuickTran Tooltip\0".encode_utf16().collect();
    unsafe {
        let hwnd = FindWindowW(core::ptr::null(), title.as_ptr());
        if hwnd != 0 {
            let ex_style = GetWindowLongW(hwnd, -20);
            if (ex_style & 0x00000080) == 0 {
                let new_style = (ex_style | 0x00000080) & !0x00040000;
                SetWindowLongW(hwnd, -20, new_style);
                SetWindowPos(hwnd, 0, 0, 0, 0, 0, 0x0001 | 0x0002 | 0x0004 | 0x0020);
            }
        }
    }
}

#[cfg(not(windows))]
pub fn make_tooltip_toolwindow() {}

#[cfg(not(windows))]
pub fn hide_window_to_tray() {}

#[cfg(not(windows))]
pub fn show_and_focus_window() {}

pub struct SystemTray {
    _tray: TrayIcon,
}

impl SystemTray {
    pub fn new(tx: Sender<TrayAction>, ctx: egui::Context) -> Option<Self> {
        let menu = Menu::new();
        let open_item = MenuItem::new("Mở QuickTran", true, None);
        let settings_item = MenuItem::new("Cài đặt", true, None);
        let quit_item = MenuItem::new("Thoát", true, None);

        let open_id = open_item.id().clone();
        let settings_id = settings_item.id().clone();
        let quit_id = quit_item.id().clone();

        let _ = menu.append(&open_item);
        let _ = menu.append(&settings_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&quit_item);

        let icon_bytes = include_bytes!("../src-tauri/icons/32x32.png");
        let icon = match image::load_from_memory(icon_bytes) {
            Ok(img) => {
                let rgba = img.into_rgba8();
                let (width, height) = rgba.dimensions();
                Icon::from_rgba(rgba.into_raw(), width, height).ok()
            }
            Err(_) => None,
        };

        let mut builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("QuickTran — Native Quick Translate");

        if let Some(ic) = icon {
            builder = builder.with_icon(ic);
        }

        let tray = builder.build().ok()?;

        // Set event handler for menu items (runs immediately on click)
        let tx_menu = tx.clone();
        let ctx_menu = ctx.clone();
        muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
            if event.id == quit_id {
                std::process::exit(0);
            } else if event.id == open_id {
                show_and_focus_window();
                let _ = tx_menu.send(TrayAction::OpenTranslator);
                ctx_menu.request_repaint();
            } else if event.id == settings_id {
                show_and_focus_window();
                let _ = tx_menu.send(TrayAction::OpenSettings);
                ctx_menu.request_repaint();
            }
        }));

        // Set event handler for tray clicks (left click opens app)
        let tx_tray = tx;
        let ctx_tray = ctx;
        tray_icon::TrayIconEvent::set_event_handler(Some(move |event: tray_icon::TrayIconEvent| {
            if let tray_icon::TrayIconEvent::Click { button, button_state, .. } = event {
                if button == tray_icon::MouseButton::Left && button_state == tray_icon::MouseButtonState::Up {
                    show_and_focus_window();
                    let _ = tx_tray.send(TrayAction::OpenTranslator);
                    ctx_tray.request_repaint();
                }
            }
        }));

        Some(Self { _tray: tray })
    }
}
