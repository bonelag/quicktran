use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone)]
pub enum HotkeyAction {
    Translate,
    Tooltip,
}


#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, MSG, WM_HOTKEY,
};

#[cfg(windows)]
fn parse_shortcut(sc: &str) -> (HOT_KEY_MODIFIERS, u32) {
    let mut mods = MOD_NOREPEAT;
    let mut vk = 0u32;

    for part in sc.split('+') {
        let p = part.trim().to_uppercase();
        match p.as_str() {
            "ALT" => mods |= MOD_ALT,
            "CTRL" | "CONTROL" => mods |= MOD_CONTROL,
            "SHIFT" => mods |= MOD_SHIFT,
            "WIN" | "SUPER" | "META" => mods |= MOD_WIN,
            s if s.len() == 1 => {
                let ch = s.chars().next().unwrap();
                if ch.is_ascii_alphanumeric() {
                    vk = ch as u32;
                }
            }
            s if s.starts_with('F') && s.len() <= 3 => {
                if let Ok(num) = s[1..].parse::<u32>() {
                    if (1..=12).contains(&num) {
                        vk = 0x6F + num; // F1 is 0x70
                    }
                }
            }
            "SPACE" => vk = 0x20,
            _ => {}
        }
    }

    (mods, vk)
}

pub struct HotkeyManager {
    running: Arc<AtomicBool>,
}

impl HotkeyManager {
    pub fn start(
        tx: Sender<HotkeyAction>,
        shortcut_translate: String,
        shortcut_tooltip: String,
    ) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        #[cfg(windows)]
        std::thread::spawn(move || {
            let (mods_trans, vk_trans) = parse_shortcut(&shortcut_translate);
            let (mods_tool, vk_tool) = parse_shortcut(&shortcut_tooltip);

            const ID_TRANSLATE: i32 = 1001;
            const ID_TOOLTIP: i32 = 1002;

            unsafe {
                let _ = RegisterHotKey(None, ID_TRANSLATE, mods_trans, vk_trans);
                let _ = RegisterHotKey(None, ID_TOOLTIP, mods_tool, vk_tool);
            }

            let mut msg = MSG::default();
            while running_clone.load(Ordering::Relaxed) {
                unsafe {
                    if GetMessageW(&mut msg, None, 0, 0).as_bool() {
                        if msg.message == WM_HOTKEY {
                            match msg.wParam.0 as i32 {
                                ID_TRANSLATE => {
                                    let _ = tx.send(HotkeyAction::Translate);
                                }
                                ID_TOOLTIP => {
                                    let _ = tx.send(HotkeyAction::Tooltip);
                                }
                                _ => {}
                            }
                        }
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            }

            unsafe {
                let _ = UnregisterHotKey(None, ID_TRANSLATE);
                let _ = UnregisterHotKey(None, ID_TOOLTIP);
            }
        });

        Self { running }
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}
