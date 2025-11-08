mod gmail;
mod calendar;
mod scheduler;
mod config;
mod db;

use anyhow::Result;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::PollingConfig;
use crate::db::DbPool;
use crate::scheduler::PollingScheduler;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "polling_services=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Agentive Inversion polling services");

    // Load configuration
    dotenv::dotenv().ok();
    let config = PollingConfig::from_env()?;

    // Initialize database pool
    let pool = DbPool::new(&config.database_url).await?;
    tracing::info!("Database connection pool initialized");

    // Create scheduler
    let scheduler = PollingScheduler::new(pool, config);

    // Start polling tasks
    let scheduler_handle = tokio::spawn(async move {
        if let Err(e) = scheduler.run().await {
            tracing::error!("Scheduler error: {:?}", e);
        }
    });

    // Wait for shutdown signal
    tracing::info!("Polling services running. Press Ctrl+C to stop.");
    signal::ctrl_c().await?;
    tracing::info!("Shutdown signal received, stopping...");

    // Graceful shutdown
    scheduler_handle.abort();

    tracing::info!("Polling services stopped");
    Ok(())
}
