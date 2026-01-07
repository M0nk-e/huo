use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub concurrent_downloads: bool,
    pub max_concurrent_chapters: usize,
    pub auto_update_enabled: bool,
    pub auto_update_interval_days: u32,
    pub download_path: Option<String>,
    pub notification_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            concurrent_downloads: true,
            max_concurrent_chapters: 5,
            auto_update_enabled: false,
            auto_update_interval_days: 7,
            download_path: None,
            notification_enabled: true,
        }
    }
}

impl Settings {
    fn get_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("huo")
            .join("settings.toml")
    }

    pub fn load() -> Self {
        let path = Self::get_path();
        
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(settings) = toml::from_str(&content) {
                    return settings;
                }
            }
        }
        
        Self::default()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let path = Self::get_path();
        
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        
        Ok(())
    }

    pub fn toggle_concurrent(&mut self) {
        self.concurrent_downloads = !self.concurrent_downloads;
    }

    pub fn set_max_concurrent(&mut self, max: usize) {
        self.max_concurrent_chapters = max.clamp(1, 10);
    }

    pub fn toggle_auto_update(&mut self) {
        self.auto_update_enabled = !self.auto_update_enabled;
    }

    pub fn set_update_interval(&mut self, days: u32) {
        self.auto_update_interval_days = days.clamp(1, 30);
    }

    pub fn set_download_path(&mut self, path: Option<String>) {
        self.download_path = path;
    }

    pub fn get_download_path(&self) -> PathBuf {
        if let Some(ref custom_path) = self.download_path {
            PathBuf::from(custom_path)
        } else {
            dirs::document_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Mangas")
        }
    }
}