use super::traits::{ChapterInfo, MangaPlugin, SearchResult};
use async_trait::async_trait;
use regex::Regex;
use scraper::{Html, Selector};
use serde_json::Value;
use std::collections::HashSet;

pub struct FlameComics;

#[async_trait]
impl MangaPlugin for FlameComics {
    fn name(&self) -> &'static str {
        "Flame Comics"
    }

    fn is_valid_url(&self, url: &str) -> bool {
        url.contains("flamecomics.xyz")
    }

    async fn get_manga_title(
        &self,
        client: &reqwest::Client,
        url: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let html = client.get(url).send().await?.text().await?;
        let re = Regex::new(r#"property="og:title"\s+content="([^"]+)""#)?;

        Ok(re
            .captures(&html)
            .and_then(|c| c.get(1))
            .map(|m| {
                m.as_str()
                    .split(" - ")
                    .next()
                    .unwrap_or(m.as_str())
                    .to_string()
            })
            .unwrap_or_else(|| "Unknown".into())
            .trim()
            .replace(' ', "_")
            .replace(':', ""))
    }

    async fn get_chapters(
        &self,
        client: &reqwest::Client,
        url: &str,
    ) -> Result<Vec<ChapterInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let html = client.get(url).send().await?.text().await?;
        let json_data = self.extract_next_data(&html)?;

        let chapters_val = self
            .find_key(&json_data, "chapters")
            .ok_or("Could not find chapters in page data")?;

        let chapters = chapters_val
            .as_array()
            .ok_or("chapters found but is not an array")?;

        let base_url = url.trim_end_matches('/');

        let mut results = Vec::new();

        for c in chapters {
            let token = c["token"].as_str().ok_or("token missing")?;
            let number = c["chapter"].as_str().ok_or("chapter missing")?;

            results.push(ChapterInfo {
                number: number.to_string(),
                url: format!("{}/{}", base_url, token),
            });
        }

        results.sort_by(|a, b| {
            let a_num: f64 = a.number.parse().unwrap_or(0.0);
            let b_num: f64 = b.number.parse().unwrap_or(0.0);
            a_num
                .partial_cmp(&b_num)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results)
    }

    async fn get_pages(
        &self,
        client: &reqwest::Client,
        chapter_url: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let html = client.get(chapter_url).send().await?.text().await?;
        let json_data = self.extract_next_data(&html)?;

        let chapter_val = self
            .find_key(&json_data, "chapter")
            .ok_or("Could not find chapter data")?;

        let series_id = chapter_val["series_id"]
            .as_i64()
            .ok_or("series_id not found")?;
        let token = chapter_val["token"].as_str().ok_or("token not found")?;

        let images_val = chapter_val.get("images").ok_or("images field not found")?;

        let images_obj = images_val.as_object().ok_or("images is not an object")?;

        let mut image_pairs: Vec<(usize, String)> = images_obj
            .iter()
            .filter_map(|(key, val)| {
                let idx = key.parse::<usize>().ok()?;
                let name = val["name"].as_str()?.to_string();
                Some((idx, name))
            })
            .collect();

        image_pairs.sort_by_key(|(idx, _)| *idx);

        let base_url = "https://cdn.flamecomics.xyz/uploads/images/series";
        let pages: Vec<String> = image_pairs
            .iter()
            .map(|(_, name)| {
                let clean_name = name.split('?').next().unwrap_or(name);
                format!("{}/{}/{}/{}", base_url, series_id, token, clean_name)
            })
            .collect();

        if pages.is_empty() {
            return Err("No images found in chapter data".into());
        }

        Ok(pages)
    }

    async fn search(
        &self,
        client: &reqwest::Client,
        query: &str,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let url = "https://flamecomics.xyz/browse";
        let html = client.get(url).send().await?.text().await?;
        let document = Html::parse_document(&html);

        let selector = Selector::parse("a[href^='/series/']").unwrap();
        let mut results = Vec::new();
        let mut seen = HashSet::new();
        let q = query.to_lowercase();

        for el in document.select(&selector) {
            let title = el.text().collect::<String>().trim().to_string();
            let href = match el.value().attr("href") {
                Some(h) => h,
                None => continue,
            };

            if title.is_empty() || title.len() < 2 {
                continue;
            }

            if title.to_lowercase().contains(&q) {
                let full_url = format!("https://flamecomics.xyz{}", href);
                if seen.insert(full_url.clone()) {
                    results.push(SearchResult {
                        title,
                        url: full_url,
                        description: None,
                    });
                }
            }
        }

        Ok(results)
    }
}

impl FlameComics {
    fn extract_next_data(
        &self,
        html: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let re =
            Regex::new(r#"<script id="__NEXT_DATA__" type="application/json">(.*?)</script>"#)?;
        let json_str = re
            .captures(html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .ok_or("Next.js data block missing")?;

        Ok(serde_json::from_str(json_str)?)
    }

    fn find_key<'a>(&self, value: &'a Value, target: &str) -> Option<&'a Value> {
        if let Some(obj) = value.as_object() {
            if obj.contains_key(target) {
                return Some(&obj[target]);
            }
            for val in obj.values() {
                if let Some(found) = self.find_key(val, target) {
                    return Some(found);
                }
            }
        } else if let Some(arr) = value.as_array() {
            for val in arr {
                if let Some(found) = self.find_key(val, target) {
                    return Some(found);
                }
            }
        }
        None
    }
}
