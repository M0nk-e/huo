use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct MangaState {
    pub series_url: String,
    pub title: String,
    pub downloaded_chapters: HashSet<String>,
    pub last_updated: String,
}

impl MangaState {
    pub fn new(series_url: String, title: String) -> Self {
        Self {
            series_url,
            title,
            downloaded_chapters: HashSet::new(),
            last_updated: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn get_path(title: &str) -> PathBuf {
        dirs::document_dir()
            .unwrap_or(PathBuf::from("."))
            .join("Mangas")
            .join(title)
            .join(".manga_state.json")
    }

    pub fn load_or_new(title: &str, url: &str) -> Self {
        let path = Self::get_path(title);
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(state) = serde_json::from_str(&content) {
                    return state;
                }
            }
        }
        Self::new(url.to_string(), title.to_string())
    }

    pub fn save(&self, title: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let path = Self::get_path(title);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn mark_downloaded(&mut self, chapter: &str) {
        self.downloaded_chapters.insert(chapter.to_string());
        self.last_updated = chrono::Utc::now().to_rfc3339();
    }
}
