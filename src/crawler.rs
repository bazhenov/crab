use crate::{
    prelude::*,
    proxy::{Proxies, ProxyStat},
    storage::{Page, Storage},
    CrawlerConfig, CrawlerReport, PageParsers, Shared,
};
use anyhow::Context;
use futures::{stream::FuturesUnordered, StreamExt};
use reqwest::{Client, Proxy, Url};
use std::{
    collections::HashSet,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use tokio::time::sleep;

#[derive(Clone, Default)]
pub struct CrawlerState {
    /// Number of requests crawler initiated from the start of it's running
    pub requests: u32,
    /// Number of requests finished successfully
    pub successfull_requests: u32,
    /// Number of new links has been found
    pub new_links_found: u32,
    /// The set of ongoing requests
    pub requests_in_flight: HashSet<Page>,

    pub proxies: Vec<(Proxy, ProxyStat)>,
}

pub async fn run_crawler(
    parsers: PageParsers,
    mut storage: Storage,
    opts: CrawlerConfig,
    navigate: bool,
    report: (Shared<CrawlerReport>, Duration),
) -> Result<()> {
    let (report, report_tick) = report;
    let mut last_report_time = Instant::now();

    let mut state = CrawlerState::default();
    let delay = Duration::from_secs_f32(opts.delay_sec);
    let mut futures = FuturesUnordered::new();
    let mut pages = vec![];
    let mut proxies = match &opts.proxies {
        Some(path) => {
            Proxies::from_file(&path).context(AppError::LoadingProxyList(path.clone()))?
        }
        None => Proxies::default(),
    };

    report.swap(Box::new(state.clone().into()), Ordering::Relaxed);

    loop {
        // REPORTING PHASE
        if last_report_time.elapsed() >= report_tick {
            let mut state = state.clone();
            state.proxies = proxies.stat();
            report.swap(Box::new(state.into()), Ordering::Relaxed);
            last_report_time = Instant::now();
        }

        // REFILLING PHASE
        if pages.is_empty() && futures.is_empty() {
            pages = storage.list_not_downloaded_pages(100).await?;
            if pages.is_empty() {
                break;
            }
        }

        // DISPATCHING PHASE
        while futures.len() < opts.threads && !pages.is_empty() {
            let next_page = pages.swap_remove(0);
            let next_proxy = proxies.next();
            let (proxy, proxy_id) = next_proxy.unzip();
            let client = create_http_client(&opts, proxy)?;

            state.requests += 1;
            state.requests_in_flight.insert(next_page.clone());

            let future = tokio::spawn(async move {
                let content = fetch_content(client, next_page.url.clone(), delay).await;
                (proxy_id, next_page, content)
            });
            futures.push(future);
        }

        // COMPLETING PHASE
        if !futures.is_empty() {
            if let Some(completed) = futures.next().await {
                let (proxy, page, response) = completed?;
                state.requests_in_flight.remove(&page);

                let success = match response {
                    Ok(content) => {
                        let valid_page = parsers.validate(page.type_id, &content)?;
                        if valid_page {
                            state.successfull_requests += 1;
                            storage.write_page_content(page.id, &content).await?;

                            if navigate {
                                navigate_page(&parsers, &page, &content, &mut storage, &mut state)
                                    .await?;
                            }
                        }

                        valid_page
                    }
                    Err(e) => {
                        debug!("Unable to download: {}", page.url);
                        trace!("{}", e);
                        false
                    }
                };

                if let Some(proxy) = proxy {
                    if success {
                        proxies.proxy_succeseed(proxy);
                    } else {
                        proxies.proxy_failed(proxy);
                    }
                }
            }
        }
    }
    Ok(())
}

async fn navigate_page(
    parsers: &PageParsers,
    page: &Page,
    content: &str,
    storage: &mut Storage,
    state: &mut CrawlerState,
) -> Result<()> {
    match parsers.navigate(page, content) {
        Ok(Some(links)) => {
            for (link, type_id) in links {
                let page_id = storage.register_page(link, type_id, page.depth + 1).await?;
                if page_id.is_some() {
                    state.new_links_found += 1;
                }
            }
        }
        Ok(None) => {}
        Err(e) => error!("next_pages() method failed on page #{}: {}", page.id, e),
    }
    Ok(())
}

fn create_http_client(opts: &CrawlerConfig, proxy: Option<Proxy>) -> Result<Client> {
    let mut builder = Client::builder();
    if let Some(proxy) = proxy {
        builder = builder.proxy(proxy);
    }
    let connect_timeout = opts.connect_timeout_sec.unwrap_or(5.0);
    let read_timeout = opts.read_timeout_sec.unwrap_or(5.0);
    let client = builder
        .connect_timeout(Duration::from_secs_f32(connect_timeout))
        .timeout(Duration::from_secs_f32(read_timeout))
        .danger_accept_invalid_certs(true)
        .build()?;
    Ok(client)
}

async fn fetch_content(client: Client, url: Url, delay: Duration) -> Result<String> {
    trace!("Starting: {}", &url);
    let instant = Instant::now();
    let response = download(client, url.as_ref()).await;
    if response.is_ok() {
        let duration = instant.elapsed();
        trace!("Downloaded in {:.1}s: {}", duration.as_secs_f32(), &url);
    }
    sleep(delay).await;
    response
}

async fn download(client: Client, url: &str) -> Result<String> {
    Ok(client.get(url).send().await?.text().await?)
}
