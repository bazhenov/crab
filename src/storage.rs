use crate::{prelude::*, PageTypeId};
use futures::{stream::BoxStream, StreamExt};
use int_enum::IntEnum;
use sqlx::{
    sqlite::{SqlitePoolOptions, SqliteRow},
    Row, SqlitePool,
};
use std::{fmt, path::Path};
use url::Url;

use refinery::{
    config::{Config, ConfigDbType},
    embed_migrations,
};
embed_migrations!("./migrations");

pub struct Storage {
    connection: SqlitePool,

    /// `sqlite3_last_insert_rowid()` doesn't change return value when INSERT OR IGNORE statement
    /// fails to insert new row in a table. We rely on last insert id when detecting if record is
    /// present in a database already. [Last Insert Rowid](https://www.sqlite.org/c3ref/last_insert_rowid.html)
    last_insert_id: i64,
}

#[repr(u8)]
#[derive(Debug, PartialEq, Clone, Copy, IntEnum, Eq, Hash)]
pub enum PageStatus {
    NotDownloaded = 1,
    Downloaded = 2,
}

impl fmt::Display for PageStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let display_value = match self {
            PageStatus::NotDownloaded => "not downloaded",
            PageStatus::Downloaded => "downloaded",
        };
        f.pad(display_value)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Page {
    pub id: i64,
    pub url: Url,
    pub type_id: PageTypeId,
    pub depth: u16,
    pub status: PageStatus,
}

type PageRow = (i64, String, PageTypeId, u16, u8);

impl Storage {
    pub async fn new(url: &str) -> Result<Self> {
        let connection = SqlitePoolOptions::new().connect(url).await?;
        let last_insert_id = 0;
        Ok(Self {
            connection,
            last_insert_id,
        })
    }

    pub async fn count_all_pages(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pages")
            .fetch_one(&self.connection)
            .await?;
        Ok(row.0)
    }

    pub async fn list_pages(&self) -> Result<Vec<Page>> {
        let query = "SELECT id, url, type, depth, status FROM pages";
        let result_set: Vec<PageRow> = sqlx::query_as(query).fetch_all(&self.connection).await?;
        let mut pages = vec![];
        for row in result_set {
            pages.push(page_from_tuple(row)?);
        }
        Ok(pages)
    }

    /// Registers new page
    ///
    /// If page with given URL already exists, [`Option::None`] is returned.
    pub async fn register_page<U: TryInto<Url>>(
        &mut self,
        url: U,
        type_id: PageTypeId,
        depth: u16,
    ) -> Result<Option<i64>>
    where
        U::Error: Sync + Send + std::error::Error + 'static,
    {
        let new_id = sqlx::query("INSERT OR IGNORE INTO pages (url, type, depth) VALUES (?, ?, ?)")
            .bind(url.try_into()?.to_string())
            .bind(type_id)
            .bind(depth)
            .execute(&self.connection)
            .await?
            .last_insert_rowid();
        if new_id > 0 && new_id != self.last_insert_id {
            self.last_insert_id = new_id;
            Ok(Some(new_id))
        } else {
            Ok(None)
        }
    }

    pub async fn list_not_downloaded_pages(&self, count: u16) -> Result<Vec<Page>> {
        let query =
            "SELECT id, url, type, depth, status FROM pages WHERE status = ? ORDER BY depth ASC LIMIT ?";
        let result_set: Vec<PageRow> = sqlx::query_as(query)
            .bind(PageStatus::NotDownloaded.int_value())
            .bind(count)
            .fetch_all(&self.connection)
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
            .execute(&self.connection)
            .await?;
        Ok(())
    }

    /// Writes page content in storage and marks page as [`PageStatus::Downloaded`]
    pub async fn write_page_content(&self, page_id: i64, content: &str) -> Result<()> {
        sqlx::query("UPDATE pages SET content = ?, status = ? WHERE id = ?")
            .bind(content)
            .bind(PageStatus::Downloaded.int_value())
            .bind(page_id)
            .execute(&self.connection)
            .await?;
        Ok(())
    }

    pub async fn read_page(&self, id: i64) -> Result<Option<Page>> {
        sqlx::query_as("SELECT id, url, type, depth, status FROM pages WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.connection)
            .await?
            .map(page_from_tuple)
            .transpose()
    }

    pub async fn read_page_content(&self, id: i64) -> Result<Option<(String, PageTypeId)>> {
        let content: Option<(String, PageTypeId)> =
            sqlx::query_as("SELECT content, type FROM pages WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.connection)
                .await?;
        Ok(content)
    }

    /// Lists downloaded pages and its content
    pub fn read_downloaded_pages(&self) -> BoxStream<Result<(Page, String)>> {
        let sql = "SELECT id, url, type, depth, status, content FROM pages WHERE content IS NOT NULL AND status = ?";
        let r = sqlx::query(sql)
            .bind(PageStatus::Downloaded.int_value())
            .fetch(&self.connection)
            .map(page_from_row);
        Box::pin(r)
    }
}

fn page_from_row(row: StdResult<SqliteRow, sqlx::Error>) -> Result<(Page, String)> {
    let row = row?;

    let page_id: i64 = row.try_get("id")?;
    let url: String = row.try_get("url")?;
    let depth: u16 = row.try_get("depth")?;
    let type_id: PageTypeId = row.try_get("type")?;
    let status: u8 = row.try_get("status")?;
    let page = page_from_tuple((page_id, url, type_id, depth, status))?;

    let content: String = row.try_get("content")?;

    Ok((page, content))
}

/// Creates pages from tuple of its attributes
///
/// - page_id - i64
/// - url - String
/// - type_id - PageType
/// - depth - u16
/// - status - u8
fn page_from_tuple(row: PageRow) -> Result<Page> {
    let (id, url, type_id, depth, status) = row;
    let url = Url::parse(&url)?;
    let status = PageStatus::from_int(status)?;
    Ok(Page {
        id,
        url,
        type_id,
        depth,
        status,
    })
}

pub fn migrate(path: impl AsRef<Path>) -> Result<()> {
    let database_path = path.as_ref().to_string_lossy();
    let mut config = Config::new(ConfigDbType::Sqlite).set_db_path(database_path.as_ref());
    migrations::runner().run(&mut config)?;
    Ok(())
}
