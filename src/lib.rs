use anyhow::Context;
use prelude::*;
use std::collections::HashMap;
pub use storage::Page;
use url::Url;

pub mod crawler;
mod proxy;
pub mod python;
pub mod storage;

pub mod prelude {

    pub type Result<T> = anyhow::Result<T>;
    pub type StdResult<T, E> = std::result::Result<T, E>;
    use std::path::PathBuf;

    pub use log::{debug, error, info, trace, warn};

    use crate::PageType;

    #[derive(Debug, thiserror::Error)]
    pub enum AppError {
        #[error("Page #{} not found", .0)]
        PageNotFound(i64),

        #[error("Opening proxy list: {}", .0.display())]
        UnableToOpenProxyList(PathBuf),

        #[error("Handler for page type {} not found", .0)]
        PageHandlerNotFound(PageType),

        #[error("Unable to create parser from file {}", .0.display())]
        UnableToCreateParser(PathBuf),

        #[error("Parser for page type {} failed", .0)]
        PageParserFailed(PageType),
    }
}

pub type PageType = u8;

/// Base type allowing user to provide parsing rules
pub trait PageParser {
    /// Parse next pages referenced in the content
    fn next_pages(&self, page: &Page, content: &str) -> Result<Option<Vec<(Url, PageType)>>>;

    /// Returns parsed key-value pairs for the page]
    fn kv(&self, content: &str) -> Result<Option<HashMap<String, String>>>;

    /// Validates page content
    ///
    /// If page is not valid it's content will not be written to storage
    /// and crawler will repeat request to the page
    fn validate(&self, _content: &str) -> Result<bool> {
        Ok(true)
    }

    fn page_type(&self) -> PageType;
}

pub struct PageParsers(pub Vec<Box<dyn PageParser>>);

impl PageParsers {
    pub fn next_pages(&self, page: &Page, content: &str) -> Result<Option<Vec<(Url, PageType)>>> {
        let parser = page_parser(&self.0[..], page.page_type)?;
        parser
            .next_pages(page, content)
            .context(AppError::PageParserFailed(page.page_type))
    }

    /// Returns parsed key-value pairs for the page
    pub fn kv(
        &self,
        page_type: PageType,
        content: &str,
    ) -> Result<Option<HashMap<String, String>>> {
        let parser = page_parser(&self.0[..], page_type)?;
        parser
            .kv(content)
            .context(AppError::PageParserFailed(page_type))
    }

    /// Validates page content
    ///
    /// If page is not valid it's content will not be written to storage
    /// and crawler will repeat request to the page
    pub fn validate(&self, page_type: PageType, content: &str) -> Result<bool> {
        let parser = page_parser(&self.0[..], page_type)?;
        let is_valid = parser
            .validate(content)
            .context(AppError::PageParserFailed(page_type))?;
        Ok(is_valid)
    }
}

fn page_parser(handlers: &[Box<dyn PageParser>], page_type: PageType) -> Result<&dyn PageParser> {
    handlers
        .iter()
        .find(|h| h.page_type() == page_type)
        .map(Box::as_ref)
        .ok_or_else(|| AppError::PageHandlerNotFound(page_type).into())
}
