use clap::{Parser, Subcommand};
use dialoguer::{Confirm, Input, MultiSelect, Select};
use huo_core::*;
use std::time::Duration;
use tokio::time::sleep;
use crate::core::settings::Settings;
use crate::core::db::Database;
use crate::core::concurrent_downloader::ConcurrentDownloader;
use crate::core::auto_update::AutoUpdateService;
use crate::core::migration;

#[derive(Parser)]
#[command(name = "huo")]
#[command(author, version, about = "🔥 A fast and intuitive manga downloader v1.5.0", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Download manga chapters
    Download {
        #[arg(short, long)]
        source: Option<String>,
        #[arg(short, long)]
        url: Option<String>,
        #[arg(short, long)]
        chapters: Option<String>,
        #[arg(short, long)]
        all: bool,
        #[arg(short = 'y', long)]
        yes: bool,
        /// Enable multi-threaded concurrent downloads
        #[arg(long)]
        multi: bool,
    },

    /// Search for manga
    Search {
        #[arg(short, long)]
        source: Option<String>,
        query: Option<String>,
    },

    /// List available sources
    Sources,

    /// Manage settings
    Settings {
        #[command(subcommand)]
        action: Option<SettingsCommands>,
    },

    /// Check for updates across all manga
    CheckUpdates,

    /// Run auto-update service
    AutoUpdate {
        /// Run as daemon (background service)
        #[arg(long)]
        daemon: bool,
    },

    /// List all downloaded manga
    List,
}

#[derive(Subcommand)]
enum SettingsCommands {
    /// View current settings
    Show,
    /// Toggle concurrent downloads
    ToggleConcurrent,
    /// Set max concurrent chapters (1-10)
    SetConcurrent { max: usize },
    /// Toggle auto-update
    ToggleAutoUpdate,
    /// Set auto-update interval in days
    SetInterval { days: u32 },
    /// Set custom download path
    SetPath { path: Option<String> },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();

    // --- ADD THIS BLOCK ---
    println!("🔌 Initializing Database...");
    let db = Database::new().await?;
    
    // Run migration
    if let Err(e) = migration::run_migration(&db).await {
        eprintln!("Migration warning: {}", e);
    }

    if cli.command.is_none() {
        return interactive_mode().await;
    }

    match cli.command.unwrap() {
        Commands::Download { source, url, chapters, all, yes, multi } => {
            handle_download(source, url, chapters, all, yes, multi).await?;
        }
        Commands::Search { source, query } => {
            handle_search(source, query).await?;
        }
        Commands::Sources => {
            list_sources();
        }
        Commands::Settings { action } => {
            handle_settings(action).await?;
        }
        Commands::CheckUpdates => {
            check_updates().await?;
        }
        Commands::AutoUpdate { daemon } => {
            run_auto_update(daemon).await?;
        }
        Commands::List => {
            list_manga().await?;
        }
    }

    Ok(())
}

