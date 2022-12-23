use std::fs::File;

use crab::prelude::*;
use crab::storage::Storage;
use refinery::config::{Config, ConfigDbType};
use tempfile::{tempdir, TempDir};

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./migrations");
}

#[tokio::test]
pub async fn count_number_of_pages_in_database() -> Result<()> {
    let storage = new_storage().await?;
    assert_eq!(0, storage.as_ref().count_pages().await?);

    Ok(())
}

struct TempStorage(Storage, TempDir);

impl AsRef<Storage> for TempStorage {
    fn as_ref(&self) -> &Storage {
        &self.0
    }
}

async fn new_storage() -> Result<TempStorage> {
    let temp_dir = tempdir()?;
    let file_name = temp_dir.path().join("sqlite.db");
    let file_name = file_name.to_str().unwrap();
    File::create(file_name)?;

    let mut config = Config::new(ConfigDbType::Sqlite).set_db_path(&file_name);
    embedded::migrations::runner().run(&mut config)?;
    let storage = Storage::new(&file_name).await?;
    Ok(TempStorage(storage, temp_dir))
}
