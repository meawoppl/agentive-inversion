use anyhow::Result;
use std::time::Duration;
use tokio::time;

use crate::config::PollingConfig;
use crate::db::DbPool;
use crate::gmail::GmailPoller;
use crate::calendar::CalendarPoller;

pub struct PollingScheduler {
    pool: DbPool,
    config: PollingConfig,
}

impl PollingScheduler {
    pub fn new(pool: DbPool, config: PollingConfig) -> Self {
        Self { pool, config }
    }

    pub async fn run(&self) -> Result<()> {
        tracing::info!("Starting polling scheduler");

        let gmail_pool = self.pool.clone();
        let gmail_config = self.config.clone();
        let gmail_handle = tokio::spawn(async move {
            Self::run_gmail_poller(gmail_pool, gmail_config).await
        });

        let calendar_pool = self.pool.clone();
        let calendar_config = self.config.clone();
        let calendar_handle = tokio::spawn(async move {
            Self::run_calendar_poller(calendar_pool, calendar_config).await
        });

        // Wait for either task to complete (which shouldn't happen unless there's an error)
        tokio::select! {
            result = gmail_handle => {
                if let Err(e) = result {
                    tracing::error!("Gmail poller task error: {:?}", e);
                }
            }
            result = calendar_handle => {
                if let Err(e) = result {
                    tracing::error!("Calendar poller task error: {:?}", e);
                }
            }
        }

        Ok(())
    }

    async fn run_gmail_poller(pool: DbPool, config: PollingConfig) -> Result<()> {
        let poller = GmailPoller::new(pool, config.clone());
        let interval = Duration::from_secs(config.gmail_polling_interval_seconds);
        let mut ticker = time::interval(interval);

        tracing::info!("Gmail poller started (interval: {:?})", interval);

        loop {
            ticker.tick().await;
            tracing::debug!("Running Gmail poll cycle");

            if let Err(e) = poller.poll().await {
                tracing::error!("Gmail polling error: {:?}", e);
                // Continue polling even on error
            }
        }
    }

    async fn run_calendar_poller(pool: DbPool, config: PollingConfig) -> Result<()> {
        let poller = CalendarPoller::new(pool, config.clone());
        let interval = Duration::from_secs(config.calendar_polling_interval_seconds);
        let mut ticker = time::interval(interval);

        tracing::info!("Calendar poller started (interval: {:?})", interval);

        loop {
            ticker.tick().await;
            tracing::debug!("Running Calendar poll cycle");

            if let Err(e) = poller.poll().await {
                tracing::error!("Calendar polling error: {:?}", e);
                // Continue polling even on error
            }
        }
    }
}
