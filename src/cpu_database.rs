use crab::{normalize_url, prelude::*, storage::Page, Navigator};
use scraper::{Html, Selector};
use url::Url;
pub(crate) struct CpuDatabase;

impl Navigator for CpuDatabase {
    fn next_pages(page: &Page, content: &str) -> Result<Vec<Url>> {
        let document = Html::parse_document(content);

        let selector =
            Selector::parse("table.processors td a").map_err(|_e| Error::InvalidSelector)?;

        let mut links = vec![];
        for f in document.select(&selector) {
            if let Some(link) = f.value().attr("href") {
                links.push(normalize_url(&page.url, link)?);
            }
        }
        Ok(links)
    }
}
