use crab::{
    prelude::*,
    storage::{Page, PageStatus, Storage},
};
use futures::StreamExt;
use refinery::config::{Config, ConfigDbType};
use std::fs::File;
use tempfile::{tempdir, TempDir};
use tokio::test;
use url::Url;

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
pub async fn write_and_read_pages_to_database() -> Result<()> {
    let storage = new_storage().await?;
    let storage = storage.as_ref();

    let url = "http://test.com";
    let new_id = storage.register_page(url, 0).await?;
    assert_eq!(new_id, Some(1));

    let pages = storage.list_not_downloaded_pages(10).await?;

    let expected_page = Page {
        id: new_id.unwrap(),
        url: Url::parse(url)?,
        depth: 0,
        status: PageStatus::NotDownloaded,
    };
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0], expected_page);

    Ok(())
}

#[test]
pub async fn read_downloaded_pages() -> Result<()> {
    let storage = new_storage().await?;
    let storage = storage.as_ref();

    let url = "http://test.com";
    let expected_content = "<html>";
    let new_id = storage.register_page(url, 0).await?.unwrap();
    storage.write_page_content(new_id, expected_content).await?;

    let mut pages = storage.read_downloaded_pages();
    if let Some(row) = pages.next().await {
        let (page, content) = row?;
        assert_eq!(page.id, new_id);
        assert_eq!(content, expected_content);
    } else {
        panic!("No pages found");
    }

    Ok(())
}

#[test]
pub async fn page_should_be_registered_only_once() -> Result<()> {
    let storage = new_storage().await?;
    let storage = storage.as_ref();

    let page_id = storage.register_page("http://test.com", 0).await?;
    assert_eq!(page_id, Some(1));

    let page_id = storage.register_page("http://test.com", 0).await?;
    assert_eq!(page_id, None);

    Ok(())
}

#[test]
pub async fn write_and_read_page_content() -> Result<()> {
    let storage = new_storage().await?;
    let storage = storage.as_ref();

    let page_id = storage.register_page("http://test.com", 0).await?.unwrap();

    let expected_html = "<html />";
    storage.write_page_content(page_id, expected_html).await?;

    let html = storage.read_page_content(page_id).await?;
    assert_eq!(html, Some(expected_html.to_owned()));

    let page = storage.read_page(page_id).await?.unwrap();
    assert_eq!(page.status, PageStatus::Downloaded);

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
