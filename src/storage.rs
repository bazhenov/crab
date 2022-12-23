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
        let new_id = sqlx::query("INSERT INTO pages (url) VALUES (?)")
            .bind(url)
            .execute(&self.0)
            .await?
            .last_insert_rowid();
        Ok(new_id)
    }

    pub async fn read_fresh_pages(&self, count: u16) -> Result<Vec<Page>> {
        let pages: Vec<Page> =
            sqlx::query_as("SELECT id, url FROM pages WHERE content IS NULL LIMIT ?")
                .bind(count)
                .fetch_all(&self.0)
                .await?
                .into_iter()
                .map(|r: (i64, String)| Page { id: r.0, url: r.1 })
                .collect::<Vec<_>>();
        Ok(pages)
    }

    pub async fn write_page_content(&self, page_id: i64, content: &str) -> Result<()> {
        sqlx::query("UPDATE pages SET content = ? WHERE id = ?")
            .bind(content)
            .bind(page_id)
            .execute(&self.0)
            .await?;
        Ok(())
    }

    pub async fn read_page_content(&self, id: i64) -> Result<Option<String>> {
        let content: Option<(String,)> = sqlx::query_as("SELECT content FROM pages WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.0)
            .await?;
        Ok(content.map(|r| r.0))
    }
}

#[derive(Debug, PartialEq)]
pub struct Page {
    pub id: i64,
    pub url: String,
}
