use crab::{prelude::*, storage::Storage};
use refinery::config::{Config, ConfigDbType};
use std::fs::File;
use tempfile::{tempdir, TempDir};
use tokio::test;

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./migrations");
}

#[test]
pub async fn count_number_of_pages_in_database() -> Result<()> {
    let storage = new_storage().await?;
    assert_eq!(0, storage.as_ref().count_all_pages().await?);

    Ok(())
}

#[test]
pub async fn read_and_write_pages_to_database() -> Result<()> {
    let storage = new_storage().await?;
    let storage = storage.as_ref();

    let page = "http://test.com";
    let new_id = storage.register_seed_page(page).await?;
    assert_eq!(new_id, 1);
    let pages = storage.read_fresh_pages(10).await?;
    assert_eq!(pages, vec![page.to_owned()]);

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
