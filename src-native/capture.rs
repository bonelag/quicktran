use std::time::Duration;

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

#[cfg(windows)]
pub fn send_ctrl(key: char) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        keybd_event, KEYEVENTF_KEYUP, VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN,
    };

    unsafe {
        // Release active modifiers synthetically so hotkey modifiers don't conflict
        keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_SHIFT.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_LWIN.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_RWIN.0 as u8, 0, KEYEVENTF_KEYUP, 0);

        let vk = match key {
            'a' | 'A' => 0x41,
            'c' | 'C' => 0x43,
            'v' | 'V' => 0x56,
            _ => key.to_ascii_uppercase() as u8,
        };

        // Press Control + key quickly (5ms)
        keybd_event(VK_CONTROL.0 as u8, 0, Default::default(), 0);
        std::thread::sleep(Duration::from_millis(5));
        keybd_event(vk, 0, Default::default(), 0);
        std::thread::sleep(Duration::from_millis(5));
        keybd_event(vk, 0, KEYEVENTF_KEYUP, 0);
        std::thread::sleep(Duration::from_millis(5));
        keybd_event(VK_CONTROL.0 as u8, 0, KEYEVENTF_KEYUP, 0);
    }
}

#[cfg(not(windows))]
pub fn send_ctrl(_key: char) {}

/// Capture text from the active foreground window.
/// If `select_all` is true, sends Ctrl+A first to select all text (matches WebView behavior).
/// If `select_all` is false, only sends Ctrl+C (for tooltip on selected text).
pub fn capture_text_from_window(select_all: bool) -> Option<String> {
    std::thread::sleep(Duration::from_millis(15));

    if select_all {
        send_ctrl('a');
        std::thread::sleep(Duration::from_millis(15));
    }

    let initial_seq = get_clipboard_seq();
    send_ctrl('c');

    let mut text = String::new();
    for _ in 0..25 {
        std::thread::sleep(Duration::from_millis(5));
        if get_clipboard_seq() != initial_seq {
            if let Ok(mut clip) = arboard::Clipboard::new() {
                if let Ok(clip_text) = clip.get_text() {
                    text = clip_text;
                    break;
                }
            }
        }
    }

    // Fallback if sequence didn't change (e.g. app doesn't increment sequence number)
    if text.is_empty() {
        if let Ok(mut clip) = arboard::Clipboard::new() {
            if let Ok(clip_text) = clip.get_text() {
                text = clip_text;
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
        std::thread::sleep(Duration::from_millis(20));
    }

    send_ctrl('v');
}
