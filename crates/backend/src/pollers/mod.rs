//! Gmail ingestion remains hosted; agent execution belongs to Portal.
pub mod email;
pub mod gmail_client;
pub mod gmail_drafts;
use chrono::{DateTime, Utc};
pub use email::start_email_polling_task;
use std::sync::Arc;
use tokio::sync::RwLock;
#[derive(Clone)]
pub struct TriageHealthState {
    pub mode: String,
    pub last_cycle_at: Option<DateTime<Utc>>,
    pub next_cycle_at: Option<DateTime<Utc>>,
    pub last_cycle_error: Option<String>,
    pub consecutive_failures: i32,
}
impl Default for TriageHealthState {
    fn default() -> Self {
        Self {
            mode: "external_portal".into(),
            last_cycle_at: None,
            next_cycle_at: None,
            last_cycle_error: None,
            consecutive_failures: 0,
        }
    }
}
pub type TriageHealth = Arc<RwLock<TriageHealthState>>;
