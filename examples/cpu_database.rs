use crab::{
    entrypoint,
    prelude::*,
    utils::{url_set_query_param, Form},
    Page, TargetPage,
};
use lazy_static::lazy_static;
use regex::Regex;
use scraper::{Html, Selector};
use std::collections::HashMap;
use url::Url;

#[tokio::main]
async fn main() -> Result<()> {
    entrypoint(vec![Box::new(ListingPage), Box::new(CpuPage)]).await
}

lazy_static! {
    static ref TD_SELECTOR: Selector = Selector::parse("td").unwrap();
    static ref TH_SELECTOR: Selector = Selector::parse("th").unwrap();
    static ref NAME_SELECTOR: Selector = Selector::parse("h1.cpuname").unwrap();
    static ref ROW_SELECTOR: Selector = Selector::parse(".details table tr").unwrap();
    static ref LINK_SELECTOR: Selector = Selector::parse("a").unwrap();
    static ref FORM: Selector = Selector::parse("form#form[action]:not([action=''])").unwrap();
    static ref FIELDS: Selector = Selector::parse("select").unwrap();
    static ref FIELD_VALUE: Selector = Selector::parse("option[value]:not([value=''])").unwrap();
    static ref ALLOWED_FILTERS: Vec<&'static str> = vec!["mfgr", "released"];
    static ref CPU_PAGE_LINK: Regex = Regex::new(r"^/cpu-specs/[^?]+\.c[0-9]+$").unwrap();
    static ref CPU_LISTING_LINK: Regex = Regex::new(r"^/cpu-specs/\?.+$").unwrap();
}

struct ListingPage;

impl ListingPage {
    const TYPE: u8 = 1;
}

impl TargetPage for ListingPage {
    fn next_pages(&self, page: &Page, content: &str) -> Result<Option<Vec<(Url, u8)>>> {
        let document = Html::parse_document(content);

        let mut links = read_links(&document, page)?;
        links.extend(read_form_links(document, page));

        Ok(Some(links))
    }

    fn kv(&self, _: &str) -> Result<Option<HashMap<String, String>>> {
        Ok(None)
    }

    fn validate(&self, content: &str) -> bool {
        validate_page(content)
    }

    fn page_type(&self) -> u8 {
        Self::TYPE
    }
}

struct CpuPage;

impl CpuPage {
    const TYPE: u8 = 2;
}

impl TargetPage for CpuPage {
    fn next_pages(&self, page: &Page, content: &str) -> Result<Option<Vec<(Url, u8)>>> {
        let document = Html::parse_document(content);
        Ok(Some(read_links(&document, page)?))
    }

    fn kv(&self, content: &str) -> Result<Option<HashMap<String, String>>> {
        let document = Html::parse_document(content);
        let mut kv = HashMap::new();

        if let Some(name) = document.select(&NAME_SELECTOR).next() {
            kv.insert("name".to_owned(), name.inner_html());
        } else {
            return Ok(Some(kv));
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
        Ok(Some(kv))
    }

    fn page_type(&self) -> u8 {
        Self::TYPE
    }

    fn validate(&self, content: &str) -> bool {
        validate_page(content)
    }
}

fn validate_page(content: &str) -> bool {
    !content.contains("captcha")
}

fn read_links(document: &Html, page: &Page) -> Result<Vec<(Url, u8)>> {
    let mut links = vec![];
    for f in document.select(&LINK_SELECTOR) {
        if let Some(link) = f.value().attr("href") {
            let page_type = if CPU_PAGE_LINK.is_match(&link) {
                CpuPage::TYPE
            } else if CPU_LISTING_LINK.is_match(&link) {
                ListingPage::TYPE
            } else {
                continue;
            };
            links.push((page.url.join(link)?, page_type));
        }
    }
    Ok(links)
}

fn read_form_links(document: Html, page: &Page) -> Vec<(Url, u8)> {
    let mut links = vec![];
    if let Some(form) = document.select(&FORM).next() {
        let fields = form.select(&FIELDS);
        let form = Form::new(&page.url, form);
        if let Some(form_url) = form.action() {
            for select in fields {
                let field_name = select.value().attr("name").unwrap_or_default();
                if field_name.is_empty() || !ALLOWED_FILTERS.contains(&field_name) {
                    continue;
                }
                for field_value in select.select(&FIELD_VALUE) {
                    let field_value = field_value.value().attr("value").unwrap_or_default();
                    if field_value.is_empty() {
                        continue;
                    }
                    let url = url_set_query_param(&form_url, field_name, field_value);
                    links.push((url, ListingPage::TYPE));
                }
            }
        }
    }
    links
}
