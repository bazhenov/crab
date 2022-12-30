use clap::Parser;
use cpu_database::CpuDatabase;
use crab::{
    prelude::*,
    storage::{Page, Storage},
    table::Table,
    Navigator,
};
use futures::{stream::FuturesUnordered, StreamExt};
use std::{
    io::stdout,
    time::{Duration, Instant},
};
use tokio::time::sleep;
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
    ExportCsv,
    ListPages,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let opts = Opts::parse();

    let storage = Storage::new(&opts.database).await?;

    match opts.command {
        Commands::RunCrawler {} => {
            Crawler::new(storage)?.run().await?;
        }
        Commands::AddSeed { seed } => {
            storage.register_page(&seed, 0).await?;
        }
        Commands::Navigate { page_id } => {
            let content = storage
                .read_page_content(page_id)
                .await?
                .ok_or(AppError::PageNotFound(page_id))?;
            let page = storage
                .read_page(page_id)
                .await?
                .ok_or(AppError::PageNotFound(page_id))?;
            let links = CpuDatabase::next_pages(&page, &content)?;
            for link in links {
                println!("{}", link);
            }
        }
        Commands::RunKv { page_id } => {
            let content = storage
                .read_page_content(page_id)
                .await?
                .ok_or(AppError::PageNotFound(page_id))?;
            let kv = CpuDatabase::kv(&content)?;
            println!("{:#?}", kv);
        }
        Commands::ExportCsv => {
            let pages = storage.list_downloaded_pages().await?;
            let mut table = Table::new();
            for page in pages {
                if let Some(content) = storage.read_page_content(page).await? {
                    let kv = CpuDatabase::kv(&content)?;
                    if !kv.is_empty() {
                        table.add_row(kv);
                    }
                }
            }
            table.write(&mut stdout())?;
        }
        Commands::ListPages => {
            for page in storage.list_pages().await? {
                println!("{}", page);
            }
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
        let delay = Duration::from_secs(10);
        let mut futures = FuturesUnordered::new();
        let mut pages = vec![];
        loop {
            // REFILLING PHASE
            if pages.is_empty() && futures.is_empty() {
                pages = self.storage.list_not_downloaded_pages(100).await?;
                if pages.is_empty() {
                    break;
                }
            }

            // DISPATCHING PHASE
            while futures.len() < n_max && !pages.is_empty() {
                let next_page = pages.swap_remove(0);
                let future = tokio::spawn(fetch_content(next_page, delay));
                futures.push(future);
            }

            // COMPLETEING PHASE
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
                            debug!("{}", e);
                            self.storage.mark_page_as_failed(page.id).await?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

async fn fetch_content(page: Page, delay: Duration) -> (Page, Result<String>) {
    trace!("Starting: {}", &page.url);
    let instant = Instant::now();
    let response = download(&page.url.to_string()).await;
    let duration = instant.elapsed();
    info!(
        "Downloaded in {:.1}s: {}",
        duration.as_secs_f32(),
        &page.url
    );
    sleep(delay).await;
    (page, response)
}

async fn download(url: &str) -> Result<String> {
    Ok(reqwest::get(url).await?.text().await?)
}
