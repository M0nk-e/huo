use crate::core::db::Database;
use crate::core::settings::Settings;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

// 1. Define a struct that matches your OLD .manga_state.json format exactly
#[derive(Deserialize)]
struct LegacyState {
    series_url: String,
    title: String,
    downloaded_chapters: HashSet<String>,
    // We parse this as a string to pass it directly to SQLite
    last_updated: String,
}

pub async fn run_migration(db: &Database) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let settings = Settings::load();
    let download_path = settings.get_download_path();

    if !download_path.exists() {
        return Ok(());
    }

    println!("🔄 Checking for legacy JSON files to migrate...");
    
    // 2. Walk through the Mangas folder
    let walker = WalkDir::new(&download_path).min_depth(2).max_depth(3);

    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        if entry.file_name() == ".manga_state.json" {
            let path = entry.path();
            // We ignore errors on individual files so one bad file doesn't stop the whole process
            if let Err(e) = migrate_file(db, path).await {
                eprintln!("⚠️ Failed to migrate {:?}: {}", path, e);
            }
        }
    }

    Ok(())
}

async fn migrate_file(db: &Database, path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 3. Read and Parse the JSON
    let content = fs::read_to_string(path)?;
    let state: LegacyState = serde_json::from_str(&content)?;

    // 4. Check if already exists in DB (Idempotency)
    if db.get_manga(&state.title).await?.is_some() {
        // Already migrated, skip it
        return Ok(());
    }

    println!("   Migrating: {}", state.title);

    // 5. Determine Source from URL (since JSON doesn't have it explicitly)
    let source = if state.series_url.contains("demonic") { "demonic" }
    else if state.series_url.contains("asura") { "asura" }
    else if state.series_url.contains("flame") { "flame" }
    else { "unknown" };

    // 6. Insert Manga Entry
    // We use the raw add_manga logic but we want to preserve the old 'last_updated'
    // So we insert, then immediately update the timestamp to match the JSON
    let manga_id = db.add_manga(&state.title, &state.series_url, source).await?;

    // Fix the timestamp to match the JSON (optional, but nice for history)
    sqlx::query("UPDATE manga SET last_updated = ? WHERE id = ?")
        .bind(&state.last_updated)
        .bind(manga_id)
        .execute(&db.pool) // You might need to make 'pool' pub in db.rs or add a helper
        .await?;

    // 7. Insert Chapters
    let mut count = 0;
    for chapter in state.downloaded_chapters {
        db.mark_chapter_downloaded(manga_id, &chapter).await?;
        count += 1;
    }

    println!("   ✓ Migrated {} chapters", count);
    
    // Optional: Rename the json file so we don't process it again (e.g., .json.bak)
    // fs::rename(path, path.with_extension("json.bak"))?;

    Ok(())
}