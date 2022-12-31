use clap::Parser;
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
use test_server::TestServer;
use tokio::time::sleep;
mod cpu_database;
mod test_server;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Opts {
    #[arg(short, long, value_name = "file", default_value = "./db.sqlite")]
    database: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Parser, Debug)]
struct RunCrawlerOpts {
    /// after downloading each page parse next pages
    #[arg(long, default_value = "false")]
    navigate: bool,

    /// timeout between requests in seconds
    #[arg(long, default_value = "0.0")]
    timeout_sec: f32,

    /// number of threads
    #[arg(long, default_value = "5")]
    threads: usize,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
enum Commands {
    /// running crawler and download pages from internet
    RunCrawler(RunCrawlerOpts),
    /// add seed page to the database
    AddSeed { seed: String },
    /// run navigation rules on a given page and print outgoing links
    Navigate { page_id: i64 },
    /// run KV-extraction rules on a given page and print results
    Kv { page_id: i64 },
    /// run KV-extraction rules on all pages and exports CSV
    ExportCsv,
    /// list pages in database
    ListPages,
}

#[tokio::main]
async fn main() -> Result<()> {
    entrypoint::<TestServer>().await
}

async fn entrypoint<T>() -> Result<()>
where
    T: Navigator,
{
    env_logger::init();
    let opts = Opts::parse();
    let storage = Storage::new(&opts.database).await?;

    match opts.command {
        Commands::RunCrawler(opts) => {
            run_crawler::<T>(storage, opts).await?;
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
            let links = T::next_pages(&page, &content)?;
            for link in links {
                println!("{}", link);
            }
        }
        Commands::Kv { page_id } => {
            let content = storage
                .read_page_content(page_id)
                .await?
                .ok_or(AppError::PageNotFound(page_id))?;
            let kv = T::kv(&content)?;
            println!("{:#?}", kv);
        }
        Commands::ExportCsv => {
            let mut table = Table::default();
            for page in storage.list_downloaded_pages().await? {
                let content = storage
                    .read_page_content(page)
                    .await?
                    .ok_or(AppError::PageNotFound(page))?;
                let kv = T::kv(&content)?;
                if !kv.is_empty() {
                    table.add_row(kv);
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

async fn run_crawler<T: Navigator>(storage: Storage, opts: RunCrawlerOpts) -> Result<()> {
    let delay = Duration::from_secs_f32(opts.timeout_sec);
    let mut futures = FuturesUnordered::new();
    let mut pages = vec![];
    loop {
        // REFILLING PHASE
        if pages.is_empty() && futures.is_empty() {
            pages = storage.list_not_downloaded_pages(100).await?;
            if pages.is_empty() {
                break;
            }
        }

        // DISPATCHING PHASE
        while futures.len() < opts.threads && !pages.is_empty() {
            let next_page = pages.swap_remove(0);
            let future = tokio::spawn(fetch_content(next_page, delay));
            futures.push(future);
        }

        // COMPLETING PHASE
        if !futures.is_empty() {
            if let Some(completed) = futures.next().await {
                let (page, response) = completed?;
                match response {
                    Ok(content) => {
                        storage.write_page_content(page.id, &content).await?;

                        if opts.navigate {
                            for link in T::next_pages(&page, &content)? {
                                storage.register_page(link.as_str(), page.depth + 1).await?;
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Unable to download: {}", page.url);
                        debug!("{}", e);
                        storage.mark_page_as_failed(page.id).await?;
                    }
                }
            }
        }
    }
    Ok(())
}

async fn fetch_content(page: Page, delay: Duration) -> (Page, Result<String>) {
    trace!("Starting: {}", &page.url);
    let instant = Instant::now();
    let response = download(page.url.as_ref()).await;
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
