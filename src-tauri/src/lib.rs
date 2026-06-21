// QuickTran — system-wide quick translate.
// Global hotkey copies the focused selection, translates it, shows a preview
// window, and (on Overwrite) pastes the translation back into the source field.

use std::sync::Mutex;
use std::time::Duration;

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
};

#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};

/// Foreground window captured at hotkey time, so we can paste back into it.
struct SourceHwnd(Mutex<Option<isize>>);

/// Dynamic global hotkey registration tracking.
struct ActiveShortcut(Mutex<Option<Shortcut>>);
struct ActiveTooltipShortcut(Mutex<Option<Shortcut>>);

/// Send a Ctrl+<key> combo to the OS via synthetic input.
fn send_ctrl(key: char) {
    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        let k = match key {
            'a' | 'A' => Key::A,
            'c' | 'C' => Key::C,
            'v' | 'V' => Key::V,
            _ => Key::Unicode(key),
        };
        // Press Control first to intercept Alt-menu focus
        let _ = enigo.key(Key::Control, Direction::Press);
        // Release active modifiers synthetically
        let _ = enigo.key(Key::Alt, Direction::Release);
        let _ = enigo.key(Key::Shift, Direction::Release);
        let _ = enigo.key(Key::Meta, Direction::Release);

        // Press character key, hold for 50ms, then release
        let _ = enigo.key(k, Direction::Press);
        std::thread::sleep(Duration::from_millis(50));
        let _ = enigo.key(k, Direction::Release);

        // Release Control
        let _ = enigo.key(Key::Control, Direction::Release);
    }
}

#[cfg(windows)]
fn capture_foreground() -> Option<isize> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        None
    } else {
        Some(hwnd.0 as isize)
    }
}

#[cfg(windows)]
fn restore_foreground(val: isize) {
    unsafe {
        let _ = SetForegroundWindow(HWND(val as *mut core::ffi::c_void));
    }
}

#[cfg(not(windows))]
fn capture_foreground() -> Option<isize> {
    None
}
#[cfg(not(windows))]
fn restore_foreground(_val: isize) {}

/// Hotkey pressed: remember the source window, copy its selection, read the
/// clipboard, then pop the preview window with the captured text.
fn handle_capture(app: &AppHandle, is_tooltip: bool) {
    if let Some(hwnd) = capture_foreground() {
        *app.state::<SourceHwnd>().0.lock().unwrap() = Some(hwnd);
    }

    let old_text = app.clipboard().read_text().unwrap_or_default();

    // Sleep a short moment to let keyboard state settle
    std::thread::sleep(Duration::from_millis(100));

    if !is_tooltip {
        send_ctrl('a');
        std::thread::sleep(Duration::from_millis(150));
    }
    send_ctrl('c');

    // Poll clipboard for up to 300ms to see if it updated
    let mut text = old_text.clone();
    for _ in 0..6 {
        std::thread::sleep(Duration::from_millis(50));
        if let Ok(clip_text) = app.clipboard().read_text() {
            if clip_text != old_text {
                text = clip_text;
                break;
            }
        }
    }

    // If still matching old text, retry copy once after a short delay
    if text == old_text {
        std::thread::sleep(Duration::from_millis(100));
        send_ctrl('c');
        std::thread::sleep(Duration::from_millis(150));
        if let Ok(clip_text) = app.clipboard().read_text() {
            text = clip_text;
        }
    }

    if is_tooltip {
        if let Some(win) = app.get_webview_window("tooltip") {
            if let Some(monitor) = app.primary_monitor().unwrap_or(None) {
                let monitor_size = monitor.size();
                let scale_factor = monitor.scale_factor();
                
                let text_lines = text.lines().count();
                let win_w_logical = 360.0;
                let mut win_h_logical = 110.0;
                if text_lines > 1 {
                    win_h_logical = (110.0 + (text_lines as f64 * 22.0)).min(280.0);
                } else if text.len() > 100 {
                    win_h_logical = 150.0;
                }
                
                let win_w_physical = (win_w_logical * scale_factor) as u32;
                let win_h_physical = (win_h_logical * scale_factor) as u32;
                
                let offset_x_physical = (20.0 * scale_factor) as i32;
                let offset_y_physical = (60.0 * scale_factor) as i32;
                
                let x = monitor_size.width as i32 - win_w_physical as i32 - offset_x_physical;
                let y = monitor_size.height as i32 - win_h_physical as i32 - offset_y_physical;
                
                let _ = win.set_size(tauri::Size::Physical(tauri::PhysicalSize { width: win_w_physical, height: win_h_physical }));
                let _ = win.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
            }
            let _ = win.unminimize();
            let _ = win.show();
            let _ = app.emit("captured-tooltip", text);
        }
    } else {
        let auto_paste = is_auto_paste_enabled();
        if let Some(win) = app.get_webview_window("main") {
            if !auto_paste {
                let _ = win.unminimize();
                let _ = win.show();
                let _ = win.set_focus();
            }
            let _ = app.emit("captured", text);
        }
    }
}

