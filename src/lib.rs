use prelude::*;
use std::collections::HashMap;
use storage::Page;
use url::Url;

pub mod storage;
pub mod table;

pub mod prelude {

    pub type Result<T> = anyhow::Result<T>;
    pub type StdResult<T, E> = std::result::Result<T, E>;
    pub use log::{debug, error, info, trace, warn};

    #[derive(Debug, thiserror::Error)]
    pub enum AppError {
        #[error("Invalid Selector")]
        InvalidSelector,

        #[error("Page #{} not found", .0)]
        PageNotFound(i64),
    }
}

pub type Link = String;
pub trait Navigator {
    fn next_pages(page: &Page, content: &str) -> Result<Vec<Url>>;

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
