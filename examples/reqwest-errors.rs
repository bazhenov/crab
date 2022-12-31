use std::time::Duration;

use crab::prelude::*;
use reqwest::{Client, Proxy};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let client = Client::builder()
        .proxy(Proxy::https("socks5://proxy:4145/")?)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .build()?;
    let response = client.get("https://api.ipify.org?format=json").send().await;

    match response {
        Ok(r) => println!("Request completed: {}", r.text().await?),
        Err(e) => {
            error!("Failed: {}", e);
            warn!(" - e.is_body(): {}", e.is_body());
            warn!(" - e.is_builder(): {}", e.is_builder());
            warn!(" - e.is_connect(): {}", e.is_connect());
            warn!(" - e.is_decode(): {}", e.is_decode());
            warn!(" - e.is_redirect(): {}", e.is_redirect());
            warn!(" - e.is_request(): {}", e.is_request());
            warn!(" - e.is_status(): {}", e.is_status());
            warn!(" - e.is_timeout(): {}", e.is_timeout());
        }
    }
    Ok(())
}
