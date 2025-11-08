use anyhow::{Context, Result};
use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub port: u16,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_redirect_uri: String,
    pub encryption_key: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: env::var("DATABASE_URL")
                .context("DATABASE_URL must be set")?,
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .context("PORT must be a valid number")?,
            google_client_id: env::var("GOOGLE_CLIENT_ID")
                .context("GOOGLE_CLIENT_ID must be set")?,
            google_client_secret: env::var("GOOGLE_CLIENT_SECRET")
                .context("GOOGLE_CLIENT_SECRET must be set")?,
            google_redirect_uri: env::var("GOOGLE_REDIRECT_URI")
                .context("GOOGLE_REDIRECT_URI must be set")?,
            encryption_key: env::var("ENCRYPTION_KEY")
                .unwrap_or_else(|_| "development-key-change-in-production".to_string()),
        })
    }
}
