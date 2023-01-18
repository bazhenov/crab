use lazy_static::lazy_static;
use reqwest::Url;
use scraper::{ElementRef, Selector};

lazy_static! {
    static ref SELECT: Selector = Selector::parse("select[name]").unwrap();
    static ref OPTION: Selector = Selector::parse("option[selected]").unwrap();
}

struct Form<'a> {
    root: ElementRef<'a>,
    base_url: Url,
}

impl<'a> Form<'a> {
    fn new(base_url: Url, root: ElementRef<'a>) -> Self {
        Self { root, base_url }
    }

    fn action(&self) -> Option<Url> {
        if !self.root.value().name().eq_ignore_ascii_case("form") {
            return None;
        }

        let action = self.root.value().attr("action").unwrap_or_default();
        let mut url = match self.base_url.join(action).ok() {
            Some(url) => url,
            None => return None,
        };

        for select_tag in self.root.select(&SELECT) {
            let name = select_tag.value().attr("name").unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            if let Some(selected_option_tag) = select_tag.select(&OPTION).next() {
                let value = selected_option_tag
                    .value()
                    .attr("value")
                    .unwrap_or_default();
                if !value.is_empty() {
                    let mut query = url.query_pairs_mut();
                    query.append_pair(name, value);
                }
            }
        }
        Some(url)
    }
}

#[cfg(test)]
mod tests {
    use scraper::Html;
    use std::str::FromStr;

    use super::*;

    #[test]
    fn check_build_form_url() {
        let html = r#"<form action="/form">
                <select name="a">
                    <option value="1" selected>1</option>
                    <option value="2">2</option>
                </select>
                <select name="b">
                    <option value="3" selected>3</option>
                    <option value="4">4</option>
                </select>
                <select name="c">
                    <option value>1</option>
                </select>
            </form>"#;

        let html = Html::parse_document(html);
        let base_url = Url::from_str("http://server.com/").unwrap();
        let form = html
            .select(&Selector::parse("form").unwrap())
            .next()
            .unwrap();
        let form = Form::new(base_url.clone(), form);

        let url = form.action();
        assert_eq!(base_url.join("/form?a=1&b=3").ok(), url)
    }
}
