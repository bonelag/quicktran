use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub source_lang: String,
    pub target_lang: String,
    pub provider: String,
    pub auto_fallback: bool,
    pub hotkey_translate: String,
    pub hotkey_tooltip: String,
    pub auto_copy: bool,
    pub auto_overwrite: bool,
    pub close_to_tray: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            source_lang: "auto".to_string(),
            target_lang: "vi".to_string(),
            provider: "google_pa".to_string(),
            auto_fallback: true,
            hotkey_translate: "Alt+T".to_string(),
            hotkey_tooltip: "Alt+D".to_string(),
            auto_copy: false,
            auto_overwrite: false,
            close_to_tray: true,
        }
    }
}

impl AppConfig {
    /// Return the path to config.json placed in the exact same directory as the executable.
    pub fn config_path() -> PathBuf {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(dir) = exe_path.parent() {
                return dir.join("config.json");
            }
        }
        PathBuf::from("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();

        // 1. Try reading config.json in the same folder as the exe
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<AppConfig>(&content) {
                    return cfg;
                }
            }
        }

        // 2. Fallback check for data.json in exe directory
        if let Some(dir) = path.parent() {
            let data_path = dir.join("data.json");
            if data_path.exists() {
                if let Ok(content) = fs::read_to_string(&data_path) {
                    if let Ok(cfg) = serde_json::from_str::<AppConfig>(&content) {
                        return cfg;
                    }
                }
            }
        }

        // 3. Fallback to %APPDATA%/quicktran if migrated
        #[cfg(windows)]
        if let Ok(appdata) = std::env::var("APPDATA") {
            let p = PathBuf::from(appdata).join("quicktran").join("config_native.json");
            if p.exists() {
                if let Ok(content) = fs::read_to_string(&p) {
                    if let Ok(cfg) = serde_json::from_str::<AppConfig>(&content) {
                        let _ = cfg.save();
                        return cfg;
                    }
                }
            }
        }

        let default_cfg = Self::default();
        default_cfg.save();
        default_cfg
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, json);
        }
    }
}
