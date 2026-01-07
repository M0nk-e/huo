// Change line 1 to this:
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions, Row};
use std::path::PathBuf;
use chrono::{DateTime, Utc};

pub struct Database {
    pub pool: SqlitePool,
}

#[derive(Debug, Clone)]
pub struct MangaRecord {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub source: String,
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
    pub last_check: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct ChapterRecord {
    pub id: i64,
    pub manga_id: i64,
    pub chapter_number: String,
    pub downloaded_at: DateTime<Utc>,
}

impl Database {
    fn get_db_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("huo")
            .join("manga.db")
    }

    pub async fn new() -> Result<Self, sqlx::Error> {
        let db_path = Self::get_db_path();
        
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&db_url)
            .await?;

        let db = Self { pool };
        db.init().await?;
        
        Ok(db)
    }

    async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS manga (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL UNIQUE,
                url TEXT NOT NULL,
                source TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_updated TEXT NOT NULL,
                last_check TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS chapters (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                manga_id INTEGER NOT NULL,
                chapter_number TEXT NOT NULL,
                downloaded_at TEXT NOT NULL,
                FOREIGN KEY (manga_id) REFERENCES manga(id) ON DELETE CASCADE,
                UNIQUE(manga_id, chapter_number)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_manga_title ON manga(title)"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_chapters_manga_id ON chapters(manga_id)"
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn add_manga(
        &self,
        title: &str,
        url: &str,
        source: &str,
    ) -> Result<i64, sqlx::Error> {
        let now = Utc::now();
        
        let result = sqlx::query(
            r#"
            INSERT INTO manga (title, url, source, created_at, last_updated)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(title) DO UPDATE SET
                url = excluded.url,
                last_updated = excluded.last_updated
            RETURNING id
            "#,
        )
        .bind(title)
        .bind(url)
        .bind(source)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .fetch_one(&self.pool)
        .await?;

        Ok(result.get(0))
    }

    pub async fn get_manga(&self, title: &str) -> Result<Option<MangaRecord>, sqlx::Error> {
        let record = sqlx::query_as::<_, (i64, String, String, String, String, String, Option<String>)>(
            "SELECT id, title, url, source, created_at, last_updated, last_check FROM manga WHERE title = ?"
        )
        .bind(title)
        .fetch_optional(&self.pool)
        .await?;

        Ok(record.map(|(id, title, url, source, created, updated, checked)| {
            MangaRecord {
                id,
                title,
                url,
                source,
                created_at: DateTime::parse_from_rfc3339(&created).unwrap().with_timezone(&Utc),
                last_updated: DateTime::parse_from_rfc3339(&updated).unwrap().with_timezone(&Utc),
                last_check: checked.and_then(|c| DateTime::parse_from_rfc3339(&c).ok().map(|d| d.with_timezone(&Utc))),
            }
        }))
    }

    pub async fn get_all_manga(&self) -> Result<Vec<MangaRecord>, sqlx::Error> {
        let records = sqlx::query_as::<_, (i64, String, String, String, String, String, Option<String>)>(
            "SELECT id, title, url, source, created_at, last_updated, last_check FROM manga ORDER BY last_updated DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(records.into_iter().map(|(id, title, url, source, created, updated, checked)| {
            MangaRecord {
                id,
                title,
                url,
                source,
                created_at: DateTime::parse_from_rfc3339(&created).unwrap().with_timezone(&Utc),
                last_updated: DateTime::parse_from_rfc3339(&updated).unwrap().with_timezone(&Utc),
                last_check: checked.and_then(|c| DateTime::parse_from_rfc3339(&c).ok().map(|d| d.with_timezone(&Utc))),
            }
        }).collect())
    }

    pub async fn mark_chapter_downloaded(
        &self,
        manga_id: i64,
        chapter_number: &str,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        
        sqlx::query(
            "INSERT OR IGNORE INTO chapters (manga_id, chapter_number, downloaded_at) VALUES (?, ?, ?)"
        )
        .bind(manga_id)
        .bind(chapter_number)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "UPDATE manga SET last_updated = ? WHERE id = ?"
        )
        .bind(now.to_rfc3339())
        .bind(manga_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_downloaded_chapters(&self, manga_id: i64) -> Result<Vec<String>, sqlx::Error> {
        let chapters = sqlx::query_as::<_, (String,)>(
            "SELECT chapter_number FROM chapters WHERE manga_id = ? ORDER BY chapter_number"
        )
        .bind(manga_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(chapters.into_iter().map(|(num,)| num).collect())
    }

    pub async fn update_last_check(&self, manga_id: i64) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        
        sqlx::query(
            "UPDATE manga SET last_check = ? WHERE id = ?"
        )
        .bind(now.to_rfc3339())
        .bind(manga_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_manga_needing_update(
        &self,
        interval_days: u32,
    ) -> Result<Vec<MangaRecord>, sqlx::Error> {
        let cutoff = Utc::now() - chrono::Duration::days(interval_days as i64);
        
        let records = sqlx::query_as::<_, (i64, String, String, String, String, String, Option<String>)>(
            "SELECT id, title, url, source, created_at, last_updated, last_check 
             FROM manga 
             WHERE last_check IS NULL OR last_check < ?
             ORDER BY last_check ASC"
        )
        .bind(cutoff.to_rfc3339())
        .fetch_all(&self.pool)
        .await?;

        Ok(records.into_iter().map(|(id, title, url, source, created, updated, checked)| {
            MangaRecord {
                id,
                title,
                url,
                source,
                created_at: DateTime::parse_from_rfc3339(&created).unwrap().with_timezone(&Utc),
                last_updated: DateTime::parse_from_rfc3339(&updated).unwrap().with_timezone(&Utc),
                last_check: checked.and_then(|c| DateTime::parse_from_rfc3339(&c).ok().map(|d| d.with_timezone(&Utc))),
            }
        }).collect())
    }

    pub async fn delete_manga(&self, title: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM manga WHERE title = ?")
            .bind(title)
            .execute(&self.pool)
            .await?;
        
        Ok(())
    }
}