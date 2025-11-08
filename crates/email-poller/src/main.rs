use anyhow::Result;
use tokio::time::{interval, Duration};

mod gmail_client;
mod processor;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    tracing::info!("Starting email poller service");

    let mut interval = interval(Duration::from_secs(300));

    loop {
        interval.tick().await;

        if let Err(e) = poll_emails().await {
            tracing::error!("Error polling emails: {}", e);
        }
    }
}

async fn poll_emails() -> Result<()> {
    tracing::info!("Polling emails...");
    Ok(())
}
