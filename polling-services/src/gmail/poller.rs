use anyhow::Result;

use crate::config::PollingConfig;
use crate::db::DbPool;

pub struct GmailPoller {
    _pool: DbPool,
    _config: PollingConfig,
}

impl GmailPoller {
    pub fn new(pool: DbPool, config: PollingConfig) -> Self {
        Self {
            _pool: pool,
            _config: config,
        }
    }

    pub async fn poll(&self) -> Result<()> {
        tracing::debug!("Gmail polling not yet implemented");

        // TODO: Implement Gmail polling
        // 1. Fetch all enabled Gmail sources from database
        // 2. For each source:
        //    - Authenticate with Gmail API
        //    - Fetch new emails since last_polled_at
        //    - Parse emails for actionable items
        //    - Create/update todos in database
        //    - Update last_polled_at timestamp

        Ok(())
    }
}
