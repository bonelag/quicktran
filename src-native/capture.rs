use std::time::{Duration, Instant};

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
pub fn release_all_modifiers() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        keybd_event, KEYEVENTF_KEYUP, VK_CONTROL, VK_LCONTROL, VK_RCONTROL,
        VK_MENU, VK_LMENU, VK_RMENU, VK_SHIFT, VK_LSHIFT, VK_RSHIFT,
        VK_LWIN, VK_RWIN,
    };
    unsafe {
        keybd_event(VK_CONTROL.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_LCONTROL.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_RCONTROL.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_LMENU.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_RMENU.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_SHIFT.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_LSHIFT.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_RSHIFT.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_LWIN.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_RWIN.0 as u8, 0, KEYEVENTF_KEYUP, 0);
    }
}

#[cfg(not(windows))]
pub fn release_all_modifiers() {}

#[cfg(windows)]
pub fn send_ctrl(key: char) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        keybd_event, KEYEVENTF_KEYUP, VK_CONTROL,
    };

    release_all_modifiers();
    std::thread::sleep(Duration::from_millis(20));

    let vk = match key {
        'a' | 'A' => 0x41,
        'c' | 'C' => 0x43,
        'v' | 'V' => 0x56,
        _ => key.to_ascii_uppercase() as u8,
    };

    unsafe {
        // Press Ctrl
        keybd_event(VK_CONTROL.0 as u8, 0, Default::default(), 0);
        std::thread::sleep(Duration::from_millis(10));

        // Press and release key
        keybd_event(vk, 0, Default::default(), 0);
        std::thread::sleep(Duration::from_millis(15));
        keybd_event(vk, 0, KEYEVENTF_KEYUP, 0);
        std::thread::sleep(Duration::from_millis(10));

        // Release Ctrl
        keybd_event(VK_CONTROL.0 as u8, 0, KEYEVENTF_KEYUP, 0);
    }
}

#[cfg(not(windows))]
pub fn send_ctrl(_key: char) {}

/// Capture selected text from the foreground window
pub fn capture_selected_text() -> Option<String> {
    let prev_seq = get_clipboard_seq();

    // Settle keyboard state briefly (wait for user's hotkey press to settle)
    std::thread::sleep(Duration::from_millis(30));
    send_ctrl('c');

    let start = Instant::now();
    let mut changed = false;

    // Fast sequence check: wait up to 160ms for clipboard to update
    while start.elapsed() < Duration::from_millis(160) {
        if get_clipboard_seq() != prev_seq {
            changed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(8));
    }

    if !changed {
        std::thread::sleep(Duration::from_millis(30));
    }

    // Read clipboard
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
        std::thread::sleep(Duration::from_millis(30));
    }

    send_ctrl('v');
}
