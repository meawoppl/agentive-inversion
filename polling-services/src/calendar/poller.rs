use anyhow::Result;

use crate::config::PollingConfig;
use crate::db::DbPool;

pub struct CalendarPoller {
    _pool: DbPool,
    _config: PollingConfig,
}

impl CalendarPoller {
    pub fn new(pool: DbPool, config: PollingConfig) -> Self {
        Self {
            _pool: pool,
            _config: config,
        }
    }

    pub async fn poll(&self) -> Result<()> {
        tracing::debug!("Calendar polling not yet implemented");

        // TODO: Implement Calendar polling
        // 1. Fetch all enabled Calendar sources from database
        // 2. For each source:
        //    - Authenticate with Google Calendar API
        //    - Fetch upcoming events
        //    - Create/update todos for events
        //    - Update last_polled_at timestamp

        Ok(())
    }
}
