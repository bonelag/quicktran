use std::sync::mpsc::Sender;
use eframe::egui;
use muda::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub enum TrayAction {
    OpenTranslator,
    OpenSettings,
}

#[cfg(windows)]
pub fn show_and_focus_window() {
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
    };
    use windows::core::PCWSTR;

    let title: Vec<u16> = "QuickTran Native\0".encode_utf16().collect();
    unsafe {
        if let Ok(hwnd) = FindWindowW(None, PCWSTR(title.as_ptr())) {
            if !hwnd.0.is_null() {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
            }
        }
    }
}

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
