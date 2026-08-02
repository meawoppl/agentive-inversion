//! Background polling tasks for email and calendar integration.
//!
//! This module consolidates polling functionality into the backend,
//! running as tokio background tasks rather than separate processes.

pub mod calendar;
pub mod email;
pub mod gmail_client;
mod processor;
pub mod triage;

pub use calendar::start_calendar_polling_task;
pub use email::start_email_polling_task;
pub use triage::{start_triage_task, TriageHealth, TriageHealthState};
