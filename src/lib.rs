use anyhow::Context;
use atom::Atom;
use crawler::CrawlerState;
use prelude::*;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
pub use storage::Page;
use url::Url;

pub mod crawler;
mod proxy;
pub mod python;
pub mod storage;

pub type Shared<T> = Arc<Atom<Box<T>>>;

pub enum CrawlerReport {
    Report(CrawlerState),
    Finished,
}

impl From<CrawlerState> for CrawlerReport {
    fn from(value: CrawlerState) -> Self {
        Self::Report(value)
    }
}

pub mod prelude {

    pub type Result<T> = anyhow::Result<T>;
    pub type StdResult<T, E> = std::result::Result<T, E>;
    use std::path::PathBuf;

    pub use log::{debug, error, info, trace, warn};

    use crate::PageTypeId;

    #[derive(Debug, thiserror::Error)]
    pub enum AppError {
        #[error("Page #{} not found", .0)]
        PageNotFound(i64),

        #[error("Loading proxy list: {}", .0.display())]
        LoadingProxyList(PathBuf),

        #[error("Page parser for type id {} not found", .0)]
        PageParserNotFound(PageTypeId),

        #[error("Unable to create parser from file {}", .0.display())]
        UnableToCreateParser(PathBuf),

        #[error("Reading config {}", .0.display())]
        ReadingConfig(PathBuf),

        #[error("Opening database")]
        OpeningDatabase,

        #[error("Loading python parsers")]
        LoadingPythonParsers,

        #[error("Parser for page type {} failed", .0)]
        PageParserFailed(PageTypeId),
    }
}

pub type PageTypeId = u8;
pub type ParsedTable = Vec<HashMap<String, String>>;
pub type ParsedTables = HashMap<String, ParsedTable>;

#[derive(Deserialize, Serialize)]
pub struct CrawlerConfig {
    /// number of threads
    pub(crate) threads: usize,

    /// delay between requests in one thread
    pub(crate) delay_sec: f32,

    pub(crate) read_timeout_sec: Option<f32>,

    pub(crate) connect_timeout_sec: Option<f32>,

    /// path to proxies list
    pub(crate) proxies: Option<PathBuf>,
}

#[derive(Deserialize, Serialize)]
pub struct CrabConfig {
    pub database: PathBuf,
    pub crawler: CrawlerConfig,
}

impl CrabConfig {
    /// Returns config for a new workspace
    ///
    /// This method doesn't use [`Default`] trait intentionally.
    pub fn default_config() -> Self {
        Self {
            database: PathBuf::from("./db.sqlite"),
            crawler: CrawlerConfig {
                threads: 1,
                delay_sec: 5.,
                read_timeout_sec: Some(10.),
                connect_timeout_sec: Some(10.),
                proxies: None,
            },
        }
    }
}

/// Base type allowing user to provide parsing rules
pub trait PageParser {
    /// Parse next pages referenced in the content
    fn navigate(&self, content: &str) -> Result<Option<Vec<(String, PageTypeId)>>>;

    /// Returns parsed key-value pairs for the page]
    fn parse(&self, content: &str) -> Result<Option<ParsedTables>>;

    /// Validates page content
    ///
    /// If page is not valid it's content will not be written to storage
    /// and crawler will repeat request to the page
    fn validate(&self, _content: &str) -> Result<bool> {
        Ok(true)
    }

    fn page_type_id(&self) -> PageTypeId;
}

pub struct PageParsers(pub Vec<Box<dyn PageParser>>);

impl PageParsers {
    pub fn navigate(&self, page: &Page, content: &str) -> Result<Option<Vec<(Url, PageTypeId)>>> {
        let urls = page_parser(&self.0[..], page.type_id)?
            .navigate(content)
            .context(AppError::PageParserFailed(page.type_id))?;
        Ok(urls.map(|urls| create_absolute_urls(urls, &page.url)))
    }

    /// Returns parsed key-value pairs for the page
    pub fn parse(&self, type_id: PageTypeId, content: &str) -> Result<Option<ParsedTables>> {
        page_parser(&self.0[..], type_id)?
            .parse(content)
            .context(AppError::PageParserFailed(type_id))
    }

    /// Validates page content
    ///
    /// If page is not valid it's content will not be written to storage
    /// and crawler will repeat request to the page
    pub fn validate(&self, type_id: PageTypeId, content: &str) -> Result<bool> {
        let is_valid = page_parser(&self.0[..], type_id)?
            .validate(content)
            .context(AppError::PageParserFailed(type_id))?;
        Ok(is_valid)
    }
}

fn page_parser(parsers: &[Box<dyn PageParser>], type_id: PageTypeId) -> Result<&dyn PageParser> {
    parsers
        .iter()
        .find(|p| p.page_type_id() == type_id)
        .map(Box::as_ref)
        .ok_or_else(|| AppError::PageParserNotFound(type_id).into())
}

fn create_absolute_urls(
    input: Vec<(String, PageTypeId)>,
    base_url: &Url,
) -> Vec<(Url, PageTypeId)> {
    input
        .into_iter()
        .filter_map(|link| create_absolute_url(link, base_url))
        .collect()
}

fn create_absolute_url(item: (String, PageTypeId), base_url: &Url) -> Option<(Url, PageTypeId)> {
    let (url, type_id) = item;
    let absolute_url = if url.starts_with("http://") || url.starts_with("https://") {
        Url::parse(&url)
    } else {
        base_url.join(&url)
    };
    match absolute_url {
        Ok(url) => Some((url, type_id)),
        Err(e) => {
            warn!(
                "Unable to build absolute URL from: {}, base url: {}",
                url, base_url
            );
            debug!("{}", e);
            None
        }
    }
}
