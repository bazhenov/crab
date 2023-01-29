use crab::{
    entrypoint,
    prelude::*,
    utils::{url_set_query_param, Form},
    Navigator, Page,
};
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
    static ref FORM: Selector = Selector::parse("form#form[action]:not([action=''])").unwrap();
    static ref FIELDS: Selector = Selector::parse("select").unwrap();
    static ref FIELD_VALUE: Selector = Selector::parse("option[value]:not([value=''])").unwrap();
    static ref ALLOWED_FILTERS: Vec<&'static str> = vec!["mfgr", "released"];
}

impl Navigator for CpuDatabase {
    fn next_pages(page: &Page, content: &str) -> Result<Vec<Url>> {
        let document = Html::parse_document(content);

        let mut links = read_cpu_links(&document, page)?;
        links.extend(read_form_links(document, page));

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

fn read_cpu_links(document: &Html, page: &Page) -> Result<Vec<Url>> {
    let mut links = vec![];
    for f in document.select(&LINK_SELECTOR) {
        if let Some(link) = f.value().attr("href") {
            if link.starts_with("/cpu-specs/") {
                links.push(page.url.join(link)?);
            }
        }
    }
    Ok(links)
}

fn read_form_links(document: Html, page: &Page) -> Vec<Url> {
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
                    links.push(url);
                }
            }
        }
    }
    links
}
