//! Email polling background task.
//!
//! This module runs as a tokio background task within the backend process,
//! periodically fetching new emails from configured Gmail accounts.

use super::gmail_client::{EmailMessage, GmailClient};
use super::processor;
use crate::db::{self, DbPool};
use crate::models::NewEmail;
use anyhow::{Context, Result};
use chrono::Utc;
use shared_types::GoogleAccount;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Configuration for the email polling task
#[derive(Debug, Clone)]
pub struct EmailPollerConfig {
    /// How often to poll for new emails (default: 5 minutes)
    pub poll_interval: Duration,
    /// Minimum seconds between polls per account (rate limiting)
    pub rate_limit_secs: u64,
    /// Maximum emails to fetch per poll
    pub max_fetch_per_poll: u32,
    /// Maximum unprocessed emails to process per cycle
    pub max_process_per_cycle: i64,
}

impl Default for EmailPollerConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(300), // 5 minutes
            rate_limit_secs: 60,                     // 1 minute minimum between polls
            max_fetch_per_poll: 50,
            max_process_per_cycle: 100,
        }
    }
}

impl EmailPollerConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let poll_interval_secs = std::env::var("EMAIL_POLL_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(300);

        let rate_limit_secs = std::env::var("EMAIL_RATE_LIMIT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);

        let max_fetch_per_poll = std::env::var("EMAIL_MAX_FETCH_PER_POLL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(50);

        let max_process_per_cycle = std::env::var("EMAIL_MAX_PROCESS_PER_CYCLE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(100);

        Self {
            poll_interval: Duration::from_secs(poll_interval_secs),
            rate_limit_secs,
            max_fetch_per_poll,
            max_process_per_cycle,
        }
    }
}

/// Tracks the last poll time for each account (by email)
struct RateLimiter {
    last_poll: HashMap<String, Instant>,
}

impl RateLimiter {
    fn new() -> Self {
        Self {
            last_poll: HashMap::new(),
        }
    }

    fn can_poll(&self, email: &str, rate_limit_secs: u64) -> bool {
        match self.last_poll.get(email) {
            Some(last) => last.elapsed().as_secs() >= rate_limit_secs,
            None => true,
        }
    }

    fn record_poll(&mut self, email: &str) {
        self.last_poll.insert(email.to_string(), Instant::now());
    }
}

/// Account state tracked between polls
#[derive(Debug, Clone, Default)]
struct AccountState {
    last_history_id: Option<u64>,
}

/// Start the email polling background task
pub async fn start_email_polling_task(pool: DbPool) {
    let config = EmailPollerConfig::from_env();

    tracing::info!(
        "Starting email polling task (interval: {:?}, rate limit: {}s)",
        config.poll_interval,
        config.rate_limit_secs
    );

    let mut rate_limiter = RateLimiter::new();
    let mut account_states: HashMap<Uuid, AccountState> = HashMap::new();

    loop {
        if let Err(e) = run_poll_cycle(&pool, &config, &mut rate_limiter, &mut account_states).await
        {
            tracing::error!("Email poll cycle failed: {}", e);
        }

        tokio::time::sleep(config.poll_interval).await;
    }
}

async fn run_poll_cycle(
    pool: &DbPool,
    config: &EmailPollerConfig,
    rate_limiter: &mut RateLimiter,
    account_states: &mut HashMap<Uuid, AccountState>,
) -> Result<()> {
    let mut conn = pool.get().await.context("Failed to get DB connection")?;

    // Get Google accounts from database (OAuth tokens are stored here)
    let accounts = db::google_accounts::list_all(&mut conn).await?;

    if accounts.is_empty() {
        tracing::debug!("No active email accounts configured");
        return Ok(());
    }

    tracing::debug!("Polling {} active email accounts", accounts.len());

    for account in accounts {
        // Check rate limiting
        if !rate_limiter.can_poll(&account.email, config.rate_limit_secs) {
            tracing::debug!("Skipping {} (rate limited)", account.email);
            continue;
        }

        // Get or create account state
        let state = account_states.entry(account.id).or_default();

        match poll_single_account(&account, state, pool, config.max_fetch_per_poll).await {
            Ok(result) => {
                if result.count > 0 {
                    tracing::info!("Fetched {} new emails from {}", result.count, account.email);
                }

                // Update state for next poll
                if let Some(history_id) = result.history_id {
                    state.last_history_id = Some(history_id);
                }

                rate_limiter.record_poll(&account.email);
            }
            Err(e) => {
                tracing::error!("Failed to poll {}: {}", account.email, e);
            }
        }
    }

    // Process any unprocessed emails
    match processor::process_pending_emails(pool, config.max_process_per_cycle).await {
        Ok(stats) => {
            if stats.processed > 0 {
                tracing::info!(
                    "Processed {} emails: {} rule matched, {} heuristic proposed, {} ignored, {} errors",
                    stats.processed,
                    stats.rule_matched,
                    stats.heuristic_proposed,
                    stats.ignored,
                    stats.errors
                );
            }
        }
        Err(e) => {
            tracing::error!("Failed to process pending emails: {}", e);
        }
    }

    Ok(())
}

struct PollResult {
    count: usize,
    history_id: Option<u64>,
}

async fn poll_single_account(
    account: &GoogleAccount,
    state: &AccountState,
    pool: &DbPool,
    max_fetch_per_poll: u32,
) -> Result<PollResult> {
    tracing::debug!("Polling {}...", account.email);

    let client = GmailClient::from_account(account)
        .await
        .context("Failed to create Gmail client")?;

    let emails = match state.last_history_id {
        Some(history_id) if history_id > 0 => {
            client
                .fetch_messages_since(history_id, max_fetch_per_poll)
                .await?
        }
        _ => client.fetch_messages(max_fetch_per_poll).await?,
    };

    let count = save_emails_to_db(&emails, account.id, pool).await?;

    // Get current history ID for next sync
    let history_id = client.get_history_id().await.ok();

    Ok(PollResult { count, history_id })
}

async fn save_emails_to_db(
    emails: &[EmailMessage],
    account_id: Uuid,
    pool: &DbPool,
) -> Result<usize> {
    let mut conn = pool.get().await.context("Failed to get DB connection")?;
    let mut count = 0;

    for email in emails {
        // Parse "From" header into address and name
        let (from_address, from_name) = parse_from_header(&email.from);

        // Parse To addresses
        let to_addresses: Vec<Option<String>> = email
            .to
            .iter()
            .map(|addr| Some(parse_from_header(addr).0))
            .collect();

        // Parse CC addresses
        let cc_addresses: Option<Vec<Option<String>>> = if email.cc.is_empty() {
            None
        } else {
            Some(
                email
                    .cc
                    .iter()
                    .map(|addr| Some(parse_from_header(addr).0))
                    .collect(),
            )
        };

        // Labels as array
        let labels: Option<Vec<Option<String>>> = if email.labels.is_empty() {
            None
        } else {
            Some(email.labels.iter().map(|l| Some(l.clone())).collect())
        };

        let new_email = NewEmail {
            account_id,
            gmail_id: email.id.clone(),
            thread_id: email.thread_id.clone(),
            history_id: email.history_id.map(|h| h as i64),
            subject: email.subject.clone(),
            from_address,
            from_name,
            to_addresses,
            cc_addresses,
            snippet: Some(email.snippet.clone()),
            body_text: email.body_text.clone(),
            body_html: email.body_html.clone(),
            labels,
            has_attachments: email.has_attachments,
            received_at: email.received_at.unwrap_or_else(Utc::now),
        };

        match db::emails::insert(&mut conn, new_email).await {
            Ok(Some(_)) => {
                tracing::debug!("  Stored: {} - {}", email.id, email.subject);
                count += 1;
            }
            Ok(None) => {
                tracing::trace!("  Skipped (duplicate): {}", email.id);
            }
            Err(e) => {
                tracing::warn!("  Failed to store {}: {}", email.id, e);
            }
        }
    }

    Ok(count)
}

/// Parse a "From" header like "John Doe <john@example.com>" into (address, name)
fn parse_from_header(from: &str) -> (String, Option<String>) {
    let from = from.trim();

    if let Some(bracket_start) = from.rfind('<') {
        if let Some(bracket_end) = from.rfind('>') {
            let address = from[bracket_start + 1..bracket_end].trim().to_string();
            let name = from[..bracket_start].trim();
            let name = name.trim_matches('"').trim();
            let name = if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            };
            return (address, name);
        }
    }

    (from.to_string(), None)
}
