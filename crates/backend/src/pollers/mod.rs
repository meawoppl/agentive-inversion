//! Background polling tasks for email and calendar integration.
//!
//! This module consolidates polling functionality into the backend,
//! running as tokio background tasks rather than separate processes.

pub mod calendar;
pub mod email;
mod gmail_client;
mod processor;

pub use calendar::start_calendar_polling_task;
pub use email::start_email_polling_task;
