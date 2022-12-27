use std::collections::HashMap;

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

    fn kv(content: &str) -> Result<HashMap<String, String>> {
        let document = Html::parse_document(content);
        let row_selector =
            Selector::parse(".details table tr").map_err(|_e| Error::InvalidSelector)?;
        let th_selector = Selector::parse("th").map_err(|_e| Error::InvalidSelector)?;
        let td_selector = Selector::parse("td").map_err(|_e| Error::InvalidSelector)?;

        let mut kv = HashMap::new();
        for f in document.select(&row_selector) {
            let th = f.select(&th_selector).next();
            let td = f.select(&td_selector).next();

            if let Some((key, value)) = th.zip(td) {
                let key = key.inner_html();
                let value = value.inner_html();
                let key = key.trim().trim_end_matches(":");
                let value = value.trim();
                kv.insert(key.to_owned(), value.to_owned());
            }
        }
        Ok(kv)
    }
}