/// Translate via Google's free gtx endpoint (no key). Source auto-detected.
fn translate_blocking(text: &str, target: &str) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(String::new());
    }
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post("https://translate.googleapis.com/translate_a/single")
        .query(&[
            ("client", "gtx"),
            ("sl", "auto"),
            ("tl", target),
            ("dt", "t"),
        ])
        .form(&[("q", text)])
        .send()
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
    let mut out = String::new();
    if let Some(arr) = json.get(0).and_then(|v| v.as_array()) {
        for seg in arr {
            if let Some(s) = seg.get(0).and_then(|v| v.as_str()) {
                out.push_str(s);
            }
        }
    }
    Ok(out)
}

#[tauri::command]
async fn translate(text: String, target: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || translate_blocking(&text, &target))
        .await
        .map_err(|e| e.to_string())?
}

/// Put the translation on the clipboard and paste it into the source field.
#[tauri::command]
fn overwrite(app: AppHandle, text: String) -> Result<(), String> {
    app.clipboard()
        .write_text(text)
        .map_err(|e| e.to_string())?;

    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }

    let target = *app.state::<SourceHwnd>().0.lock().unwrap();
    if let Some(val) = target {
        restore_foreground(val);
        std::thread::sleep(Duration::from_millis(120));
        send_ctrl('v');
    }
    Ok(())
}

/// Copy the translation to the clipboard without pasting.
#[tauri::command]
fn copy_text(app: AppHandle, text: String) -> Result<(), String> {
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

/// Hide the preview window (keeps the app alive in the tray).
#[tauri::command]
fn hide_window(window: tauri::Window) {
    let _ = window.hide();
}

#[tauri::command]
async fn request_post(
    url: String,
    headers: std::collections::HashMap<String, String>,
    body: serde_json::Value,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::new();
        let mut req = client.post(&url);
        for (k, v) in headers {
            req = req.header(k, v);
        }
        let resp = req.json(&body).send().map_err(|e| e.to_string())?;
        let status = resp.status();
        if !status.is_success() {
            let err_body = resp.text().unwrap_or_default();
            return Err(format!("HTTP {status}: {err_body}"));
        }
        let json: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
        Ok(json)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn request_get(
    url: String,
    headers: std::collections::HashMap<String, String>,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::new();
        let mut req = client.get(&url);
        for (k, v) in headers {
            req = req.header(k, v);
        }
        let resp = req.send().map_err(|e| e.to_string())?;
        let status = resp.status();
        if !status.is_success() {
            let err_body = resp.text().unwrap_or_default();
            return Err(format!("HTTP {status}: {err_body}"));
        }
        let json: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
        Ok(json)
    })
    .await
    .map_err(|e| e.to_string())?
}

use std::path::PathBuf;

fn get_config_path() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe_path.parent().ok_or("No parent directory")?;
    Ok(dir.join("data.json"))
}

fn is_auto_paste_enabled() -> bool {
    if let Ok(path) = get_config_path() {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(auto_paste) = json.get("settings").and_then(|s| s.get("autoPaste")).and_then(|v| v.as_bool()) {
                        return auto_paste;
                    }
                }
            }
        }
    }
    false
}

