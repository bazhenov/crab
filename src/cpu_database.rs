use crab::{prelude::*, Link, Navigator};
use scraper::{Html, Selector};
pub(crate) struct CpuDatabase;

impl Navigator for CpuDatabase {
    fn next_pages(content: &str) -> Result<Vec<Link>> {
        let document = Html::parse_document(content);

        let selector =
            Selector::parse("table.processors td a").map_err(|_e| Error::InvalidSelector)?;

        let mut links = vec![];
        for f in document.select(&selector) {
            if let Some(link) = f.value().attr("href") {
                links.push(link.to_owned());
            }
        }
        Ok(links)
    }
}
