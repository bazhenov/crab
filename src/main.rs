use crab::{
    prelude::*,
    storage::{Page, Storage},
};
use futures::{stream::FuturesUnordered, StreamExt};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let storage = Storage::new("file:./db.sqlite").await?;
    let crawler = Crawler::new(storage)?;

    crawler.run().await?;

    Ok(())
}

struct Crawler {
    storage: Storage,
}

impl Crawler {
    pub fn new(storage: Storage) -> Result<Self> {
        Ok(Self { storage })
    }

    pub async fn run(&self) -> Result<()> {
        let n_max = 2;
        let mut futures = FuturesUnordered::new();
        let mut pages = vec![];
        loop {
            if pages.is_empty() && futures.is_empty() {
                pages = self.storage.read_fresh_pages(100).await?;
                if pages.is_empty() {
                    break;
                }
            }

            while futures.len() < n_max && !pages.is_empty() {
                let next_page = pages.swap_remove(0);
                let future = tokio::spawn(fetch_content(next_page));
                futures.push(future);
            }

            if !futures.is_empty() {
                if let Some(completed) = futures.next().await {
                    let (page, response) = completed?;
                    match response {
                        Ok(content) => self.storage.write_page_content(page.id, &content).await?,
                        Err(e) => {
                            warn!("Unable to download: {}", page.url);
                            warn!("{}", e);
                            self.storage.write_page_content(page.id, "Error").await?
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

async fn fetch_content(page: Page) -> (Page, Result<String>) {
    trace!("Strating loading of: {}", &page.url);
    let response = download(&page.url).await;
    trace!("Finished loading of: {}", &page.url);
    (page, response)
}

async fn download(url: &str) -> Result<String> {
    Ok(reqwest::get(url).await?.text().await?)
}
