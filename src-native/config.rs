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
    /// Return the path to config.json strictly located in the same directory as the executable.
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

        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<AppConfig>(&content) {
                    return cfg;
                }
            }
        }

        Self::default()
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, json);
        }
    }
}
