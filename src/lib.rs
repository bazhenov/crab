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
    }
}

/// Base type allowing user to provide parsing rules
pub trait Navigator {
    /// Parse next pages referenced in the content
    fn next_pages(page: &Page, content: &str) -> Result<Vec<Url>>;

    /// Returns parsed key-value pairs for the page]
    fn kv(content: &str) -> Result<HashMap<String, String>>;

    /// Validates page content
    ///
    /// If page is not valid it's content will not be written to storage
    /// and crawler will repeat request to the page
    fn validate(_content: &str) -> bool {
        true
    }
}
