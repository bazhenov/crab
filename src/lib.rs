pub use cli::entrypoint;
use prelude::*;
use std::collections::HashMap;
pub use storage::Page;
use url::Url;

pub(crate) mod cli;
pub(crate) mod crawler;
pub(crate) mod proxy;
pub mod storage;
pub(crate) mod table;
pub(crate) mod terminal;
pub mod utils;

pub mod prelude {

    pub type Result<T> = anyhow::Result<T>;
    pub type StdResult<T, E> = std::result::Result<T, E>;
    use std::path::PathBuf;

    pub use log::{debug, error, info, trace, warn};

    #[derive(Debug, thiserror::Error)]
    pub enum AppError {
        #[error("Invalid Selector")]
        InvalidSelector,

        #[error("Page #{} not found", .0)]
        PageNotFound(i64),

        #[error("Opening proxy list: {}", .0.display())]
        UnableToOpenProxyList(PathBuf),

        #[error("Handler for page type {} not found", .0)]
        PageHandlerNotFound(u8),
    }
}

/// Base type allowing user to provide parsing rules
pub trait TargetPage {
    /// Parse next pages referenced in the content
    fn next_pages(&self, page: &Page, content: &str) -> Result<Option<Vec<(Url, u8)>>>;

    /// Returns parsed key-value pairs for the page]
    fn kv(&self, content: &str) -> Result<Option<HashMap<String, String>>>;

    /// Validates page content
    ///
    /// If page is not valid it's content will not be written to storage
    /// and crawler will repeat request to the page
    fn validate(&self, _content: &str) -> bool {
        true
    }

    fn page_type(&self) -> u8;
}

pub(crate) fn page_handler(
    handlers: &[Box<dyn TargetPage>],
    page_type: u8,
) -> Option<&dyn TargetPage> {
    handlers
        .iter()
        .find(|h| h.page_type() == page_type)
        .map(Box::as_ref)
}
