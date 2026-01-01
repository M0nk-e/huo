use clap::{Parser, Subcommand};
use dialoguer::{Confirm, Input, MultiSelect, Select};
use huo_core::*;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Parser)]
#[command(name = "huo")]
#[command(author, version, about = "🔥 A fast and intuitive manga downloader", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Download manga chapters
    Download {
        /// Source: asura, demonic, flame
        #[arg(short, long)]
        source: Option<String>,

        /// Series URL
        #[arg(short, long)]
        url: Option<String>,

        /// Download specific chapters (e.g., "1,2,3" or "1-5")
        #[arg(short, long)]
        chapters: Option<String>,

        /// Download all chapters (including already downloaded)
        #[arg(short, long)]
        all: bool,

        /// Skip confirmation prompts
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Search for manga
    Search {
        /// Source: asura, demonic, flame
        #[arg(short, long)]
        source: Option<String>,

        /// Search query
        query: Option<String>,
    },

    /// List available sources
    Sources,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();

    // If no command provided, run interactive mode
    if cli.command.is_none() {
        return interactive_mode().await;
    }

    match cli.command.unwrap() {
        Commands::Download {
            source,
            url,
            chapters,
            all,
            yes,
        } => {
            handle_download(source, url, chapters, all, yes).await?;
        }
        Commands::Search { source, query } => {
            handle_search(source, query).await?;
        }
        Commands::Sources => {
            list_sources();
        }
    }

    Ok(())
}

async fn interactive_mode() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = build_client()?;
    let plugins = get_plugins();

    let plugin_names: Vec<&str> = plugins.iter().map(|p| p.name()).collect();
    let selection = Select::new()
        .with_prompt("Select Manga Source")
        .items(&plugin_names)
        .default(0)
        .interact()?;

    let plugin = &plugins[selection];

    let action_opts = vec!["Search for manga", "Paste URL directly"];
    let action = Select::new()
        .with_prompt("How would you like to find the manga?")
        .items(&action_opts)
        .default(0)
        .interact()?;

    let url = if action == 0 {
        match tui::search::run_search_tui(&client, plugin.as_ref()).await? {
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
            println!(
                "⚠️  Warning: URL doesn't match expected format for {}.",
                plugin.name()
            );
            if !Confirm::new().with_prompt("Continue anyway?").interact()? {
                return Ok(());
            }
        }

        url
    };

    download_manga(&client, plugin.as_ref(), &url).await
}

async fn handle_download(
    source: Option<String>,
    url: Option<String>,
    chapters_arg: Option<String>,
    all: bool,
    yes: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = build_client()?;
    let plugins = get_plugins();

    // Select source
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

    // Get URL
    let url = if let Some(u) = url {
        u
    } else {
        Input::new()
            .with_prompt("Series URL")
            .interact_text()?
    };

    // Pass the CLI arguments into the downloader logic
    download_manga_advanced(&client, plugin.as_ref(), &url, chapters_arg, all, yes).await
}

