use crab::{entrypoint, prelude::*, Navigator, Page};
use scraper::{Html, Selector};
use std::collections::HashMap;
use url::Url;

#[tokio::main]
async fn main() -> Result<()> {
    entrypoint::<TestServer>().await
}

struct TestServer;

impl Navigator for TestServer {
    fn next_pages(page: &Page, content: &str) -> Result<Vec<Url>> {
        let document = Html::parse_document(content);

        let mut links = vec![];

        let selector =
            Selector::parse("section.pager a").map_err(|_e| AppError::InvalidSelector)?;
        for f in document.select(&selector) {
            if let Some(link) = f.value().attr("href") {
                links.push(page.url.join(link)?);
            }
        }

        let selector = Selector::parse("ul a").map_err(|_e| AppError::InvalidSelector)?;
        for f in document.select(&selector) {
            if let Some(link) = f.value().attr("href") {
                links.push(page.url.join(link)?);
            }
        }
        Ok(links)
    }

    fn kv(content: &str) -> Result<HashMap<String, String>> {
        let document = Html::parse_document(content);
        let input_selector = Selector::parse(".input").map_err(|_e| AppError::InvalidSelector)?;
        let output_selector = Selector::parse(".output").map_err(|_e| AppError::InvalidSelector)?;

        let mut kv = HashMap::new();
        let input = document.select(&input_selector).next();
        let output = document.select(&output_selector).next();

        if let Some((input, output)) = input.zip(output) {
            kv.insert("input".to_owned(), input.inner_html());
            kv.insert("output".to_owned(), output.inner_html());
        }
        Ok(kv)
    }
}
