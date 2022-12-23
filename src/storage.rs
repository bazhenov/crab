use crate::prelude::*;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

pub struct Storage(SqlitePool);

impl Storage {
    pub async fn new(url: &str) -> Result<Self> {
        let connection = SqlitePoolOptions::new().connect(url).await?;
        Ok(Self(connection))
    }

    pub async fn count_all_pages(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pages")
            .fetch_one(&self.0)
            .await?;
        Ok(row.0)
    }

    pub async fn register_seed_page(&self, url: &str) -> Result<i64> {
        let new_id = sqlx::query("INSERT INTO pages (url) VALUES (?1)")
            .bind(url)
            .execute(&self.0)
            .await?
            .last_insert_rowid();
        Ok(new_id)
    }

    pub async fn read_fresh_pages(&self, count: u16) -> Result<Vec<String>> {
        let pages: Vec<String> =
            sqlx::query_as("SELECT url FROM pages WHERE content IS NULL LIMIT ?")
                .bind(count)
                .fetch_all(&self.0)
                .await?
                .into_iter()
                .map(|r: (String,)| r.0)
                .collect::<Vec<_>>();
        Ok(pages)
    }
}
