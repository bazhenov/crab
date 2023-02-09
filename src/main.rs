use anyhow::Context;
use atom::Atom;
use clap::Parser;
use crab::{
    crawler::{run_crawler, RunCrawlerOpts},
    prelude::*,
    python::{self, PythonPageParser},
    storage::Storage,
    PageParser, PageParsers, PageTypeId,
};
use futures::{select, FutureExt, StreamExt};
use std::{env::current_dir, fs, io::stdout, path::Path, sync::Arc, time::Duration};
use table::Table;
use tokio::task::spawn_blocking;

mod table;
mod terminal;

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
    /// running crawler and download pages from internet
    RunCrawler(RunCrawlerOpts),

    /// add page to the database
    Register { url: String, type_id: PageTypeId },

    /// run navigation rules on a given page and print outgoing links
    Navigate { page_id: i64 },

    /// run navigation rules on all downloaded pages and writes found links back to pages database
    NavigateAll,

    /// run parsing rules on a given page and print results
    Parse {
        #[arg(short, long)]
        name: Vec<String>,
        page_id: i64,
    },

    /// run parsing rules on all pages and exports CSV
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

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let opts = Opts::parse();
    let mut storage = Storage::new(&opts.database).await?;
    let parsers = PageParsers(create_python_parsers(current_dir()?)?);

    match opts.command {
        Commands::RunCrawler(opts) => {
            let report = Arc::new(Atom::empty());
            let tick_interval = Duration::from_millis(100);
            let terminal_handle = {
                let report = report.clone();
                spawn_blocking(move || terminal::ui(report, tick_interval))
            };
            let crawling_handle = run_crawler(parsers, storage, opts, (report, tick_interval));

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

        Commands::Register { url, type_id } => {
            storage.register_page(url.as_str(), type_id, 0).await?;
        }

        Commands::Navigate { page_id } => {
            let content = storage.read_page_content(page_id).await?;
            let page = storage.read_page(page_id).await?;
            let (page, (content, _)) = page.zip(content).ok_or(AppError::PageNotFound(page_id))?;
            for (link, type_id) in parsers.navigate(&page, &content)?.unwrap_or_default() {
                println!("{:3}  {}", type_id, link);
            }
        }

        Commands::NavigateAll => {
            // Need to buffer all found page links so iterating over downloaded pages doesn't
            // interfere with page registering process
            let mut links = vec![];

            let mut pages = storage.read_downloaded_pages();
            while let Some(row) = pages.next().await {
                let (page, content) = row?;
                let page_links = parsers.navigate(&page, &content)?;
                links.push((page.depth, page_links));
            }
            drop(pages);

            for (page_depth, page_links) in links {
                for (link, type_id) in page_links.unwrap_or_default() {
                    storage
                        .register_page(link.as_str(), type_id, page_depth)
                        .await?;
                }
            }
        }

        Commands::Parse { name, page_id } => {
            let (content, type_id) = storage
                .read_page_content(page_id)
                .await?
                .ok_or(AppError::PageNotFound(page_id))?;
            let pairs = parsers.parse(type_id, &content)?.unwrap_or_default();
            for (key, value) in pairs.into_iter().filter(key_contains(&name)) {
                println!("{}: {}", &key, &value)
            }
        }

        Commands::ExportCsv { name } => {
            let mut table = Table::default();
            let mut pages = storage.read_downloaded_pages();

            while let Some(row) = pages.next().await {
                let (page, content) = row?;
                let pairs = parsers
                    .parse(page.type_id, &content)?
                    .unwrap_or_default()
                    .into_iter()
                    .filter(key_contains(&name));
                table.add_row(pairs);
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
                if !parsers.validate(page.type_id, &content)? {
                    println!("{}\t{}", page.id, page.url);
                    if reset {
                        storage.reset_page(page.id).await?;
                    }
                }
            }
        }

        Commands::Dump { page_id } => {
            let (content, _) = storage
                .read_page_content(page_id)
                .await?
                .ok_or(AppError::PageNotFound(page_id))?;
            println!("{}", content);
        }

        Commands::Reset { page_id } => storage.reset_page(page_id).await?,
    }

    Ok(())
}

/// Initialize python environment and create python parser.
///
/// Python parsers created using following convention:
/// * each parser is located in separate python file in current working directory;
/// * each parser should be named `{type_id}_name.py`, where `type_id` is [`PageTypeId`] of a parser
/// (eg. `1_listing_page.py`).
fn create_python_parsers(path: impl AsRef<Path>) -> Result<Vec<Box<dyn PageParser>>> {
    python::prepare();
    let mut parsers: Vec<Box<dyn PageParser>> = vec![];
    for path in fs::read_dir(path)? {
        let path = path?;
        let file_name = path.file_name();
        let file_name = file_name.to_str().unwrap_or_default();
        if path.path().is_file() && file_name.starts_with("parser_") {
            if let Some(module_name) = file_name.strip_suffix(".py") {
                trace!("Building parser from python file: {}", file_name);
                let parser = PythonPageParser::new(module_name)
                    .context(AppError::UnableToCreateParser(path.path()))?;
                parsers.push(Box::new(parser))
            }
        }
    }
    Ok(parsers)
}

/// Returns a closure for a filtering on a key contains a string
fn key_contains<T>(needles: &[String]) -> impl Fn(&(String, T)) -> bool + '_ {
    move |(key, _): &(String, T)| {
        if needles.is_empty() {
            true
        } else {
            let key = key.to_lowercase();
            needles
                .iter()
                .map(|s| s.to_lowercase())
                .any(|needle| key.contains(&needle))
        }
    }
}
