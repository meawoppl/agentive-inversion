use crate::config::{AccountConfig, Config};
use crate::imap_client::ImapClient;
use anyhow::{Context, Result};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

/// Tracks the last poll time for each account
pub struct RateLimiter {
    last_poll: HashMap<String, Instant>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            last_poll: HashMap::new(),
        }
    }

    /// Check if enough time has passed since last poll
    pub fn can_poll(&self, account_email: &str, rate_limit_secs: u64) -> bool {
        match self.last_poll.get(account_email) {
            Some(last) => last.elapsed().as_secs() >= rate_limit_secs,
            None => true,
        }
    }

    /// Record that we just polled this account
    pub fn record_poll(&mut self, account_email: &str) {
        self.last_poll
            .insert(account_email.to_string(), Instant::now());
    }

    /// Get seconds until next allowed poll
    pub fn seconds_until_allowed(&self, account_email: &str, rate_limit_secs: u64) -> u64 {
        match self.last_poll.get(account_email) {
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

/// Tracks the highest UID we've seen for each account
pub struct UidTracker {
    last_uid: HashMap<String, u32>,
    min_uid: HashMap<String, u32>,
    backfill_complete: HashMap<String, bool>,
}

impl UidTracker {
    pub fn new() -> Self {
        Self {
            last_uid: HashMap::new(),
            min_uid: HashMap::new(),
            backfill_complete: HashMap::new(),
        }
    }

    pub fn get(&self, account_email: &str) -> Option<u32> {
        self.last_uid.get(account_email).copied()
    }

    pub fn get_min(&self, account_email: &str) -> Option<u32> {
        self.min_uid.get(account_email).copied()
    }

    pub fn is_backfill_complete(&self, account_email: &str) -> bool {
        self.backfill_complete
            .get(account_email)
            .copied()
            .unwrap_or(false)
    }

    pub fn mark_backfill_complete(&mut self, account_email: &str) {
        self.backfill_complete
            .insert(account_email.to_string(), true);
    }

    pub fn update(&mut self, account_email: &str, uid: u32) {
        // Update max UID
        let current_max = self.last_uid.get(account_email).copied().unwrap_or(0);
        if uid > current_max {
            self.last_uid.insert(account_email.to_string(), uid);
        }

        // Update min UID
        let current_min = self.min_uid.get(account_email).copied().unwrap_or(u32::MAX);
        if uid < current_min {
            self.min_uid.insert(account_email.to_string(), uid);
        }
    }
}

impl Default for UidTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Format filename as yymmdd_hhmmss-email-uid.json
pub fn format_email_filename(
    received_at: Option<chrono::DateTime<chrono::Utc>>,
    email: &str,
    uid: &str,
) -> String {
    let timestamp = received_at
        .map(|dt| dt.format("%y%m%d_%H%M%S").to_string())
        .unwrap_or_else(|| "000000_000000".to_string());

    let email_safe = sanitize_for_filename(email);

    format!("{}-{}-{}.json", timestamp, email_safe, uid)
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

/// Poll a single account and download new emails (forward) and backfill old emails (backward)
pub async fn poll_account(
    account: &AccountConfig,
    inbox_dir: &Path,
    max_fetch_per_poll: u32,
    uid_tracker: &mut UidTracker,
) -> Result<usize> {
    tracing::info!("Polling {} ({})...", account.name, account.email);

    let mut client = ImapClient::connect(&account.imap_server, &account.email, &account.password)
        .await
        .context("Failed to connect")?;

    let mut count = 0;

    // Forward fetch: get new emails since last poll
    let emails = match uid_tracker.get(&account.email) {
        Some(last_uid) => {
            client
                .fetch_emails_since_uid(last_uid, max_fetch_per_poll)
                .await?
        }
        None => {
            // First poll - get recent emails
            client.fetch_recent_emails(max_fetch_per_poll).await?
        }
    };

    count += save_emails(&emails, account, inbox_dir, uid_tracker)?;

    // Backward fetch: keep fetching older emails until backfill complete
    while !uid_tracker.is_backfill_complete(&account.email) {
        if let Some(min_uid) = uid_tracker.get_min(&account.email) {
            tracing::info!(
                "Backfilling older emails for {} (min_uid: {})...",
                account.email,
                min_uid
            );

            let older_emails = client
                .fetch_emails_before_uid(min_uid, max_fetch_per_poll)
                .await?;

            if older_emails.is_empty() {
                tracing::info!(
                    "Backfill complete for {} - no more older emails",
                    account.email
                );
                uid_tracker.mark_backfill_complete(&account.email);
            } else {
                tracing::info!(
                    "Found {} older emails for {}",
                    older_emails.len(),
                    account.email
                );
                count += save_emails(&older_emails, account, inbox_dir, uid_tracker)?;
            }
        } else {
            break;
        }
    }

    client.logout().await.ok();

    Ok(count)
}

/// Save emails to disk and update UID tracker
fn save_emails(
    emails: &[crate::imap_client::EmailMessage],
    account: &AccountConfig,
    inbox_dir: &Path,
    uid_tracker: &mut UidTracker,
) -> Result<usize> {
    let mut count = 0;

    for email in emails {
        let uid: u32 = email.id.parse().unwrap_or(0);
        uid_tracker.update(&account.email, uid);

        let filename = format_email_filename(email.received_at, &account.email, &email.id);
        let filepath = inbox_dir.join(&filename);

        // Skip if already downloaded
        if filepath.exists() {
            continue;
        }

        let email_data = serde_json::json!({
            "uid": email.id,
            "mailbox": account.email,
            "imap_server": account.imap_server,
            "subject": email.subject,
            "from": email.from,
            "received_at": email.received_at,
            "snippet": email.snippet,
            "body": email.body,
            "unsubscribe": email.unsubscribe,
        });

        let json = serde_json::to_string_pretty(&email_data)?;
        fs::write(&filepath, &json)?;

        tracing::info!("  Downloaded: {}", filename);
        count += 1;
    }

    Ok(count)
}

/// Email metadata stored in the JSON files
#[derive(Debug, serde::Deserialize)]
pub struct EmailMetadata {
    pub uid: String,
    pub mailbox: String,
    pub imap_server: String,
}

/// Create a file watcher for the archive queue directory
pub fn create_archive_watcher(
    queue_dir: &Path,
) -> Result<(RecommendedWatcher, mpsc::Receiver<PathBuf>)> {
    let (tx, rx) = mpsc::channel();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        if let Ok(event) = res {
            // We care about new files being created or moved into the directory
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

    let uid: u32 = metadata.uid.parse().unwrap_or(0);
    if uid == 0 {
        tracing::warn!("Invalid UID in {}", path.display());
        return Ok(false);
    }

    // Find the matching account config
    let account = config
        .accounts
        .iter()
        .find(|a| a.email == metadata.mailbox && a.imap_server == metadata.imap_server);

    let account = match account {
        Some(a) => a,
        None => {
            tracing::warn!(
                "No account config found for {} @ {}",
                metadata.mailbox,
                metadata.imap_server
            );
            return Ok(false);
        }
    };

    tracing::info!("Archiving email {} from {}...", uid, account.email);

    // Connect and archive
    archive_emails(account, &[uid]).await?;

    // Remove the file from the queue
    fs::remove_file(path)?;

    Ok(true)
}

/// Process emails in the archive queue
pub async fn process_archive_queue(config: &Config) -> Result<usize> {
    let queue_dir = &config.archive_queue_dir;
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

    // Group by account
    let mut by_account: HashMap<String, Vec<(std::path::PathBuf, u32)>> = HashMap::new();

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

        let uid: u32 = metadata.uid.parse().unwrap_or(0);
        if uid == 0 {
            continue;
        }

        let key = format!("{}@{}", metadata.mailbox, metadata.imap_server);
        by_account.entry(key).or_default().push((path, uid));
    }

    let mut total_archived = 0;

    for (account_key, items) in by_account {
        // Parse account key
        let parts: Vec<&str> = account_key.splitn(2, '@').collect();
        if parts.len() != 2 {
            continue;
        }

        // Find the matching account config
        // The key format is "email@server" but email itself contains @
        // So we need to find the account that matches
        let account = config
            .accounts
            .iter()
            .find(|a| format!("{}@{}", a.email, a.imap_server) == account_key);

        let account = match account {
            Some(a) => a,
            None => {
                tracing::warn!("No account config found for {}", account_key);
                continue;
            }
        };

        let uids: Vec<u32> = items.iter().map(|(_, uid)| *uid).collect();
        let paths: Vec<_> = items.iter().map(|(p, _)| p.clone()).collect();

        tracing::info!("Archiving {} emails from {}...", uids.len(), account.email);

        // Connect and archive
        match archive_emails(account, &uids).await {
            Ok(_) => {
                // Remove the files from the queue
                for path in &paths {
                    if let Err(e) = fs::remove_file(path) {
                        tracing::warn!("Failed to remove {}: {}", path.display(), e);
                    }
                }
                total_archived += uids.len();
            }
            Err(e) => {
                tracing::error!("Failed to archive emails for {}: {}", account.email, e);
            }
        }
    }

    Ok(total_archived)
}

async fn archive_emails(account: &AccountConfig, uids: &[u32]) -> Result<()> {
    let mut client = ImapClient::connect(&account.imap_server, &account.email, &account.password)
        .await
        .context("Failed to connect")?;

    client.archive_many(uids).await?;
    client.logout().await.ok();

    Ok(())
}
