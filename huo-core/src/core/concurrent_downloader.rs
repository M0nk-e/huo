use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::task::JoinSet;

pub struct ConcurrentDownloader;

impl ConcurrentDownloader {
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

    fn fire_style() -> ProgressStyle {
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40.red/yellow}] {pos}/{len} {elapsed}")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("█▓░") // Use standard block characters
    }

    pub async fn save_chapters_concurrent(
        client: &reqwest::Client,
        base_path: &Path,
        chapters: Vec<(String, Vec<String>)>, // (chapter_number, pages)
        max_concurrent: usize,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let multi = Arc::new(MultiProgress::new());
        let client = Arc::new(client.clone());
        
        // Create main progress bar
        let main_pb = multi.add(ProgressBar::new(chapters.len() as u64));
        main_pb.set_style(
            ProgressStyle::default_bar()
                .template("🔥 Overall [{bar:50.red/yellow}] {pos}/{len} chapters")
                .unwrap_or_else(|_| ProgressStyle::default_bar())
                .progress_chars("█▓░")
        );

        let mut tasks = JoinSet::new();
        let mut successful = Vec::new();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));

        for (chapter_number, pages) in chapters {
            let client = Arc::clone(&client);
            let multi = Arc::clone(&multi);
            let sem = Arc::clone(&semaphore);
            let base_path = base_path.to_path_buf();
            
            tasks.spawn(async move {
                let _permit = sem.acquire().await.ok()?;
                
                let folder_name = Self::chapter_to_padded(&chapter_number);
                let save_dir = base_path.join(&folder_name);
                
                if let Err(e) = fs::create_dir_all(&save_dir) {
                    eprintln!("Failed to create directory for chapter {}: {}", chapter_number, e);
                    return None;
                }

                let pb = multi.add(ProgressBar::new(pages.len() as u64));
                pb.set_style(Self::fire_style());
                pb.set_message(format!("Ch {}", folder_name));

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
                                
                                if let Err(e) = fs::write(save_dir.join(filename), bytes) {
                                    eprintln!("Failed to write image {}: {}", idx + 1, e);
                                }
                            }
                            Err(e) => eprintln!("Failed to download image {}: {}", idx + 1, e),
                        },
                        Err(e) => eprintln!("Failed to fetch image {}: {}", idx + 1, e),
                    }
                    pb.inc(1);
                }

                pb.finish_with_message(format!("✓ Ch {}", folder_name));
                Some(chapter_number)
            });
        }

        while let Some(result) = tasks.join_next().await {
            if let Ok(Some(chapter)) = result {
                successful.push(chapter);
                main_pb.inc(1);
            }
        }

        main_pb.finish_with_message("🔥 Download complete!");
        
        Ok(successful)
    }

    pub async fn save_chapter_single(
        client: &reqwest::Client,
        base_path: &Path,
        chapter_number: &str,
        pages: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let folder_name = Self::chapter_to_padded(chapter_number);
        let save_dir = base_path.join(&folder_name);
        fs::create_dir_all(&save_dir)?;

        let pb = ProgressBar::new(pages.len() as u64);
        pb.set_style(Self::fire_style());
        pb.set_message(format!("🔥 Ch {}", folder_name));

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
                    Err(e) => eprintln!("Failed to download image {}: {}", idx + 1, e),
                },
                Err(e) => eprintln!("Failed to fetch image {}: {}", idx + 1, e),
            }
            pb.inc(1);
        }

        pb.finish_with_message(format!("✓ Ch {}", folder_name));
        Ok(())
    }
}