fn parse_shortcut(config: &serde_json::Value) -> Result<Shortcut, String> {
    let ctrl = config.get("ctrl").and_then(|v| v.as_bool()).unwrap_or(false);
    let alt = config.get("alt").and_then(|v| v.as_bool()).unwrap_or(true);
    let shift = config.get("shift").and_then(|v| v.as_bool()).unwrap_or(false);
    let win = config.get("win").and_then(|v| v.as_bool()).unwrap_or(false);
    let key_str = config.get("key").and_then(|v| v.as_str()).unwrap_or("KeyT");

    use std::str::FromStr;
    let code = Code::from_str(key_str).map_err(|_| format!("Invalid key code: {key_str}"))?;

    let mut mods = Modifiers::empty();
    if ctrl { mods |= Modifiers::CONTROL; }
    if alt { mods |= Modifiers::ALT; }
    if shift { mods |= Modifiers::SHIFT; }
    if win { mods |= Modifiers::SUPER; }

    Ok(Shortcut::new(Some(mods), code))
}

fn update_registered_shortcuts(app: &AppHandle, settings: &serde_json::Value) -> Result<(), String> {
    let gs = app.global_shortcut();

    // 1. Update main shortcut
    if let Some(main_config) = settings.get("settings").and_then(|s| s.get("shortcut")) {
        if let Ok(new_shortcut) = parse_shortcut(main_config) {
            let active_state = app.state::<ActiveShortcut>();
            let mut active = active_state.0.lock().unwrap();
            if let Some(old_shortcut) = active.take() {
                let _ = gs.unregister(old_shortcut);
            }
            let _ = gs.unregister(new_shortcut);
            let _ = gs.register(new_shortcut);
            *active = Some(new_shortcut);
        }
    }

    // 2. Update tooltip shortcut
    if let Some(tooltip_config) = settings.get("settings").and_then(|s| s.get("tooltipShortcut")) {
        if let Ok(new_shortcut) = parse_shortcut(tooltip_config) {
            let active_state = app.state::<ActiveTooltipShortcut>();
            let mut active = active_state.0.lock().unwrap();
            if let Some(old_shortcut) = active.take() {
                let _ = gs.unregister(old_shortcut);
            }
            let _ = gs.unregister(new_shortcut);
            let _ = gs.register(new_shortcut);
            *active = Some(new_shortcut);
        }
    }

    Ok(())
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: serde_json::Value) -> Result<(), String> {
    let path = get_config_path()?;
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())?;

    let _ = update_registered_shortcuts(&app, &settings);

    Ok(())
}

#[tauri::command]
fn load_settings_rust() -> Result<serde_json::Value, String> {
    let path = get_config_path()?;
    if !path.exists() {
        return Ok(serde_json::Value::Null);
    }
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(json)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.unminimize();
                let _ = win.show();
                let _ = win.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let app = app.clone();
                        let shortcut = shortcut.clone();
                        std::thread::spawn(move || {
                            let is_main = {
                                let active_state = app.state::<ActiveShortcut>();
                                let active = active_state.0.lock().unwrap();
                                active.as_ref().map(|s| s == &shortcut).unwrap_or(false)
                            };
                            let is_tooltip = {
                                let active_state = app.state::<ActiveTooltipShortcut>();
                                let active = active_state.0.lock().unwrap();
                                active.as_ref().map(|s| s == &shortcut).unwrap_or(false)
                            };

                            if is_main {
                                handle_capture(&app, false);
                            } else if is_tooltip {
                                handle_capture(&app, true);
                            }
                        });
                    }
                })
                .build(),
        )
        .manage(SourceHwnd(Mutex::new(None)))
        .manage(ActiveShortcut(Mutex::new(None)))
        .manage(ActiveTooltipShortcut(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            translate,
            overwrite,
            copy_text,
            hide_window,
            request_get,
            request_post,
            save_settings,
            load_settings_rust
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(150));
                
                let config = load_settings_rust().unwrap_or(serde_json::Value::Null);
                let _ = update_registered_shortcuts(&handle, &config);
            });

            // Tray icon with Show / Quit.
            let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("QuickTran")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.unminimize();
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window hides it instead of quitting (stays in tray).
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
