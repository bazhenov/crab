use crate::{
    cli::RunCrawlerOpts,
    prelude::*,
    proxy::Proxies,
    storage::{Page, Storage},
    Navigator,
};
use anyhow::Context;
use atom::Atom;
use futures::{stream::FuturesUnordered, StreamExt};
use reqwest::{Client, Proxy, Url};
use std::{
    collections::HashSet,
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};
use tokio::time::sleep;

#[derive(Clone, Default)]
pub(crate) struct CrawlerState {
    /// Number of requests crawler initiated from the start of it's running
    pub(crate) requests: u32,
    /// Number of requests finished successfully
    pub(crate) successfull_requests: u32,
    /// Number of new links has been found
    pub(crate) new_links_found: u32,
    /// The set of ongoing requests
    pub(crate) requests_in_flight: HashSet<Page>,
}

pub(crate) async fn run_crawler<T: Navigator>(
    storage: Storage,
    opts: RunCrawlerOpts,
    report: (Arc<Atom<Box<CrawlerState>>>, Duration),
) -> Result<()> {
    let (report, report_tick) = report;
    let mut last_report_time = Instant::now();

    let mut state = CrawlerState::default();
    let delay = Duration::from_secs_f32(opts.timeout_sec);
    let mut futures = FuturesUnordered::new();
    let mut pages = vec![];
    let mut proxies = match opts.proxies_list {
        Some(path) => Proxies::from_file(&path).context(AppError::UnableToOpenProxyList(path))?,
        None => Proxies::default(),
    };

    report.swap(Box::new(state.clone()), Ordering::Relaxed);

    loop {
        // REPORTING PHASE
        if last_report_time.elapsed() >= report_tick {
            report.swap(Box::new(state.clone()), Ordering::Relaxed);
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
            let client = create_http_client(proxy)?;

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
                        let valid_page = T::validate(&content);
                        if valid_page {
                            state.successfull_requests += 1;
                            storage.write_page_content(page.id, &content).await?;

                            if opts.navigate {
                                navigate_page::<T>(&page, &content, &storage, &mut state).await?;
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

async fn navigate_page<T: Navigator>(
    page: &Page,
    content: &str,
    storage: &Storage,
    state: &mut CrawlerState,
) -> Result<()> {
    match T::next_pages(page, content) {
        Ok(links) => {
            for link in links {
                let page_id = storage.register_page(link, page.depth + 1).await?;
                if page_id.is_some() {
                    state.new_links_found += 1;
                }
            }
        }
        Err(e) => error!("next_pages() method failed on page #{}: {}", page.id, e),
    }
    Ok(())
}

fn create_http_client(proxy: Option<Proxy>) -> Result<Client> {
    let mut builder = Client::builder();
    if let Some(proxy) = proxy {
        builder = builder.proxy(proxy);
    }
    let client = builder
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
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
