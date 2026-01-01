use super::traits::{ChapterInfo, MangaPlugin, SearchResult};
use async_trait::async_trait;
use regex::Regex;
use scraper::{Html, Selector};
use serde::Deserialize;
use std::collections::HashSet;

pub struct AsuraScans;

#[derive(Deserialize)]
struct AsuraNode {
    name: serde_json::Value,
}

#[derive(Deserialize)]
struct AsuraPageObj {
    url: String,
}

#[async_trait]
impl MangaPlugin for AsuraScans {
    fn name(&self) -> &'static str {
        "Asura Scans"
    }

    fn is_valid_url(&self, url: &str) -> bool {
        url.contains("asuracomic.net")
    }

    async fn get_manga_title(
        &self,
        client: &reqwest::Client,
        url: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let html = client.get(url).send().await?.text().await?;
        let re = Regex::new(r"<title>(.*?) - Asura Scans</title>")?;

        Ok(re
            .captures(&html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "Unknown".into())
            .replace(' ', "_")
            .replace(':', ""))
    }

    async fn get_chapters(
        &self,
        client: &reqwest::Client,
        url: &str,
    ) -> Result<Vec<ChapterInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let html = client.get(url).send().await?.text().await?;
        let re = Regex::new(r#"[\\"]+chapters[\\"]+\s*:\s*(\[.*?\])"#)?;

        let json_str = re
            .captures(&html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().replace("\\\"", "\""))
            .ok_or("Could not find chapter list")?;

        let nodes: Vec<AsuraNode> = serde_json::from_str(&json_str)?;

        Ok(nodes
            .into_iter()
            .map(|n| {
                let num = n.name.to_string().replace('"', "");
                ChapterInfo {
                    number: num.clone(),
                    url: format!("{}/chapter/{}", url, num),
                }
            })
            .collect())
    }

    async fn get_pages(
        &self,
        client: &reqwest::Client,
        chapter_url: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let html = client.get(chapter_url).send().await?.text().await?;
        let re = Regex::new(r#"[\\"]+pages[\\"]+\s*:\s*(\[.*?\])"#)?;

        let json_str = re
            .captures(&html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().replace("\\\"", "\""))
            .ok_or("Could not find pages")?;

        if json_str.contains('{') {
            let objs: Vec<AsuraPageObj> = serde_json::from_str(&json_str)?;
            Ok(objs.into_iter().map(|p| p.url).collect())
        } else {
            Ok(serde_json::from_str(&json_str)?)
        }
    }

    async fn search(
        &self,
        client: &reqwest::Client,
        query: &str,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let clean_query = query.replace(['(', ')', '[', ']'], "").trim().to_string();
        let url = format!("https://asuracomic.net/series?page=1&name={}", clean_query);

        let html = client.get(&url).send().await?.text().await?;
        let document = Html::parse_document(&html);

        let selector = Selector::parse("a[href^='series/']").unwrap();
        let title_span_selector = Selector::parse("span.block.font-bold").unwrap();

        let mut results = Vec::new();

        for el in document.select(&selector) {
            let title = match el.select(&title_span_selector).next() {
                Some(span) => span.text().collect::<String>().trim().to_string(),
                None => el.text().collect::<String>().trim().to_string(),
            };

            let href = match el.value().attr("href") {
                Some(h) => h,
                None => continue,
            };

            if title.is_empty() {
                continue;
            }

            results.push(SearchResult {
                title,
                url: format!("https://asuracomic.net/{}", href),
                description: None,
            });
        }

        let q = clean_query.to_lowercase();
        results.retain(|r| r.title.to_lowercase().contains(&q));

        let mut seen = HashSet::new();
        results.retain(|r| seen.insert(r.url.clone()));

        Ok(results)
    }
}
