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

    /// Validates page content
    ///
    /// If page is not valid it's content will not be written to storage
    /// and crawler will repeat request to the page
    fn validate(content: &str) -> bool;

    /// Returns parsed key-value pairs for the page]
    fn kv(content: &str) -> Result<HashMap<String, String>>;
}

pub fn normalize_url(base_url: &Url, link: &str) -> Result<Url> {
    Ok(base_url.join(link)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_normalize_url() -> Result<()> {
        let url = Url::parse("http://server.com/foo/bar")?;
        assert_eq!(
            normalize_url(&url, "/bar")?.to_string(),
            "http://server.com/bar",
        );
        assert_eq!(
            normalize_url(&url, "./baz")?.to_string(),
            "http://server.com/foo/baz",
        );

        assert_eq!(
            normalize_url(&url, "../baz")?.to_string(),
            "http://server.com/baz",
        );
        Ok(())
    }

    #[test]
    fn check_normalize_url_get_params() -> Result<()> {
        let url = Url::parse("http://server.com/foo/bar")?;
        assert_eq!(
            normalize_url(&url, "/bar?b=1&a=2")?.to_string(),
            "http://server.com/bar?b=1&a=2",
        );
        Ok(())
    }
}
