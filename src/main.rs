use clap::Parser;
use cpu_database::CpuDatabase;
use crab::{
    prelude::*,
    storage::{Page, Storage},
    Navigator,
};
use futures::{stream::FuturesUnordered, StreamExt};
mod cpu_database;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Opts {
    #[arg(short, long, value_name = "file", default_value = "./db.sqlite")]
    database: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
enum Commands {
    RunCrawler {},
    AddSeed { seed: String },
    Navigate { page_id: i64 },
    RunKv { page_id: i64 },
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let opts = Opts::parse();

    match opts.command {
        Commands::RunCrawler {} => {
            let storage = Storage::new(&opts.database).await?;
            Crawler::new(storage)?.run().await?;
        }
        Commands::AddSeed { seed } => {
            let storage = Storage::new(&opts.database).await?;
            storage.register_seed_page(&seed).await?;
        }
        Commands::Navigate { page_id } => {
            let storage = Storage::new(&opts.database).await?;
            let content = storage
                .read_page_content(page_id)
                .await?
                .ok_or(Error::PageNotFound(page_id))?;
            let page = storage
                .read_page(page_id)
                .await?
                .ok_or(Error::PageNotFound(page_id))?;
            let links = CpuDatabase::next_pages(&page, &content)?;
            for link in links {
                println!("{}", link);
            }
        }
        Commands::RunKv { page_id } => {
            let storage = Storage::new(&opts.database).await?;
            let content = storage
                .read_page_content(page_id)
                .await?
                .ok_or(Error::PageNotFound(page_id))?;
            let kv = CpuDatabase::kv(&content)?;
            println!("{:#?}", kv);
        }
    }

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
                        Ok(content) => {
                            self.storage.write_page_content(page.id, &content).await?;
                            let links = CpuDatabase::next_pages(&page, &content)?;
                            for link in links {
                                self.storage
                                    .register_page(link.as_str(), page.depth + 1)
                                    .await?;
                            }
                        }
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
    let response = download(&page.url.to_string()).await;
    trace!("Finished loading of: {}", &page.url);
    (page, response)
}

async fn download(url: &str) -> Result<String> {
    Ok(reqwest::get(url).await?.text().await?)
}
