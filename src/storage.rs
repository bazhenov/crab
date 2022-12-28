use crate::prelude::*;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use url::Url;

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

    pub async fn list_downloaded_pages(&self) -> Result<Vec<i64>> {
        let row: Vec<(i64,)> =
            sqlx::query_as("SELECT id FROM pages WHERE content IS NOT NULL AND content != 'Error'")
                .fetch_all(&self.0)
                .await?;
        let row = row.into_iter().map(|(id,)| id).collect::<Vec<_>>();
        Ok(row)
    }

    pub async fn register_seed_page(&self, url: &str) -> Result<i64> {
        let new_id = sqlx::query("INSERT INTO pages (url) VALUES (?)")
            .bind(url)
            .execute(&self.0)
            .await?
            .last_insert_rowid();
        Ok(new_id)
    }

    pub async fn register_page(&self, url: &str, depth: i32) -> Result<i64> {
        let new_id = sqlx::query("INSERT INTO pages (url, depth) VALUES (?, ?)")
            .bind(url)
            .bind(depth)
            .execute(&self.0)
            .await?
            .last_insert_rowid();
        Ok(new_id)
    }

    pub async fn read_fresh_pages(&self, count: u16) -> Result<Vec<Page>> {
        let result_set: Vec<(i64, String, i32)> =
            sqlx::query_as("SELECT id, url, depth FROM pages WHERE content IS NULL LIMIT ?")
                .bind(count)
                .fetch_all(&self.0)
                .await?;
        let mut pages = vec![];
        for (id, url, depth) in result_set {
            let url = Url::parse(&url)?;
            pages.push(Page { id, url, depth });
        }
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

    pub async fn read_page(&self, id: i64) -> Result<Option<Page>> {
        let content: Option<(i64, String, i32)> =
            sqlx::query_as("SELECT id, url, depth FROM pages WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.0)
                .await?;
        if let Some((id, url, depth)) = content {
            let url = Url::parse(&url)?;
            Ok(Some(Page { id, url, depth }))
        } else {
            Ok(None)
        }
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
    pub url: Url,
    pub depth: i32,
}
