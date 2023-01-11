use crate::prelude::*;
use futures::{stream::BoxStream, StreamExt};
use int_enum::IntEnum;
use sqlx::{
    sqlite::{SqlitePoolOptions, SqliteRow},
    Row, SqlitePool,
};
use std::fmt;
use url::Url;

pub struct Storage(SqlitePool);

#[repr(u8)]
#[derive(Debug, PartialEq, Clone, Copy, IntEnum, Eq, Hash)]
pub enum PageStatus {
    NotDownloaded = 1,
    Downloaded = 2,
    Failed = 3,
}

impl fmt::Display for PageStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PageStatus::NotDownloaded => write!(f, "NotDownloaded"),
            PageStatus::Downloaded => write!(f, "Downloaded"),
            PageStatus::Failed => write!(f, "Failed"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Page {
    pub id: i64,
    pub url: Url,
    pub depth: u16,
    pub status: PageStatus,
}

impl fmt::Display for Page {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Page {:3}   depth {:3}   {:10}     {}",
            self.id, self.depth, self.status, self.url
        )
    }
}

type PageRow = (i64, String, u16, u8);

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

    pub async fn list_pages(&self) -> Result<Vec<Page>> {
        let query = "SELECT id, url, depth, status FROM pages";
        let result_set: Vec<PageRow> = sqlx::query_as(query).fetch_all(&self.0).await?;
        let mut pages = vec![];
        for row in result_set {
            pages.push(page_from_tuple(row)?);
        }
        Ok(pages)
    }

    /// Registers new page
    ///
    /// If page with given rul already exists, [`Option::None`] is returned.
    pub async fn register_page<U: TryInto<Url>>(&self, url: U, depth: u16) -> Result<Option<i64>>
    where
        U::Error: Sync + Send + std::error::Error + 'static,
    {
        let new_id = sqlx::query("INSERT OR IGNORE INTO pages (url, depth) VALUES (?, ?)")
            .bind(url.try_into()?.to_string())
            .bind(depth)
            .execute(&self.0)
            .await?
            .last_insert_rowid();
        Ok(Some(new_id).filter(|id| *id > 0))
    }

    pub async fn list_not_downloaded_pages(&self, count: u16) -> Result<Vec<Page>> {
        let query =
            "SELECT id, url, depth, status FROM pages WHERE status = ? ORDER BY depth ASC LIMIT ?";
        let result_set: Vec<PageRow> = sqlx::query_as(query)
            .bind(PageStatus::NotDownloaded.int_value())
            .bind(count)
            .fetch_all(&self.0)
            .await?;
        let mut pages = vec![];
        for row in result_set {
            pages.push(page_from_tuple(row)?);
        }
        Ok(pages)
    }

    pub async fn reset_page(&self, page_id: i64) -> Result<()> {
        sqlx::query("UPDATE pages SET status = ? WHERE id = ?")
            .bind(PageStatus::NotDownloaded.int_value())
            .bind(page_id)
            .execute(&self.0)
            .await?;
        Ok(())
    }

    /// Writes page content in storage and marks page as [`PageStatus::Downloaded`]
    pub async fn write_page_content(&self, page_id: i64, content: &str) -> Result<()> {
        sqlx::query("UPDATE pages SET content = ?, status = ? WHERE id = ?")
            .bind(content)
            .bind(PageStatus::Downloaded.int_value())
            .bind(page_id)
            .execute(&self.0)
            .await?;
        Ok(())
    }

    pub async fn read_page(&self, id: i64) -> Result<Option<Page>> {
        let content: Option<PageRow> =
            sqlx::query_as("SELECT id, url, depth, status FROM pages WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.0)
                .await?;
        if let Some(content) = content {
            page_from_tuple(content).map(Some)
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

    /// Lists downloaded pages and its content
    pub fn read_downloaded_pages(&self) -> BoxStream<Result<(Page, String)>> {
        let sql = "SELECT id, url, depth, status, content FROM pages WHERE content IS NOT NULL AND status = ?";
        let r = sqlx::query(sql)
            .bind(PageStatus::Downloaded.int_value())
            .fetch(&self.0)
            .map(page_from_row);
        Box::pin(r)
    }
}

fn page_from_row(row: StdResult<SqliteRow, sqlx::Error>) -> Result<(Page, String)> {
    let row = row?;

    let page_id = row.try_get::<i64, _>("id")?;
    let url = row.try_get::<String, _>("url")?;
    let depth = row.try_get::<u16, _>("depth")?;
    let status = row.try_get::<u8, _>("status")?;
    let page = page_from_tuple((page_id, url, depth, status))?;

    let content = row.try_get::<String, _>("content")?;

    Ok((page, content))
}

/// Creates pages from tuple of its attributes
///
/// - page_id - i64
/// - url - String
/// - depth - u16
/// - status - u8
fn page_from_tuple(row: PageRow) -> Result<Page> {
    let (id, url, depth, status) = row;
    let url = Url::parse(&url)?;
    let status = PageStatus::from_int(status)?;
    Ok(Page {
        id,
        url,
        depth,
        status,
    })
}
