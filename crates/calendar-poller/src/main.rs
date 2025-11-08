use anyhow::Result;
use tokio::time::{interval, Duration};

mod calendar_client;
mod processor;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    tracing::info!("Starting calendar poller service");

    let mut interval = interval(Duration::from_secs(300));

    loop {
        interval.tick().await;

        if let Err(e) = poll_calendars().await {
            tracing::error!("Error polling calendars: {}", e);
        }
    }
}

async fn poll_calendars() -> Result<()> {
    tracing::info!("Polling calendars...");
    Ok(())
}
