use crate::config::{Config, GmailConfig};
use crate::gmail_client::{EmailMessage, GmailClient};
use anyhow::{Context, Result};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

/// Tracks the last poll time for each account (by email)
pub struct RateLimiter {
    last_poll: HashMap<String, Instant>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            last_poll: HashMap::new(),
        }
    }

    pub fn can_poll(&self, email: &str, rate_limit_secs: u64) -> bool {
        match self.last_poll.get(email) {
            Some(last) => last.elapsed().as_secs() >= rate_limit_secs,
            None => true,
        }
    }

    pub fn record_poll(&mut self, email: &str) {
        self.last_poll.insert(email.to_string(), Instant::now());
    }

    pub fn seconds_until_allowed(&self, email: &str, rate_limit_secs: u64) -> u64 {
        match self.last_poll.get(email) {
            Some(last) => {
                let elapsed = last.elapsed().as_secs();
                rate_limit_secs.saturating_sub(elapsed)
            }
            None => 0,
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Format filename as yymmdd_hhmmss-email-msgid.json
pub fn format_email_filename(
    received_at: Option<chrono::DateTime<chrono::Utc>>,
    email: &str,
    message_id: &str,
) -> String {
    let timestamp = received_at
        .map(|dt| dt.format("%y%m%d_%H%M%S").to_string())
        .unwrap_or_else(|| "000000_000000".to_string());

    let email_safe = sanitize_for_filename(email);
    let msg_id_safe = sanitize_for_filename(message_id);

    format!("{}-{}-{}.json", timestamp, email_safe, msg_id_safe)
}

fn sanitize_for_filename(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' | '@' | ' ' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect()
}

/// Poll result containing the count and the new history ID
pub struct PollResult {
    pub count: usize,
    pub history_id: Option<u64>,
}

/// Account state tracked between polls
#[derive(Debug, Clone, Default)]
pub struct AccountState {
    pub last_history_id: Option<u64>,
}

/// Poll a single account and download new emails
pub async fn poll_account(
    account: &GmailConfig,
    state: &AccountState,
    inbox_dir: &Path,
    max_fetch_per_poll: u32,
) -> Result<PollResult> {
    tracing::info!("Polling {} ({})...", account.name, account.email);

    let client = GmailClient::new(account)
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

    let count = save_emails(&emails, &account.email, inbox_dir)?;

    // Get current history ID for next sync
    let history_id = client.get_history_id().await.ok();

    Ok(PollResult { count, history_id })
}

/// Save emails to disk
fn save_emails(emails: &[EmailMessage], account_email: &str, inbox_dir: &Path) -> Result<usize> {
    let mut count = 0;

    for email in emails {
        let filename = format_email_filename(email.received_at, account_email, &email.id);
        let filepath = inbox_dir.join(&filename);

        // Skip if already downloaded
        if filepath.exists() {
            continue;
        }

        let email_data = serde_json::json!({
            "message_id": email.id,
            "thread_id": email.thread_id,
            "mailbox": account_email,
            "subject": email.subject,
            "from": email.from,
            "received_at": email.received_at,
            "snippet": email.snippet,
            "body": email.body,
            "history_id": email.history_id,
        });

        let json = serde_json::to_string_pretty(&email_data)?;
        fs::write(&filepath, &json)?;

        tracing::info!("  Downloaded: {}", filename);
        count += 1;
    }

    Ok(count)
}

/// Email metadata stored in the JSON files (for archiving)
#[derive(Debug, serde::Deserialize)]
pub struct EmailMetadata {
    pub message_id: String,
    pub mailbox: String,
}

/// Create a file watcher for the archive queue directory
pub fn create_archive_watcher(
    queue_dir: &Path,
) -> Result<(RecommendedWatcher, mpsc::Receiver<PathBuf>)> {
    let (tx, rx) = mpsc::channel();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        if let Ok(event) = res {
            match event.kind {
                EventKind::Create(_) | EventKind::Modify(_) => {
                    for path in event.paths {
                        if path.extension().is_some_and(|ext| ext == "json") {
                            let _ = tx.send(path);
                        }
                    }
                }
                _ => {}
            }
        }
    })?;

    watcher.watch(queue_dir, RecursiveMode::NonRecursive)?;

    Ok((watcher, rx))
}

/// Process a single file from the archive queue
pub async fn process_archive_file(config: &Config, path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(path).context("Failed to read file")?;
    let metadata: EmailMetadata = serde_json::from_str(&content).context("Failed to parse JSON")?;

    // Find account by email address
    let account = config
        .accounts
        .iter()
        .find(|a| a.email == metadata.mailbox)
        .context("Account not found in config")?;

    tracing::info!(
        "Archiving email {} from {}...",
        metadata.message_id,
        account.email
    );

    // Create client and archive
    let client = GmailClient::new(account)
        .await
        .context("Failed to create Gmail client")?;
    client.archive_message(&metadata.message_id).await?;

    // Remove the file from the queue
    fs::remove_file(path)?;

    Ok(true)
}

/// Process emails in the archive queue
pub async fn process_archive_queue(config: &Config, queue_dir: &Path) -> Result<usize> {
    if !queue_dir.exists() {
        return Ok(0);
    }

    let entries: Vec<_> = fs::read_dir(queue_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();

    if entries.is_empty() {
        return Ok(0);
    }

    tracing::info!("Found {} emails to archive", entries.len());

    // Group by account email
    let mut by_account: HashMap<String, Vec<(PathBuf, String)>> = HashMap::new();

    for entry in entries {
        let path = entry.path();
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to read {}: {}", path.display(), e);
                continue;
            }
        };

        let metadata: EmailMetadata = match serde_json::from_str(&content) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("Failed to parse {}: {}", path.display(), e);
                continue;
            }
        };

        by_account
            .entry(metadata.mailbox.clone())
            .or_default()
            .push((path, metadata.message_id));
    }

    let mut total_archived = 0;

    for (email, items) in by_account {
        let account = match config.accounts.iter().find(|a| a.email == email) {
            Some(a) => a,
            None => {
                tracing::warn!("Account {} not found in config, skipping", email);
                continue;
            }
        };

        let message_ids: Vec<String> = items.iter().map(|(_, id)| id.clone()).collect();
        let paths: Vec<_> = items.iter().map(|(p, _)| p.clone()).collect();

        tracing::info!(
            "Archiving {} emails from {}...",
            message_ids.len(),
            account.email
        );

        match archive_emails(account, &message_ids).await {
            Ok(_) => {
                for path in &paths {
                    if let Err(e) = fs::remove_file(path) {
                        tracing::warn!("Failed to remove {}: {}", path.display(), e);
                    }
                }
                total_archived += message_ids.len();
            }
            Err(e) => {
                tracing::error!("Failed to archive emails for {}: {}", account.email, e);
            }
        }
    }

    Ok(total_archived)
}

async fn archive_emails(account: &GmailConfig, message_ids: &[String]) -> Result<()> {
    let client = GmailClient::new(account)
        .await
        .context("Failed to create Gmail client")?;

    client.archive_many(message_ids).await?;

    Ok(())
}

/// Check if an error indicates auth is required
pub fn is_auth_error(err: &anyhow::Error) -> bool {
    let err_str = err.to_string().to_lowercase();
    err_str.contains("unauthorized")
        || err_str.contains("invalid_grant")
        || err_str.contains("token")
        || err_str.contains("refresh")
        || err_str.contains("no refresh token")
        || err_str.contains("re-authorization")
}
