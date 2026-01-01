use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::path::Path;

pub struct Downloader;

impl Downloader {
    fn chapter_to_padded(name: &str) -> String {
        let clean = name.trim().replace("\"", "");
        if let Ok(num) = clean.parse::<f64>() {
            if num.fract() == 0.0 {
                format!("{:03}", num as i32)
            } else {
                format!("{:06.1}", num).replace(".", "_")
            }
        } else {
            clean.replace(".", "_").replace(" ", "_")
        }
    }

    pub async fn save_chapter(
        client: &reqwest::Client,
        base_path: &Path,
        chapter_number: &str,
        pages: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let folder_name = Self::chapter_to_padded(chapter_number);
        let save_dir = base_path.join(&folder_name);
        fs::create_dir_all(&save_dir)?;

        let pb = ProgressBar::new(pages.len() as u64);

        // Use unwrap_or to provide fallback style if template fails
        let style = ProgressStyle::default_bar()
            .template("{msg} [{bar:40.cyan/blue}] {pos}/{len}")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("#>-");

        pb.set_style(style);
        pb.set_message(format!("DL {}", folder_name));

        for (idx, url) in pages.iter().enumerate() {
            if url.trim().is_empty() {
                pb.inc(1);
                continue;
            }

            match client.get(url).send().await {
                Ok(response) => match response.bytes().await {
                    Ok(bytes) => {
                        let ext = url
                            .split('?')
                            .next()
                            .and_then(|u| u.split('.').last())
                            .unwrap_or("jpg");
                        let filename = format!("{:03}.{}", idx + 1, ext);
                        fs::write(save_dir.join(filename), bytes)?;
                    }
                    Err(e) => {
                        eprintln!("Failed to download image {}: {}", idx + 1, e);
                    }
                },
                Err(e) => {
                    eprintln!("Failed to fetch image {}: {}", idx + 1, e);
                }
            }
            pb.inc(1);
        }

        pb.finish_with_message("Done");
        Ok(())
    }
}
