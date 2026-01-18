//! Calendar polling background task (placeholder).
//!
//! This module provides the structure for calendar polling but is currently
//! a stub. The actual Google Calendar API integration is not yet implemented.
//!
//! TODO: Implement Google Calendar API integration similar to gmail_client.rs
//!
//! When implemented, this should:
//! 1. Use the same OAuth2 flow as Gmail
//! 2. Fetch upcoming calendar events
//! 3. Create AgentDecisions for events that might need todo items
//! 4. Store calendar events in the calendar_events table

use crate::db::DbPool;
use std::time::Duration;

/// Configuration for the calendar polling task
#[derive(Debug, Clone)]
pub struct CalendarPollerConfig {
    /// How often to poll for calendar events (default: 15 minutes)
    pub poll_interval: Duration,
    /// How many days ahead to look for events
    pub days_ahead: u32,
}

impl Default for CalendarPollerConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(900), // 15 minutes
            days_ahead: 7,
        }
    }
}

impl CalendarPollerConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let poll_interval_secs = std::env::var("CALENDAR_POLL_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(900);

        let days_ahead = std::env::var("CALENDAR_DAYS_AHEAD")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(7);

        Self {
            poll_interval: Duration::from_secs(poll_interval_secs),
            days_ahead,
        }
    }
}

/// Start the calendar polling background task.
///
/// Currently this is a placeholder that logs periodically but does not
/// actually poll any calendars. The Google Calendar API integration
/// is not yet implemented.
pub async fn start_calendar_polling_task(_pool: DbPool) {
    let config = CalendarPollerConfig::from_env();

    tracing::info!(
        "Calendar polling task started (interval: {:?}, days ahead: {}) [STUB - not implemented]",
        config.poll_interval,
        config.days_ahead
    );

    loop {
        tracing::debug!("Calendar poll tick (no-op - integration not implemented)");
        tokio::time::sleep(config.poll_interval).await;
    }
}
