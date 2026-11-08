use std::time::Duration;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn GetClipboardSequenceNumber() -> u32;
}

#[cfg(windows)]
pub fn get_clipboard_seq() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

#[cfg(not(windows))]
pub fn get_clipboard_seq() -> u32 {
    0
}

#[cfg(windows)]
pub fn capture_foreground() -> Option<isize> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        None
    } else {
        Some(hwnd.0 as isize)
    }
}

#[cfg(windows)]
pub fn restore_foreground(val: isize) {
    unsafe {
        let _ = SetForegroundWindow(HWND(val as *mut core::ffi::c_void));
    }
}

#[cfg(not(windows))]
pub fn capture_foreground() -> Option<isize> {
    None
}
#[cfg(not(windows))]
pub fn restore_foreground(_val: isize) {}

#[cfg(windows)]
pub fn get_mouse_pos() -> Option<(f32, f32)> {
    extern "system" {
        fn GetCursorPos(lpPoint: *mut POINT) -> i32;
    }
    #[repr(C)]
    struct POINT {
        x: i32,
        y: i32,
    }
    let mut pt = POINT { x: 0, y: 0 };
    unsafe {
        if GetCursorPos(&mut pt) != 0 {
            Some((pt.x as f32, pt.y as f32))
        } else {
            None
        }
    }
}

#[cfg(not(windows))]
pub fn get_mouse_pos() -> Option<(f32, f32)> {
    None
}

/// Wait until physical modifier keys are released so synthetic Ctrl keystrokes don't conflict
pub fn wait_for_hotkey_release() {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            GetAsyncKeyState, VK_CONTROL, VK_LCONTROL, VK_RCONTROL,
            VK_MENU, VK_LMENU, VK_RMENU, VK_SHIFT, VK_LSHIFT, VK_RSHIFT,
            VK_LWIN, VK_RWIN,
        };
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_millis(250) {
            unsafe {
                let ctrl = (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0
                    || (GetAsyncKeyState(VK_LCONTROL.0 as i32) as u16 & 0x8000) != 0
                    || (GetAsyncKeyState(VK_RCONTROL.0 as i32) as u16 & 0x8000) != 0;
                let alt = (GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0
                    || (GetAsyncKeyState(VK_LMENU.0 as i32) as u16 & 0x8000) != 0
                    || (GetAsyncKeyState(VK_RMENU.0 as i32) as u16 & 0x8000) != 0;
                let shift = (GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0
                    || (GetAsyncKeyState(VK_LSHIFT.0 as i32) as u16 & 0x8000) != 0
                    || (GetAsyncKeyState(VK_RSHIFT.0 as i32) as u16 & 0x8000) != 0;
                let win = (GetAsyncKeyState(VK_LWIN.0 as i32) as u16 & 0x8000) != 0
                    || (GetAsyncKeyState(VK_RWIN.0 as i32) as u16 & 0x8000) != 0;
                if !ctrl && !alt && !shift && !win {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

pub fn send_ctrl(key: char) {
    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        let k = match key {
            'a' | 'A' => Key::A,
            'c' | 'C' => Key::C,
            'v' | 'V' => Key::V,
            _ => Key::Unicode(key),
        };
        // Release active modifiers synthetically so hotkey modifiers don't conflict
        let _ = enigo.key(Key::Alt, Direction::Release);
        let _ = enigo.key(Key::Shift, Direction::Release);
        let _ = enigo.key(Key::Meta, Direction::Release);
        let _ = enigo.key(Key::Control, Direction::Release);

        // Press Control + key quickly (8ms)
        let _ = enigo.key(Key::Control, Direction::Press);
        std::thread::sleep(Duration::from_millis(8));
        let _ = enigo.key(k, Direction::Click);
        std::thread::sleep(Duration::from_millis(8));
        let _ = enigo.key(Key::Control, Direction::Release);
    }
}

/// Capture text from the active foreground window.
/// If `select_all` is true, sends Ctrl+A first to select all text.
/// If `select_all` is false, only sends Ctrl+C (for tooltip on selected text).
pub fn capture_text_from_window(select_all: bool) -> Option<String> {
    wait_for_hotkey_release();
    std::thread::sleep(Duration::from_millis(15));

    if select_all {
        send_ctrl('a');
        std::thread::sleep(Duration::from_millis(25));
    }

    let initial_seq = get_clipboard_seq();
    send_ctrl('c');

    let mut text = String::new();
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(10));
        if get_clipboard_seq() != initial_seq {
            if let Ok(mut clip) = arboard::Clipboard::new() {
                if let Ok(clip_text) = clip.get_text() {
                    text = clip_text;
                    break;
                }
            }
        }
    }

    let trimmed = text.trim();
    if !trimmed.is_empty() {
        Some(trimmed.to_string())
    } else {
        None
    }
}

pub fn capture_selected_text() -> Option<String> {
    capture_text_from_window(false)
}

pub fn capture_all_text() -> Option<String> {
    capture_text_from_window(true)
}

/// Fast paste back to the active source window
pub fn paste_text(text: &str, source_hwnd: Option<isize>) {
    if let Ok(mut clip) = arboard::Clipboard::new() {
        let _ = clip.set_text(text.to_string());
    }

    if let Some(hwnd) = source_hwnd {
        restore_foreground(hwnd);
        std::thread::sleep(Duration::from_millis(30));
    }

    send_ctrl('v');
}
