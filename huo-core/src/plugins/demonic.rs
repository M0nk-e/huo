use super::traits::{ChapterInfo, MangaPlugin, SearchResult};
use async_trait::async_trait;
use regex::Regex;
use scraper::{Html, Selector};
use std::collections::HashSet;

pub struct DemonicScans;

#[async_trait]
impl MangaPlugin for DemonicScans {
    fn name(&self) -> &'static str {
        "Demonic Scans"
    }

    fn is_valid_url(&self, url: &str) -> bool {
        url.contains("demonicscans.org")
    }

    async fn get_manga_title(
        &self,
        client: &reqwest::Client,
        url: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let html = client.get(url).send().await?.text().await?;
        let document = Html::parse_document(&html);

        if let Ok(selector) = Selector::parse("h1") {
            if let Some(element) = document.select(&selector).next() {
                let title = element.text().collect::<String>();
                return Ok(title
                    .trim()
                    .replace(" ", "_")
                    .replace(":", "")
                    .replace("/", "_"));
            }
        }

        let re = Regex::new(r"/title/([^/]+)")?;
        if let Some(cap) = re.captures(url) {
            return Ok(cap[1].replace("-", "_"));
        }

        Ok("Unknown_Manga".to_string())
    }

    async fn get_chapters(
        &self,
        client: &reqwest::Client,
        url: &str,
    ) -> Result<Vec<ChapterInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let html = client.get(url).send().await?.text().await?;
        let document = Html::parse_document(&html);

        let link_selector = Selector::parse("a.chplinks").unwrap();
        let mut chapters = Vec::new();
        let mut seen = HashSet::new();

        for element in document.select(&link_selector) {
            if let Some(href) = element.value().attr("href") {
                let re = Regex::new(r"chapter=([\d\.]+)")?;
                if let Some(cap) = re.captures(href) {
                    let chapter_num = cap[1].to_string();

                    if !seen.contains(&chapter_num) {
                        seen.insert(chapter_num.clone());

                        let full_url = if href.starts_with("http") {
                            href.to_string()
                        } else if href.starts_with("/") {
                            format!("https://demonicscans.org{}", href)
                        } else {
                            format!("https://demonicscans.org/{}", href)
                        };

                        chapters.push(ChapterInfo {
                            number: chapter_num,
                            url: full_url,
                        });
                    }
                }
            }
        }

        chapters.sort_by(|a, b| {
            let a_num = a.number.parse::<f64>().unwrap_or(0.0);
            let b_num = b.number.parse::<f64>().unwrap_or(0.0);
            a_num
                .partial_cmp(&b_num)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if chapters.is_empty() {
            return Err("No chapters found. The page structure may have changed.".into());
        }

        Ok(chapters)
    }

    async fn get_pages(
        &self,
        client: &reqwest::Client,
        url: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let html = client.get(url).send().await?.text().await?;
        let document = Html::parse_document(&html);

        let img_selector = Selector::parse("img.imgholder").unwrap();
        let mut pages = Vec::new();

        for element in document.select(&img_selector) {
            if let Some(src) = element.value().attr("src") {
                let url = src.trim();
                if !url.is_empty()
                    && (url.contains(".jpg") || url.contains(".png") || url.contains(".webp"))
                    && !pages.contains(&url.to_string())
                {
                    pages.push(url.to_string());
                }
            }
        }

        if pages.is_empty() {
            return Err("No images found for this chapter.".into());
        }

        Ok(pages)
    }

    async fn search(
        &self,
        client: &reqwest::Client,
        query: &str,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "https://demonicscans.org/search.php?manga={}",
            query.replace(" ", "%20")
        );

        let html = client.get(&url).send().await?.text().await?;
        let document = Html::parse_document(&html);

        let mut results = Vec::new();
        let mut seen_urls = HashSet::new();

        let entry_selector = Selector::parse("a[href^='/manga/']").unwrap();
        
        let title_div_selector = Selector::parse("div.seach-right div").unwrap();

        for element in document.select(&entry_selector) {
            if let Some(href) = element.value().attr("href") {
                let title = match element.select(&title_div_selector).next() {
                    Some(div) => div.text().collect::<String>().trim().to_string(),
                    None => {
                        element.text().collect::<String>().trim().to_string()
                    }
                };

                if title.is_empty() || title.to_lowercase().contains("novel :") {
                    continue;
                }

                let full_url = if href.starts_with("http") {
                    href.to_string()
                } else {
                    format!("https://demonicscans.org{}", if href.starts_with('/') { "" } else { "/" }) + href
                };

                if seen_urls.insert(full_url.clone()) {
                    results.push(SearchResult {
                        title,
                        url: full_url,
                        description: None,
                    });
                }
            }
        }

        let q = query.to_lowercase();
        results.retain(|r| r.title.to_lowercase().contains(&q));

        Ok(results)
    }

}
