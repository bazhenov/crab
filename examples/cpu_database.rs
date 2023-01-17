use crab::{entrypoint, normalize_url, prelude::*, Navigator, Page};
use lazy_static::lazy_static;
use scraper::{Html, Selector};
use std::collections::HashMap;
use url::Url;

#[tokio::main]
async fn main() -> Result<()> {
    entrypoint::<CpuDatabase>().await
}

struct CpuDatabase;

lazy_static! {
    static ref TD_SELECTOR: Selector = Selector::parse("td").unwrap();
    static ref TH_SELECTOR: Selector = Selector::parse("th").unwrap();
    static ref NAME_SELECTOR: Selector = Selector::parse("h1.cpuname").unwrap();
    static ref ROW_SELECTOR: Selector = Selector::parse(".details table tr").unwrap();
    static ref LINK_SELECTOR: Selector = Selector::parse("a").unwrap();
    static ref FORM_SELECTOR: Selector =
        Selector::parse("form#form[action]:not([action=''])").unwrap();
    static ref SELECT_SELECTOR: Selector = Selector::parse("select").unwrap();
    static ref OPTION_SELECTOR: Selector =
        Selector::parse("option[value]:not([value=''])").unwrap();
    static ref ALLOWED_FILTERS: Vec<&'static str> = vec!["mfgr", "released"];
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

        for form in document.select(&FORM_SELECTOR) {
            let form_url = form.value().attr("action").unwrap_or_default();
            if form_url.is_empty() {
                continue;
            }
            let url = normalize_url(&page.url, form_url)?;

            for select in form.select(&SELECT_SELECTOR) {
                let filter_name = select.value().attr("name").unwrap_or_default();
                if filter_name == "" || !ALLOWED_FILTERS.contains(&filter_name) {
                    continue;
                }
                for option in select.select(&OPTION_SELECTOR) {
                    let mut url = url.clone();
                    let value = option.value().attr("value").unwrap_or_default();
                    if value.is_empty() {
                        continue;
                    }
                    url.query_pairs_mut().append_pair(filter_name, value);
                    links.push(url);
                }
            }
        }

        Ok(links)
    }

    fn kv(content: &str) -> Result<HashMap<String, String>> {
        let document = Html::parse_document(content);
        let mut kv = HashMap::new();

        if let Some(name) = document.select(&NAME_SELECTOR).next() {
            kv.insert("name".to_owned(), name.inner_html());
        } else {
            return Ok(kv);
        }
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