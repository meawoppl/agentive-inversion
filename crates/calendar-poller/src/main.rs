use anyhow::Result;

mod calendar_client;
mod processor;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    tracing::warn!("Calendar poller is not yet implemented");
    tracing::warn!("This service will exit immediately. See issue #26 for implementation status.");

    Ok(())
}
