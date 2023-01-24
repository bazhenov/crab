use crate::{
    crawler::run_crawler, prelude::*, storage::Storage, table::Table, terminal, Navigator,
};
use atom::Atom;
use clap::Parser;
use futures::{select, FutureExt, StreamExt};
use std::{io::stdout, path::PathBuf, sync::Arc, time::Duration};
use tokio::task::spawn_blocking;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Opts {
    #[arg(short, long, value_name = "file", default_value = "./db.sqlite")]
    database: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Parser, Debug)]
pub(crate) struct RunCrawlerOpts {
    /// after downloading each page parse next pages
    #[arg(long, default_value = "false")]
    pub(crate) navigate: bool,

    /// timeout between requests in seconds
    #[arg(long, default_value = "0.0")]
    pub(crate) timeout_sec: f32,

    /// number of threads
    #[arg(long, default_value = "5")]
    pub(crate) threads: usize,

    /// path to proxies list
    #[arg(short, long)]
    pub(crate) proxies_list: Option<PathBuf>,
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
        name: Vec<String>,
        page_id: i64,
    },

    /// run KV-extraction rules on all pages and exports CSV
    ExportCsv {
        #[arg(short, long)]
        name: Vec<String>,
    },

    /// list pages in database
    ListPages,

    /// prints pages failed validation check
    Validate {
        /// resets not valid pages to initial state
        #[arg(short, long)]
        reset: bool,
    },

    /// prints a page
    Dump { page_id: i64 },

    /// resets page download status
    Reset { page_id: i64 },
}

pub async fn entrypoint<T>() -> Result<()>
where
    T: Navigator,
{
    env_logger::init();
    let opts = Opts::parse();
    let mut storage = Storage::new(&opts.database).await?;

    match opts.command {
        Commands::RunCrawler(opts) => {
            let report = Arc::new(Atom::empty());
            let tick_interval = Duration::from_millis(100);
            let terminal_handle = {
                let report = report.clone();
                spawn_blocking(move || terminal::ui(report, tick_interval))
            };
            let crawling_handle = run_crawler::<T>(storage, opts, (report, tick_interval));

            let mut crawler_handle = Box::pin(crawling_handle.fuse());
            let mut terminal_handle = Box::pin(terminal_handle.fuse());

            select! {
                // If terminal is finished first we do not want to wait on crawler
                result = terminal_handle => result??,
                // If crawler is finished first we still need to wait on terminal
                result = crawler_handle => {
                    result?;
                    terminal_handle.await??;
                },
            };
        }

        Commands::AddSeed { seed } => {
            storage.register_page(seed.as_str(), 0).await?;
        }

        Commands::Navigate { page_id } => {
            let content = storage.read_page_content(page_id).await?;
            let page = storage.read_page(page_id).await?;
            let (page, content) = page.zip(content).ok_or(AppError::PageNotFound(page_id))?;
            for link in T::next_pages(&page, &content)? {
                println!("{}", link);
            }
        }

        Commands::NavigateAll => {
            // Need to buffer all found page links so iterating over downloaded pages doesn't
            // interfere with page registering process
            let mut links = vec![];

            let mut pages = storage.read_downloaded_pages();
            while let Some(row) = pages.next().await {
                let (page, content) = row?;
                let page_links = T::next_pages(&page, &content)?;
                links.push((page.depth, page_links));
            }
            drop(pages);

            for (page_depth, page_links) in links {
                for link in page_links {
                    storage.register_page(link.as_str(), page_depth).await?;
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
            let mut pages = storage.read_downloaded_pages();

            while let Some(row) = pages.next().await {
                let (_, content) = row?;
                let kv = T::kv(&content)?.into_iter().filter(key_contains(&name));
                table.add_row(kv);
            }
            table.write(&mut stdout())?;
        }

        Commands::ListPages => {
            for page in storage.list_pages().await? {
                println!("{}", page);
            }
        }

        Commands::Validate { reset } => {
            let mut pages = storage.read_downloaded_pages();
            while let Some(row) = pages.next().await {
                let (page, content) = row?;
                if !T::validate(&content) {
                    println!("{}\t{}", page.id, page.url);
                    if reset {
                        storage.reset_page(page.id).await?;
                    }
                }
            }
        }

        Commands::Dump { page_id } => {
            let content = storage
                .read_page_content(page_id)
                .await?
                .ok_or(AppError::PageNotFound(page_id))?;
            println!("{}", content);
        }

        Commands::Reset { page_id } => storage.reset_page(page_id).await?,
    }

    Ok(())
}

/// Returns a closure for a filtering on a key contains a string
fn key_contains<T>(needles: &[String]) -> impl Fn(&(String, T)) -> bool + '_ {
    move |(key, _): &(String, T)| {
        if needles.is_empty() {
            true
        } else {
            for needle in needles {
                if key.to_lowercase().contains(&needle.to_lowercase()) {
                    return true;
                }
            }
            false
        }
    }
}
