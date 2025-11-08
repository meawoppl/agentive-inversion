use anyhow::{Context, Result};
use std::env;

#[derive(Debug, Clone)]
pub struct PollingConfig {
    pub database_url: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub gmail_polling_interval_seconds: u64,
    pub calendar_polling_interval_seconds: u64,
    pub encryption_key: String,
}

impl PollingConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: env::var("DATABASE_URL")
                .context("DATABASE_URL must be set")?,
            google_client_id: env::var("GOOGLE_CLIENT_ID")
                .context("GOOGLE_CLIENT_ID must be set")?,
            google_client_secret: env::var("GOOGLE_CLIENT_SECRET")
                .context("GOOGLE_CLIENT_SECRET must be set")?,
            gmail_polling_interval_seconds: env::var("GMAIL_POLLING_INTERVAL_SECONDS")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .context("GMAIL_POLLING_INTERVAL_SECONDS must be a valid number")?,
            calendar_polling_interval_seconds: env::var("CALENDAR_POLLING_INTERVAL_SECONDS")
                .unwrap_or_else(|_| "600".to_string())
                .parse()
                .context("CALENDAR_POLLING_INTERVAL_SECONDS must be a valid number")?,
            encryption_key: env::var("ENCRYPTION_KEY")
                .unwrap_or_else(|_| "development-key-change-in-production".to_string()),
        })
    }
}
