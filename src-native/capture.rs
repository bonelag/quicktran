use std::time::{Duration, Instant};
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

pub fn send_ctrl(key: char) {
    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        let k = match key {
            'a' | 'A' => Key::A,
            'c' | 'C' => Key::C,
            'v' | 'V' => Key::V,
            _ => Key::Unicode(key),
        };
        // Release active modifiers synthetically
        let _ = enigo.key(Key::Alt, Direction::Release);
        let _ = enigo.key(Key::Shift, Direction::Release);
        let _ = enigo.key(Key::Meta, Direction::Release);

        // Press Control + key
        let _ = enigo.key(Key::Control, Direction::Press);
        std::thread::sleep(Duration::from_millis(5));
        let _ = enigo.key(k, Direction::Click);
        std::thread::sleep(Duration::from_millis(5));
        let _ = enigo.key(Key::Control, Direction::Release);
    }
}

/// Capture selected text from the foreground window
pub fn capture_selected_text() -> Option<String> {
    let prev_seq = get_clipboard_seq();

    // Settle keyboard state briefly
    std::thread::sleep(Duration::from_millis(15));
    send_ctrl('c');

    let start = Instant::now();
    let mut changed = false;

    // Fast sequence check
    while start.elapsed() < Duration::from_millis(120) {
        if get_clipboard_seq() != prev_seq {
            changed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    if !changed {
        return None;
    }

    // Read clipboard only when sequence number actually changed
    if let Ok(mut clip) = arboard::Clipboard::new() {
        if let Ok(text) = clip.get_text() {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    None
}

/// Fast paste back to the active source window
pub fn paste_text(text: &str, source_hwnd: Option<isize>) {
    if let Ok(mut clip) = arboard::Clipboard::new() {
        let _ = clip.set_text(text.to_string());
    }

    if let Some(hwnd) = source_hwnd {
        restore_foreground(hwnd);
        std::thread::sleep(Duration::from_millis(20));
    }

    send_ctrl('v');
}
