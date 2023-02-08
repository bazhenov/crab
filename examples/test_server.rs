use crab::{entrypoint, prelude::*, Page, PageParser, PageType};
use lazy_static::lazy_static;
use scraper::{Html, Selector};
use std::collections::HashMap;
use url::Url;

#[tokio::main]
async fn main() -> Result<()> {
    entrypoint(vec![Box::new(ListingPage), Box::new(DataPage)]).await
}

lazy_static! {
    static ref PAGER_LINK: Selector = Selector::parse("section.pager a").unwrap();
    static ref DATA_PAGE_LINK: Selector = Selector::parse("ul a").unwrap();
    static ref INPUT: Selector = Selector::parse(".input").unwrap();
    static ref OUTPUT: Selector = Selector::parse(".output").unwrap();
}

struct ListingPage;

impl ListingPage {
    const TYPE: PageType = 1;
}

impl PageParser for ListingPage {
    fn next_pages(&self, page: &Page, content: &str) -> Result<Option<Vec<(Url, PageType)>>> {
        let document = Html::parse_document(content);

        let mut links = vec![];

        for f in document.select(&PAGER_LINK) {
            if let Some(link) = f.value().attr("href") {
                links.push((page.url.join(link)?, ListingPage::TYPE));
            }
        }

        for f in document.select(&DATA_PAGE_LINK) {
            if let Some(link) = f.value().attr("href") {
                links.push((page.url.join(link)?, DataPage::TYPE));
            }
        }
        Ok(Some(links))
    }

    fn kv(&self, _: &str) -> Result<Option<HashMap<String, String>>> {
        Ok(None)
    }

    fn page_type(&self) -> PageType {
        Self::TYPE
    }
}

struct DataPage;

impl DataPage {
    const TYPE: PageType = 2;
}

impl PageParser for DataPage {
    fn next_pages(&self, _: &Page, _: &str) -> Result<Option<Vec<(Url, PageType)>>> {
        Ok(None)
    }

    fn kv(&self, content: &str) -> Result<Option<HashMap<String, String>>> {
        let document = Html::parse_document(content);
        let mut kv = HashMap::new();

        let input = document.select(&INPUT).next();
        let output = document.select(&OUTPUT).next();

        if let Some((input, output)) = input.zip(output) {
            kv.insert("input".to_owned(), input.inner_html());
            kv.insert("output".to_owned(), output.inner_html());
        }
        Ok(Some(kv))
    }

    fn page_type(&self) -> PageType {
        Self::TYPE
    }
}
