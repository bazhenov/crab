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

    pub async fn version(&self) -> Result<String> {
        let row: (String,) = sqlx::query_as("SELECT sqlite_version()")
            .fetch_one(&self.connection)
            .await?;
        Ok(row.0)
    }
}
