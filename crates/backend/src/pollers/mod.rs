//! Background polling tasks for email and calendar integration.
//!
//! This module consolidates the email-poller functionality into the backend,
//! running as tokio background tasks rather than separate processes.

pub mod email;
mod gmail_client;
mod processor;

pub use email::start_email_polling_task;