async fn interactive_mode() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = build_client()?;
    let plugins = get_plugins();
    let settings = Settings::load();

    loop {
        let main_menu = vec![
            "Download Manga",
            "Search Manga",
            "List Downloaded Manga",
            "Check for Updates",
            "Settings",
            "Exit",
        ];

        let selection = Select::new()
            .with_prompt("🔥 Huo - Main Menu")
            .items(&main_menu)
            .default(0)
            .interact()?;

        match selection {
            0 => {
                // Download
                if let Err(e) = download_flow(&client, &plugins, &settings).await {
                    eprintln!("Error: {}", e);
                    Input::<String>::new()
                        .with_prompt("Press Enter to continue")
                        .allow_empty(true)
                        .interact()?;
                }
            }
            1 => {
                // Search
                if let Err(e) = search_flow(&client, &plugins).await {
                    eprintln!("Error: {}", e);
                    Input::<String>::new()
                        .with_prompt("Press Enter to continue")
                        .allow_empty(true)
                        .interact()?;
                }
            }
            2 => {
                // List
                if let Err(e) = list_manga().await {
                    eprintln!("Error: {}", e);
                }
                Input::<String>::new()
                    .with_prompt("Press Enter to continue")
                    .allow_empty(true)
                    .interact()?;
            }
            3 => {
                // Check updates
                if let Err(e) = check_updates().await {
                    eprintln!("Error: {}", e);
                }
                Input::<String>::new()
                    .with_prompt("Press Enter to continue")
                    .allow_empty(true)
                    .interact()?;
            }
            4 => {
                // Settings
                settings_menu().await?;
            }
            5 => {
                // Exit
                println!("🔥 Goodbye!");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

async fn download_flow(
    client: &reqwest::Client,
    plugins: &[Box<dyn MangaPlugin>],
    settings: &Settings,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let plugin_names: Vec<&str> = plugins.iter().map(|p| p.name()).collect();
    let selection = Select::new()
        .with_prompt("Select Manga Source")
        .items(&plugin_names)
        .default(0)
        .interact()?;

    let plugin = &plugins[selection];

    let action_opts = vec!["Search for manga", "Paste URL directly", "← Back"];
    let action = Select::new()
        .with_prompt("How would you like to find the manga?")
        .items(&action_opts)
        .default(0)
        .interact()?;

    if action == 2 {
        return Ok(()); // Back
    }

    let url = if action == 0 {
        match tui::search::run_search_tui(client, plugin.as_ref()).await? {
            Some(url) => {
                println!("✓ Selected: {}", url);
                url
            }
            None => {
                println!("✗ Search cancelled.");
                return Ok(());
            }
        }
    } else {
        let mut url: String = Input::new()
            .with_prompt(format!("Paste the {} Series Link", plugin.name()))
            .interact_text()?;

        url = url.trim().to_string();

        if !plugin.is_valid_url(&url) {
            println!("⚠️  Warning: URL doesn't match expected format for {}.", plugin.name());
            if !Confirm::new().with_prompt("Continue anyway?").interact()? {
                return Ok(());
            }
        }

        url
    };

    download_manga(client, plugin.as_ref(), &url, settings).await
}

async fn search_flow(
    client: &reqwest::Client,
    plugins: &[Box<dyn MangaPlugin>],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let plugin_names: Vec<&str> = plugins.iter().map(|p| p.name()).collect();
    let mut items = plugin_names.clone();
    items.push("← Back");
    
    let selection = Select::new()
        .with_prompt("Select Manga Source")
        .items(&items)
        .default(0)
        .interact()?;

    if selection == items.len() - 1 {
        return Ok(()); // Back
    }

    let plugin = &plugins[selection];

    match tui::search::run_search_tui(client, plugin.as_ref()).await? {
        Some(url) => println!("✓ Selected: {}\n   Use Download menu to download this manga", url),
        None => println!("✗ Search cancelled"),
    }

    Ok(())
}

async fn settings_menu() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut settings = Settings::load();

    loop {
        let menu = vec![
            format!("Concurrent Downloads: {}", if settings.concurrent_downloads { "ON" } else { "OFF" }),
            format!("Max Concurrent Chapters: {}", settings.max_concurrent_chapters),
            format!("Auto-Update: {}", if settings.auto_update_enabled { "ON" } else { "OFF" }),
            format!("Update Interval: {} days", settings.auto_update_interval_days),
            format!("Download Path: {}", settings.get_download_path().display()),
            "Save & Back".to_string(),
        ];

        let selection = Select::new()
            .with_prompt("⚙️  Settings")
            .items(&menu)
            .default(0)
            .interact()?;

        match selection {
            0 => settings.toggle_concurrent(),
            1 => {
                let max: usize = Input::new()
                    .with_prompt("Max concurrent chapters (1-10)")
                    .default(settings.max_concurrent_chapters)
                    .interact()?;
                settings.set_max_concurrent(max);
            }
            2 => settings.toggle_auto_update(),
            3 => {
                let days: u32 = Input::new()
                    .with_prompt("Update interval in days (1-30)")
                    .default(settings.auto_update_interval_days)
                    .interact()?;
                settings.set_update_interval(days);
            }
            4 => {
                let path: String = Input::new()
                    .with_prompt("Download path (empty for default)")
                    .allow_empty(true)
                    .interact()?;
                settings.set_download_path(if path.is_empty() { None } else { Some(path) });
            }
            5 => {
                settings.save()?;
                println!("✓ Settings saved!");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

async fn download_manga(
    client: &reqwest::Client,
    plugin: &dyn MangaPlugin,
    url: &str,
    settings: &Settings,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("📚 Fetching series metadata...");
    let title = plugin.get_manga_title(client, url).await?;
    let all_chapters = plugin.get_chapters(client, url).await?;

    if all_chapters.is_empty() {
        println!("✗ No chapters found");
        return Ok(());
    }

    println!("✓ Found {} chapters for {}", all_chapters.len(), title);

    // Initialize database
    let db = Database::new().await?;
    let manga_id = db.add_manga(&title, url, plugin.name()).await?;
    let downloaded = db.get_downloaded_chapters(manga_id).await?;

    let new_chapters: Vec<_> = all_chapters
        .iter()
        .filter(|c| !downloaded.contains(&c.number))
        .cloned()
        .collect();

    if new_chapters.is_empty() {
        println!("✓ All chapters are up to date");
        return Ok(());
    }

    let action_opts = vec![
        format!("Download {} New Chapters", new_chapters.len()),
        "Select Specific Chapters".to_string(),
        "Re-download All".to_string(),
    ];

    let action = Select::new()
        .with_prompt("Action")
        .items(&action_opts)
        .default(0)
        .interact()?;

    let target_chapters = match action {
        0 => new_chapters,
        1 => {
            let labels: Vec<String> = new_chapters.iter().map(|c| format!("Chapter {}", c.number)).collect();
            let chosen = MultiSelect::new()
                .with_prompt("Select chapters")
                .items(&labels)
                .interact()?;
            chosen.into_iter().map(|i| new_chapters[i].clone()).collect()
        }
        2 => all_chapters,
        _ => return Ok(()),
    };

    let base_path = settings.get_download_path().join(&title);

    println!("\n🔥 Starting download...\n");

    if settings.concurrent_downloads && target_chapters.len() > 1 {
        // Concurrent download
        let mut chapters_with_pages = Vec::new();
        
        for chapter in &target_chapters {
            match plugin.get_pages(client, &chapter.url).await {
                Ok(pages) => {
                    if !pages.is_empty() {
                        chapters_with_pages.push((chapter.number.clone(), pages));
                    }
                }
                Err(e) => eprintln!("✗ Failed to fetch pages for chapter {}: {}", chapter.number, e),
            }
            sleep(Duration::from_millis(500)).await;
        }

        let successful = ConcurrentDownloader::save_chapters_concurrent(
            client,
            &base_path,
            chapters_with_pages,
            settings.max_concurrent_chapters,
        ).await?;

        for chapter_num in successful {
            db.mark_chapter_downloaded(manga_id, &chapter_num).await?;
        }
    } else {
        // Sequential download
        for (i, chapter) in target_chapters.iter().enumerate() {
            println!("[{}/{}] Chapter {}", i + 1, target_chapters.len(), chapter.number);

            match plugin.get_pages(client, &chapter.url).await {
                Ok(pages) => {
                    if pages.is_empty() {
                        eprintln!("⚠️  No images found for chapter {}", chapter.number);
                        continue;
                    }

                    if let Err(e) = ConcurrentDownloader::save_chapter_single(client, &base_path, &chapter.number, pages).await {
                        eprintln!("✗ Failed: {}", e);
                    } else {
                        db.mark_chapter_downloaded(manga_id, &chapter.number).await?;
                    }
                }
                Err(e) => eprintln!("✗ Failed to fetch pages: {}", e),
            }

            if i < target_chapters.len() - 1 {
                sleep(Duration::from_secs(1)).await;
            }
        }
    }

    println!("\n✓ Done! Saved to: {:?}", base_path);
    Ok(())
}

// Continued in next message...
// Helper functions continued...

async fn handle_download(
    source: Option<String>,
    url: Option<String>,
    chapters_arg: Option<String>,
    all: bool,
    yes: bool,
    multi: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = build_client()?;
    let plugins = get_plugins();
    let mut settings = Settings::load();

    if multi {
        settings.concurrent_downloads = true;
    }

    let plugin = if let Some(src) = source {
        plugins
            .iter()
            .find(|p| p.name().to_lowercase().contains(&src.to_lowercase()))
            .ok_or(format!("Source '{}' not found", src))?
    } else {
        let plugin_names: Vec<&str> = plugins.iter().map(|p| p.name()).collect();
        let selection = Select::new()
            .with_prompt("Select Manga Source")
            .items(&plugin_names)
            .interact()?;
        &plugins[selection]
    };

    let url = if let Some(u) = url {
        u
    } else {
        Input::new().with_prompt("Series URL").interact_text()?
    };

    download_manga(&client, plugin.as_ref(), &url, &settings).await
}

async fn handle_search(
    source: Option<String>,
    query: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = build_client()?;
    let plugins = get_plugins();

    let plugin = if let Some(src) = source {
        plugins
            .iter()
            .find(|p| p.name().to_lowercase().contains(&src.to_lowercase()))
            .ok_or(format!("Source '{}' not found", src))?
    } else {
        let plugin_names: Vec<&str> = plugins.iter().map(|p| p.name()).collect();
        let selection = Select::new()
            .with_prompt("Select Manga Source")
            .items(&plugin_names)
            .interact()?;
        &plugins[selection]
    };

    if let Some(q) = query {
        println!("🔍 Searching for '{}'...", q);
        let results = plugin.search(&client, &q).await?;

        if results.is_empty() {
            println!("✗ No results found");
            return Ok(());
        }

        for (i, result) in results.iter().enumerate() {
            println!("{}. {}", i + 1, result.title);
            println!("   {}", result.url);
        }
    } else {
        match tui::search::run_search_tui(&client, plugin.as_ref()).await? {
            Some(url) => println!("✓ Selected: {}", url),
            None => println!("✗ Search cancelled"),
        }
    }

    Ok(())
}

async fn handle_settings(
    action: Option<SettingsCommands>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut settings = Settings::load();

    match action {
        Some(SettingsCommands::Show) => {
            println!("🔥 Current Settings:");
            println!("  Concurrent Downloads: {}", if settings.concurrent_downloads { "ON" } else { "OFF" });
            println!("  Max Concurrent: {}", settings.max_concurrent_chapters);
            println!("  Auto-Update: {}", if settings.auto_update_enabled { "ON" } else { "OFF" });
            println!("  Update Interval: {} days", settings.auto_update_interval_days);
            println!("  Download Path: {}", settings.get_download_path().display());
            println!("  Notifications: {}", if settings.notification_enabled { "ON" } else { "OFF" });
        }
        Some(SettingsCommands::ToggleConcurrent) => {
            settings.toggle_concurrent();
            settings.save()?;
            println!("✓ Concurrent downloads: {}", if settings.concurrent_downloads { "ON" } else { "OFF" });
        }
        Some(SettingsCommands::SetConcurrent { max }) => {
            settings.set_max_concurrent(max);
            settings.save()?;
            println!("✓ Max concurrent chapters set to: {}", settings.max_concurrent_chapters);
        }
        Some(SettingsCommands::ToggleAutoUpdate) => {
            settings.toggle_auto_update();
            settings.save()?;
            println!("✓ Auto-update: {}", if settings.auto_update_enabled { "ON" } else { "OFF" });
        }
        Some(SettingsCommands::SetInterval { days }) => {
            settings.set_update_interval(days);
            settings.save()?;
            println!("✓ Update interval set to: {} days", settings.auto_update_interval_days);
        }
        Some(SettingsCommands::SetPath { path }) => {
            settings.set_download_path(path);
            settings.save()?;
            println!("✓ Download path set to: {}", settings.get_download_path().display());
        }
        None => {
            settings_menu().await?;
        }
    }

    Ok(())
}

async fn check_updates() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let settings = Settings::load();
    let plugins = get_plugins();

    println!("🔍 Checking for updates...");

    let service = AutoUpdateService::new(settings.clone(), plugins).await?;
    let reports = service.run_once().await?;

    if reports.is_empty() {
        println!("✓ All manga are up to date!");
    } else {
        println!("{}", service.format_report(&reports));
    }

    Ok(())
}

async fn run_auto_update(daemon: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let settings = Settings::load();

    if !settings.auto_update_enabled {
        println!("⚠️  Auto-update is disabled in settings.");
        println!("   Enable it with: huo settings toggle-auto-update");
        return Ok(());
    }

    let plugins = get_plugins();
    let service = AutoUpdateService::new(settings, plugins).await?;

    if daemon {
        println!("🔥 Starting auto-update daemon...");
        println!("   Press Ctrl+C to stop");
        service.run_daemon().await?;
    } else {
        println!("🔍 Running one-time update check...");
        let reports = service.run_once().await?;
        
        if reports.is_empty() {
            println!("✓ All manga are up to date!");
        } else {
            println!("{}", service.format_report(&reports));
        }
    }

    Ok(())
}

async fn list_manga() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let db = Database::new().await?;
    let manga_list = db.get_all_manga().await?;

    if manga_list.is_empty() {
        println!("No manga downloaded yet.");
        return Ok(());
    }

    println!("📚 Downloaded Manga:\n");
    println!("{:<30} {:<15} {:<20}", "Title", "Source", "Last Updated");
    println!("{}", "─".repeat(65));

    for manga in manga_list {
        let title = if manga.title.len() > 28 {
            format!("{}...", &manga.title[..28])
        } else {
            manga.title.clone()
        };

        let last_updated = manga.last_updated.format("%Y-%m-%d %H:%M").to_string();
        
        println!("{:<30} {:<15} {:<20}", title, manga.source, last_updated);
        
        let chapters = db.get_downloaded_chapters(manga.id).await?;
        println!("   {} chapters downloaded", chapters.len());
    }

    Ok(())
}

fn list_sources() {
    println!("🔥 Available sources:");
    println!("  • Asura Scans (asura)");
    println!("  • Demonic Scans (demonic)");
    println!("  • Flame Comics (flame)");
}

fn build_client() -> Result<reqwest::Client, Box<dyn std::error::Error + Send + Sync>> {
    Ok(reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
        .build()?)
}

fn get_plugins() -> Vec<Box<dyn MangaPlugin>> {
    vec![
        Box::new(AsuraScans),
        Box::new(DemonicScans),
        Box::new(FlameComics),
    ]
}