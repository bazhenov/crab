use anyhow::Context;
use clap::Parser;
use crab::{prelude::*, proxy::Proxies, storage::Storage, table::Table, Navigator};
use futures::{stream::FuturesUnordered, StreamExt};
use reqwest::{Client, Proxy, Url};
use std::{
    io::stdout,
    path::PathBuf,
    time::{Duration, Instant},
};
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

    /// path to proxies list
    #[arg(short, long)]
    proxies_list: Option<PathBuf>,
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
    /// run navigation rules on all downloaded pages and writes found links back to pages database
    NavigateAll,
    /// run KV-extraction rules on a given page and print results
    Kv {
        #[arg(short, long)]
        name: Option<String>,
        page_id: i64,
    },
    /// run KV-extraction rules on all pages and exports CSV
    ExportCsv {
        #[arg(short, long)]
        name: Option<String>,
    },
    /// list pages in database
    ListPages,
}

#[tokio::main]
async fn main() -> Result<()> {
    entrypoint::<cpu_database::CpuDatabase>().await
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

        Commands::NavigateAll => {
            for page_id in storage.list_downloaded_pages().await? {
                let page = storage
                    .read_page(page_id)
                    .await?
                    .ok_or(AppError::PageNotFound(page_id))?;
                let content = storage
                    .read_page_content(page_id)
                    .await?
                    .ok_or(AppError::PageNotFound(page_id))?;
                for link in T::next_pages(&page, &content)? {
                    storage.register_page(link.as_str(), page.depth + 1).await?;
                }
            }
        }

        Commands::Kv { name, page_id } => {
            let content = storage
                .read_page_content(page_id)
                .await?
                .ok_or(AppError::PageNotFound(page_id))?;
            let kv = T::kv(&content)?;
            for (key, value) in kv.into_iter().filter(key_contains(&name)) {
                println!("{}: {}", &key, &value)
            }
        }

        Commands::ExportCsv { name } => {
            let mut table = Table::default();
            for page_id in storage.list_downloaded_pages().await? {
                let content = storage
                    .read_page_content(page_id)
                    .await?
                    .ok_or(AppError::PageNotFound(page_id))?;
                let kv = T::kv(&content)?;
                if !kv.is_empty() {
                    table.add_row(kv.into_iter().filter(key_contains(&name)));
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
    let mut proxies = match opts.proxies_list {
        Some(path) => Proxies::from_file(&path).context(AppError::UnableToOpenProxyList(path))?,
        None => Proxies::default(),
    };

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
            let next_proxy = proxies.next();
            let (proxy, proxy_id) = next_proxy.unzip();
            let client = create_http_client(proxy)?;
            let future = tokio::spawn(async move {
                let content = fetch_content(client, next_page.url.clone(), delay).await;
                (proxy_id, next_page, content)
            });
            futures.push(future);
        }

        // COMPLETING PHASE
        if !futures.is_empty() {
            if let Some(completed) = futures.next().await {
                let (proxy, page, response) = completed?;
                match response {
                    Ok(content) => {
                        storage.write_page_content(page.id, &content).await?;
                        if let Some(proxy) = proxy {
                            proxies.proxy_succeseed(proxy);
                        }

                        if opts.navigate {
                            for link in T::next_pages(&page, &content)? {
                                storage.register_page(link.as_str(), page.depth + 1).await?;
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Unable to download: {}", page.url);
                        debug!("{}", e);
                        // storage.mark_page_as_failed(page.id).await?;
                        if let Some(proxy) = proxy {
                            proxies.proxy_failed(proxy);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn create_http_client(proxy: Option<Proxy>) -> Result<Client> {
    let mut builder = Client::builder();
    if let Some(proxy) = proxy {
        builder = builder.proxy(proxy);
    }
    let client = builder
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .build()?;
    Ok(client)
}

async fn fetch_content(client: Client, url: Url, delay: Duration) -> Result<String> {
    trace!("Starting: {}", &url);
    let instant = Instant::now();
    let response = download(client, url.as_ref()).await;
    let duration = instant.elapsed();
    trace!("Downloaded in {:.1}s: {}", duration.as_secs_f32(), &url);
    sleep(delay).await;
    response
}

async fn download(client: Client, url: &str) -> Result<String> {
    Ok(client.get(url).send().await?.text().await?)
}

/// Returns a closure for a filtering on a key contains a string
fn key_contains<'a, T>(needle: &'a Option<String>) -> impl Fn(&(String, T)) -> bool + 'a {
    move |(key, _): &(String, T)| match needle {
        Some(needle) => key.to_lowercase().contains(&needle.to_lowercase()),
        _ => false,
    }
}
