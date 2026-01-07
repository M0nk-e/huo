use crate::core::db::Database;
use crate::core::settings::Settings;
use crate::plugins::traits::MangaPlugin;
use notify_rust::Notification;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

pub struct AutoUpdateService {
    db: Database,
    settings: Settings,
    plugins: HashMap<String, Box<dyn MangaPlugin>>,
}

pub struct UpdateReport {
    pub manga_title: String,
    pub new_chapters: Vec<String>,
}

impl AutoUpdateService {
    pub async fn new(
        settings: Settings,
        plugins: Vec<Box<dyn MangaPlugin>>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let db = Database::new().await?;
        
        let plugin_map: HashMap<String, Box<dyn MangaPlugin>> = plugins
            .into_iter()
            .map(|p| (p.name().to_lowercase(), p))
            .collect();

        Ok(Self {
            db,
            settings,
            plugins: plugin_map,
        })
    }

    pub async fn run_once(&self) -> Result<Vec<UpdateReport>, Box<dyn std::error::Error + Send + Sync>> {
        if !self.settings.auto_update_enabled {
            return Ok(Vec::new());
        }

        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
            .build()?;

        let manga_list = self.db.get_manga_needing_update(self.settings.auto_update_interval_days).await?;
        
        if manga_list.is_empty() {
            return Ok(Vec::new());
        }

        let mut reports = Vec::new();

        for manga in manga_list {
            // Find the appropriate plugin
            let plugin = match self.plugins.get(&manga.source.to_lowercase()) {
                Some(p) => p,
                None => {
                    eprintln!("No plugin found for source: {}", manga.source);
                    continue;
                }
            };

            // Check for new chapters
            match plugin.get_chapters(&client, &manga.url).await {
                Ok(available_chapters) => {
                    let downloaded = self.db.get_downloaded_chapters(manga.id).await?;
                    
                    let new_chapters: Vec<String> = available_chapters
                        .iter()
                        .filter(|ch| !downloaded.contains(&ch.number))
                        .map(|ch| ch.number.clone())
                        .collect();

                    if !new_chapters.is_empty() {
                        reports.push(UpdateReport {
                            manga_title: manga.title.clone(),
                            new_chapters: new_chapters.clone(),
                        });
                    }

                    // Update last check time
                    self.db.update_last_check(manga.id).await?;
                }
                Err(e) => {
                    eprintln!("Failed to check updates for {}: {}", manga.title, e);
                }
            }

            // Be nice to servers
            sleep(Duration::from_secs(2)).await;
        }

        // Send notification if there are updates
        if !reports.is_empty() && self.settings.notification_enabled {
            self.send_notification(&reports)?;
        }

        Ok(reports)
    }

    pub async fn run_daemon(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        loop {
            if let Err(e) = self.run_once().await {
                eprintln!("Auto-update check failed: {}", e);
            }

            // Sleep for the configured interval
            let sleep_duration = Duration::from_secs(
                self.settings.auto_update_interval_days as u64 * 24 * 3600
            );
            sleep(sleep_duration).await;
        }
    }

    fn send_notification(&self, reports: &[UpdateReport]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let total_chapters: usize = reports.iter().map(|r| r.new_chapters.len()).sum();
        let manga_count = reports.len();

        let summary = if manga_count == 1 {
            format!(
                "{} new chapter(s) for {}",
                reports[0].new_chapters.len(),
                reports[0].manga_title
            )
        } else {
            format!("{} new chapters for {} manga series", total_chapters, manga_count)
        };

        #[cfg(not(target_os = "linux"))]
        {
            Notification::new()
                .summary("🔥 Huo - New Manga Updates!")
                .body(&summary)
                .icon("manga")
                .timeout(5000)
                .show()?;
        }

        #[cfg(target_os = "linux")]
        {
            Notification::new()
                .summary("🔥 Huo - New Manga Updates!")
                .body(&summary)
                .icon("manga")
                .timeout(notify_rust::Timeout::Milliseconds(5000))
                .show()?;
        }

        Ok(())
    }

    pub fn format_report(&self, reports: &[UpdateReport]) -> String {
        let mut output = String::new();
        output.push_str("🔥 Auto-Update Report\n");
        output.push_str("═".repeat(50).as_str());
        output.push('\n');

        for report in reports {
            output.push_str(&format!("\n📚 {}\n", report.manga_title));
            output.push_str(&format!("   {} new chapter(s): ", report.new_chapters.len()));
            output.push_str(&report.new_chapters.join(", "));
            output.push('\n');
        }

        output.push('\n');
        output.push_str("═".repeat(50).as_str());
        output.push('\n');
        output.push_str(&format!("Total: {} manga, {} chapters\n", 
            reports.len(),
            reports.iter().map(|r| r.new_chapters.len()).sum::<usize>()
        ));

        output
    }
}