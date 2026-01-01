use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterInfo {
    pub number: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    #[allow(dead_code)]
    pub description: Option<String>,
}

#[async_trait]
pub trait MangaPlugin: Send + Sync {
    fn name(&self) -> &'static str;
    fn is_valid_url(&self, url: &str) -> bool;
    async fn get_manga_title(
        &self,
        client: &reqwest::Client,
        url: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>>;
    async fn get_chapters(
        &self,
        client: &reqwest::Client,
        url: &str,
    ) -> Result<Vec<ChapterInfo>, Box<dyn std::error::Error + Send + Sync>>;
    async fn get_pages(
        &self,
        client: &reqwest::Client,
        chapter_url: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>>;
    async fn search(
        &self,
        client: &reqwest::Client,
        query: &str,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>>;
}