async fn download_manga_advanced(
    client: &reqwest::Client,
    plugin: &dyn MangaPlugin,
    url: &str,
    chapters_arg: Option<String>,
    all: bool,
    yes: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("📚 Fetching series metadata...");
    let title = plugin.get_manga_title(client, url).await?;
    let all_chapters = plugin.get_chapters(client, url).await?;

    if all_chapters.is_empty() {
        println!("✗ No chapters found");
        return Ok(());
    }

    let mut state = MangaState::load_or_new(&title, url);

    // Filter chapters based on whether --all was passed
    let mut target_chapters = if all {
        all_chapters.clone()
    } else {
        all_chapters
            .iter()
            .filter(|c| !state.downloaded_chapters.contains(&c.number))
            .cloned()
            .collect()
    };

    // If specific chapters were requested via --chapters "1,2,5"
    if let Some(arg) = chapters_arg {
        let wanted: Vec<&str> = arg.split(',').collect();
        target_chapters.retain(|c| wanted.contains(&c.number.as_str()));
    }

    if target_chapters.is_empty() {
        println!("✓ No new chapters to download.");
        return Ok(());
    }

    // Interactive confirmation (skipped if --yes is used)
    if !yes {
        println!("✓ Found {} chapters for {}", target_chapters.len(), title);
        let confirm = Confirm::new()
            .with_prompt(format!("Download {} chapters?", target_chapters.len()))
            .default(true)
            .interact()?;
        if !confirm { return Ok(()); }
    }

    let base_path = dirs::document_dir()
        .unwrap_or(std::path::PathBuf::from("."))
        .join("Mangas")
        .join(&title);

    for (i, chapter) in target_chapters.iter().enumerate() {
        println!("[{}/{}] Chapter {}", i + 1, target_chapters.len(), chapter.number);

        match plugin.get_pages(client, &chapter.url).await {
            Ok(pages) => {
                if let Err(e) = Downloader::save_chapter(client, &base_path, &chapter.number, pages).await {
                    eprintln!("✗ Failed: {}", e);
                } else {
                    state.mark_downloaded(&chapter.number);
                    let _ = state.save(&title);
                }
            }
            Err(e) => eprintln!("✗ Failed to fetch pages: {}", e),
        }
        
        if i < target_chapters.len() - 1 {
            sleep(Duration::from_secs(1)).await;
        }
    }

    Ok(())
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
        // Direct search
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
        // TUI search
        match tui::search::run_search_tui(&client, plugin.as_ref()).await? {
            Some(url) => println!("✓ Selected: {}", url),
            None => println!("✗ Search cancelled"),
        }
    }

    Ok(())
}

async fn download_manga(
    client: &reqwest::Client,
    plugin: &dyn MangaPlugin,
    url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("📚 Fetching series metadata...");
    let title = plugin.get_manga_title(client, url).await?;
    let all_chapters = plugin.get_chapters(client, url).await?;

    if all_chapters.is_empty() {
        println!("✗ No chapters found");
        return Ok(());
    }

    println!("✓ Found {} chapters for {}", all_chapters.len(), title);

    let mut state = MangaState::load_or_new(&title, url);

    let new_chapters: Vec<_> = all_chapters
        .iter()
        .filter(|c| !state.downloaded_chapters.contains(&c.number))
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
            let labels: Vec<String> = new_chapters
                .iter()
                .map(|c| format!("Chapter {}", c.number))
                .collect();
            let chosen = MultiSelect::new()
                .with_prompt("Select chapters")
                .items(&labels)
                .interact()?;
            chosen.into_iter().map(|i| new_chapters[i].clone()).collect()
        }
        2 => {
            if Confirm::new()
                .with_prompt("Clear download history?")
                .interact()?
            {
                state.downloaded_chapters.clear();
                all_chapters
            } else {
                return Ok(());
            }
        }
        _ => return Ok(()),
    };

    let base_path = dirs::document_dir()
        .unwrap_or(std::path::PathBuf::from("."))
        .join("Mangas")
        .join(&title);

    println!("\n🔥 Starting download...\n");

    for (i, chapter) in target_chapters.iter().enumerate() {
        println!("[{}/{}] Chapter {}", i + 1, target_chapters.len(), chapter.number);

        match plugin.get_pages(client, &chapter.url).await {
            Ok(pages) => {
                if pages.is_empty() {
                    eprintln!("⚠️  No images found for chapter {}", chapter.number);
                    continue;
                }

                if let Err(e) =
                    Downloader::save_chapter(client, &base_path, &chapter.number, pages).await
                {
                    eprintln!("✗ Failed: {}", e);
                } else {
                    state.mark_downloaded(&chapter.number);
                    let _ = state.save(&title);
                }
            }
            Err(e) => eprintln!("✗ Failed to fetch pages: {}", e),
        }

        if i < target_chapters.len() - 1 {
            sleep(Duration::from_secs(2)).await;
        }
    }

    println!("\n✓ Done! Saved to: {:?}", base_path);
    Ok(())
}

fn list_sources() {
    println!("Available sources:");
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