use crab::{
    prelude::*,
    storage::{self, Page, PageStatus, Storage},
};
use futures::StreamExt;
use std::ops::Deref;
use std::{fs::File, ops::DerefMut};
use tempfile::{tempdir, TempDir};
use tokio::test;
use url::Url;

#[test]
pub async fn count_number_of_pages_in_database() -> Result<()> {
    let storage = new_storage().await?;
    assert_eq!(0, storage.count_all_pages().await?);

    Ok(())
}

#[test]
pub async fn write_and_read_pages_to_database() -> Result<()> {
    let mut storage = new_storage().await?;

    let type_id = 1;
    let url = "http://test.com";
    let new_id = storage.register_page(url, type_id, 0).await?;
    assert_eq!(new_id, Some(1));

    let pages = storage.list_not_downloaded_pages(10).await?;

    let expected_page = Page {
        id: new_id.unwrap(),
        url: Url::parse(url)?,
        type_id,
        depth: 0,
        status: PageStatus::NotDownloaded,
    };
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0], expected_page);

    Ok(())
}

#[test]
pub async fn read_downloaded_pages() -> Result<()> {
    let mut storage = new_storage().await?;

    let url = "http://test.com";
    let expected_content = "<html>";
    let new_id = storage.register_page(url, 1, 0).await?.unwrap();
    storage.write_page_content(new_id, expected_content).await?;

    let mut pages = storage.read_downloaded_pages();
    let Some(row) = pages.next().await else {
        panic!("No pages found");
    };
    let (page, content) = row?;
    assert_eq!(page.id, new_id);
    assert_eq!(content, expected_content);

    Ok(())
}

#[test]
pub async fn page_should_be_registered_only_once() -> Result<()> {
    let mut storage = new_storage().await?;

    let page_id = storage.register_page("http://test.com", 1, 0).await?;
    assert_eq!(page_id, Some(1));

    let page_id = storage.register_page("http://test.com", 1, 0).await?;
    assert_eq!(page_id, None);

    let page_id = storage.register_page("http://test.com", 1, 0).await?;
    assert_eq!(page_id, None);

    Ok(())
}

#[test]
pub async fn write_and_read_page_content() -> Result<()> {
    let mut storage = new_storage().await?;

    let expected_type_id = 1;
    let expected_html = "<html />";

    let page_id = storage
        .register_page("http://test.com", expected_type_id, 0)
        .await?
        .unwrap();

    storage.write_page_content(page_id, expected_html).await?;

    let (html, type_id) = storage
        .read_page_content(page_id)
        .await?
        .ok_or(AppError::PageNotFound(page_id))?;
    assert_eq!(html, expected_html);
    assert_eq!(type_id, expected_type_id);

    let page = storage.read_page(page_id).await?.unwrap();
    assert_eq!(page.status, PageStatus::Downloaded);

    Ok(())
}

struct TempStorage(Storage, TempDir);

impl Deref for TempStorage {
    type Target = Storage;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TempStorage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

async fn new_storage() -> Result<TempStorage> {
    let temp_dir = tempdir()?;
    let file_name = temp_dir.path().join("sqlite.db");
    let file_name = file_name.to_str().unwrap();
    File::create(file_name)?;
    storage::migrate(file_name)?;
    let storage = Storage::new(&file_name).await?;
    Ok(TempStorage(storage, temp_dir))
}
