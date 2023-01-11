use crab::{normalize_url, prelude::*, storage::Page, Navigator};
use lazy_static::lazy_static;
use scraper::{Html, Selector};
use std::collections::HashMap;
use url::Url;

pub(crate) struct CpuDatabase;

lazy_static! {
    static ref TD_SELECTOR: Selector = Selector::parse("td").unwrap();
    static ref TH_SELECTOR: Selector = Selector::parse("th").unwrap();
    static ref ROW_SELECTOR: Selector = Selector::parse(".details table tr").unwrap();
    static ref LINK_SELECTOR: Selector = Selector::parse("a").unwrap();
}

impl Navigator for CpuDatabase {
    fn next_pages(page: &Page, content: &str) -> Result<Vec<Url>> {
        let document = Html::parse_document(content);

        let mut links = vec![];
        for f in document.select(&LINK_SELECTOR) {
            if let Some(link) = f.value().attr("href") {
                if link.starts_with("/cpu-specs/") {
                    links.push(normalize_url(&page.url, link)?);
                }
            }
        }
        Ok(links)
    }

    fn kv(content: &str) -> Result<HashMap<String, String>> {
        let document = Html::parse_document(content);
        let mut kv = HashMap::new();
        for f in document.select(&ROW_SELECTOR) {
            let th = f.select(&TH_SELECTOR).next();
            let td = f.select(&TD_SELECTOR).next();

            if let Some((key, value)) = th.zip(td) {
                let key = key.inner_html();
                let value = value.inner_html();
                let key = key.trim().trim_end_matches(':');
                let value = value.trim();
                kv.insert(key.to_owned(), value.to_owned());
            }
        }
        Ok(kv)
    }

    fn validate(content: &str) -> bool {
        !content.contains("captcha")
    }
}
