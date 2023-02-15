use anyhow::Context;
use atom::Atom;
use clap::Parser;
use crab::{
    crawler::run_crawler,
    prelude::*,
    python::{self, PythonPageParser},
    storage::{self, Storage},
    CrabConfig, CrawlerReport, PageParser, PageParsers, PageTypeId,
};
use futures::{select, FutureExt, StreamExt};
use std::{
    fs::{self, File},
    io::stdout,
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use table::Table;
use tokio::task::spawn_blocking;

mod table;
mod terminal;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Opts {
    #[arg(short = 'w', default_value = ".")]
    workspace: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
enum Commands {
    /// migrates database to a new version
    Migrate,

    /// create new parsing environment
    New {
        /// path to workspace
        workspace: PathBuf,
    },

    /// running crawler and download pages from the Internet
    RunCrawler {
        /// after downloading each page parse next pages
        #[arg(long, default_value = "false")]
        navigate: bool,
    },

    /// add page to the database
    Register { url: String, type_id: PageTypeId },

    /// run navigation rules on a given page and print outgoing links
    Navigate { page_id: i64 },

    /// run navigation rules on all downloaded pages and write found links back to the pages database
    NavigateAll,

    /// run parsing rules on the given page and print results
    Parse {
        /// list of comma separated column names to filter
        #[arg(short = 'n')]
        columns: Vec<String>,
        // page id to parse
        page_id: i64,
    },

    /// run parsing rules on all pages and exports CSV
    ExportTable {
        /// list of comma separated column names to filter
        #[arg(short = 'n')]
        columns: Vec<String>,
        /// table name to print
        table: String,
    },

    /// list pages in the database
    ListPages {
        /// disable header output
        #[arg(short = 'n', long, default_value_t = false)]
        no_header: bool,
    },

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

    /// display information about parsers
    Parsers,
}

#[tokio::main]
async fn main() -> Result<()> {
    // temporary workaround of https://github.com/rust-lang/rust-analyzer/issues/14137
    entrypoint().await
}

fn read_config(path: impl AsRef<Path>) -> Result<CrabConfig> {
    let toml = fs::read_to_string(&path)?;
    Ok(toml::from_str(&toml)?)
}

async fn read_env(opts: &Opts) -> Result<(CrabConfig, Storage, PageParsers)> {
    let config_path = opts.workspace.join("crab.toml");
    let config = read_config(&config_path).context(AppError::ReadingConfig(config_path.clone()))?;

    let database_path = config.database.to_str().unwrap();
    let storage = Storage::new(database_path)
        .await
        .context(AppError::OpeningDatabase)?;

    let parsers =
        create_dyn_python_parsers(&opts.workspace).context(AppError::LoadingPythonParsers)?;
    let parsers = PageParsers(parsers);
    Ok((config, storage, parsers))
}

async fn entrypoint() -> Result<()> {
    env_logger::init();
    let app_opts = Opts::parse();

    match &app_opts.command {
        Commands::New { workspace } => {
            fs::create_dir(workspace)?;

            let config = CrabConfig::default_config();
            fs::write(workspace.join("crab.toml"), toml::to_string(&config)?)?;

            let database_path = workspace.join(&config.database);
            File::create(&database_path)?;
            storage::migrate(database_path)?;
            fs::write(
                workspace.join("parser_home_page.py"),
                include_str!("example_parser.py"),
            )?;
        }

        Commands::Migrate => {
            let (config, _, _) = read_env(&app_opts).await?;
            storage::migrate(config.database)?;
        }

        Commands::RunCrawler { navigate } => {
            let (config, storage, parsers) = read_env(&app_opts).await?;
            let report = Arc::new(Atom::empty());
            let tick_interval = Duration::from_millis(100);
            let terminal_handle = {
                let report = report.clone();
                spawn_blocking(move || terminal::ui(report, tick_interval))
            };
            let crawling_handle = run_crawler(
                parsers,
                storage,
                config.crawler,
                *navigate,
                (report.clone(), tick_interval),
            );

            let mut crawler_handle = Box::pin(crawling_handle.fuse());
            let mut terminal_handle = Box::pin(terminal_handle.fuse());

            select! {
                // If terminal is finished first we do not want to wait on crawler
                result = terminal_handle => result??,
                // If crawler is finished first we still need to wait on terminal
                result = crawler_handle => {
                    report.swap(Box::new(CrawlerReport::Finished), Ordering::Relaxed);
                    result?;
                    terminal_handle.await??;
                },
            };
        }

        Commands::Register { url, type_id } => {
            let (_, mut storage, _) = read_env(&app_opts).await?;
            storage.register_page(url.as_str(), *type_id, 0).await?;
        }

        Commands::Navigate { page_id } => {
            let (_, storage, parsers) = read_env(&app_opts).await?;
            let content = storage.read_page_content(*page_id).await?;
            let page = storage.read_page(*page_id).await?;
            let (page, (content, _)) = page.zip(content).ok_or(AppError::PageNotFound(*page_id))?;
            for (link, type_id) in parsers.navigate(&page, &content)?.unwrap_or_default() {
                println!("{:3}  {}", type_id, link);
            }
        }

        Commands::NavigateAll => {
            let (_, mut storage, parsers) = read_env(&app_opts).await?;
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

        Commands::Parse { columns, page_id } => {
            let (_, storage, parsers) = read_env(&app_opts).await?;
            let (content, type_id) = storage
                .read_page_content(*page_id)
                .await?
                .ok_or(AppError::PageNotFound(*page_id))?;
            let tables = parsers.parse(type_id, &content)?.unwrap_or_default();
            for (table_name, table) in tables.into_iter() {
                println!("{table_name}");
                println!("------------------------");
                for row in table.into_iter() {
                    let columns = row.into_iter().filter(column_contains(columns));
                    for (idx, (column, value)) in columns.enumerate() {
                        let prefix = if idx == 0 { "-" } else { " " };
                        println!("{} {}: {}", prefix, &column, &value);
                    }
                }
                println!();
            }
        }

        Commands::ExportTable { table, columns } => {
            let (_, storage, parsers) = read_env(&app_opts).await?;
            let mut csv = Table::default();
            let mut pages = storage.read_downloaded_pages();

            while let Some(row) = pages.next().await {
                let (page, content) = row?;
                let mut tables = parsers.parse(page.type_id, &content)?.unwrap_or_default();
                let table = tables.remove(table).unwrap_or_default();
                for row in table.into_iter() {
                    csv.add_row(row.into_iter().filter(column_contains(columns)));
                }
            }
            csv.write(&mut stdout())?;
        }

        Commands::ListPages { no_header } => {
            let (_, storage, _) = read_env(&app_opts).await?;
            if !no_header {
                println!(
                    "{:>7}  {:>7}  {:>5}  {:<15}  {:<20}",
                    "id", "type_id", "depth", "status", "url"
                );
                println!("{}", "-".repeat(120));
            }
            for page in storage.list_pages().await? {
                println!(
                    "{:>7}  {:>7}  {:>5}  {:<15}  {:<20}",
                    page.id, page.type_id, page.depth, page.status, page.url
                )
            }
        }

        Commands::Validate { reset } => {
            let (_, storage, parsers) = read_env(&app_opts).await?;

            let mut invalid_pages = vec![];
            let mut pages = storage.read_downloaded_pages();
            while let Some(row) = pages.next().await {
                let (page, content) = row?;
                if !parsers.validate(page.type_id, &content)? {
                    println!("{}\t{}", page.id, page.url);
                    invalid_pages.push(page.id);
                }
            }

            // Page reset should be done after page iteration process is completed.
            // Lock timeout will be generated otherwise.
            if *reset {
                drop(pages);
                for page_id in invalid_pages.into_iter() {
                    storage.reset_page(page_id).await?;
                }
            }
        }

        Commands::Dump { page_id } => {
            let (_, storage, _) = read_env(&app_opts).await?;
            let (content, _) = storage
                .read_page_content(*page_id)
                .await?
                .ok_or(AppError::PageNotFound(*page_id))?;
            println!("{}", content);
        }

        Commands::Reset { page_id } => {
            let (_, storage, _) = read_env(&app_opts).await?;
            storage.reset_page(*page_id).await?
        }

        Commands::Parsers => {
            println!(
                "{:<25}   {:>8}   {:<12} {:<12} {:<12}",
                "MODULE NAME", "TYPE ID", "NAVIGATION", "PARSING", "VALIDATION"
            );
            for parser in create_python_parsers(&app_opts.workspace)? {
                println!(
                    "{:<25}   {:>8}   {:<12} {:<12} {:<12}",
                    parser.module_name(),
                    parser.page_type_id(),
                    label(parser.support_navigation(), "yes", "no"),
                    label(parser.support_parsing(), "yes", "no"),
                    label(parser.support_validation(), "yes", "no")
                )
            }
        }
    }

    Ok(())
}

fn label<'a>(v: bool, yes: &'a str, no: &'a str) -> &'a str {
    if v {
        yes
    } else {
        no
    }
}

/// Initialize python environment and create python parser.
///
/// Python parsers created using following convention:
/// * each parser is located in separate python file in current working directory;
/// * each parser should be named `{type_id}_name.py`, where `type_id` is [`PageTypeId`] of a parser
/// (eg. `1_listing_page.py`).
fn create_dyn_python_parsers(path: impl AsRef<Path>) -> Result<Vec<Box<dyn PageParser>>> {
    Ok(create_python_parsers(path)?
        .into_iter()
        .map(heap_allocate)
        .collect())
}

fn heap_allocate<T: PageParser + 'static>(parser: T) -> Box<dyn PageParser> {
    Box::new(parser)
}

/// Initialize python environment and create python parser.
///
/// Python parsers created using following convention:
/// * each parser is located in separate python file in current working directory;
/// * each parser should be named `{type_id}_name.py`, where `type_id` is [`PageTypeId`] of a parser
/// (eg. `1_listing_page.py`).
fn create_python_parsers(path: impl AsRef<Path>) -> Result<Vec<PythonPageParser>> {
    python::prepare();
    let mut parsers = vec![];
    for path in fs::read_dir(path)? {
        let path = path?;
        let file_name = path.file_name();
        let file_name = file_name.to_str().unwrap_or_default();
        if path.path().is_file() && file_name.starts_with("parser_") {
            if let Some(module_name) = file_name.strip_suffix(".py") {
                trace!("Building parser from python file: {}", file_name);
                let parser = PythonPageParser::new(module_name)
                    .context(AppError::UnableToCreateParser(path.path()))?;
                parsers.push(parser)
            }
        }
    }
    Ok(parsers)
}

/// Returns a closure for a filtering on a key contains a string
fn column_contains<T>(needles: &[String]) -> impl Fn(&(String, T)) -> bool + '_ {
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
