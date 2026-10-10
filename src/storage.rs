use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedDevice {
    pub ip: String,
    pub port: u16,
    pub nickname: String,
    pub last_connected: String,
    pub is_favorite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub custom_adb_path: Option<String>,
    pub custom_scrcpy_path: Option<String>,
    pub default_port: u16,
    pub auto_refresh: bool,
    pub refresh_interval_secs: u64,
    pub saved_devices: Vec<SavedDevice>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            custom_adb_path: None,
            custom_scrcpy_path: None,
            default_port: 5555,
            auto_refresh: true,
            refresh_interval_secs: 5,
            saved_devices: vec![],
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        if let Ok(app_data) = std::env::var("APPDATA") {
            let mut path = PathBuf::from(app_data);
            path.push("ADB-GUI");
            let _ = fs::create_dir_all(&path);
            path.push("config.json");
            path
        } else {
            PathBuf::from("adb_gui_config.json")
        }
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str(&content) {
                    return cfg;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn add_or_update_device(&mut self, ip: &str, port: u16, nickname: Option<String>) {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        if let Some(existing) = self.saved_devices.iter_mut().find(|d| d.ip == ip && d.port == port) {
            existing.last_connected = now;
            if let Some(nick) = nickname {
                if !nick.trim().is_empty() {
                    existing.nickname = nick;
                }
            }
        } else {
            self.saved_devices.push(SavedDevice {
                ip: ip.to_string(),
                port,
                nickname: nickname.unwrap_or_else(|| format!("{}:{}", ip, port)),
                last_connected: now,
                is_favorite: false,
            });
        }
        let _ = self.save();
    }

    pub fn remove_saved_device(&mut self, index: usize) {
        if index < self.saved_devices.len() {
            self.saved_devices.remove(index);
            let _ = self.save();
        }
    }
}
