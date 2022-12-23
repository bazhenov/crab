use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

use crate::prelude::*;

pub struct Storage {
    connection: SqlitePool,
}

impl Storage {
    pub async fn new(url: &str) -> Result<Self> {
        let connection = SqlitePoolOptions::new().connect(url).await?;
        Ok(Self { connection })
    }

    pub async fn count_pages(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pages")
            .fetch_one(&self.connection)
            .await?;
        Ok(row.0)
    }
}
