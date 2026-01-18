use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Directory where downloaded emails are stored
    pub inbox_dir: PathBuf,

    /// Directory where emails to be archived are placed
    pub archive_queue_dir: PathBuf,

    /// How often to poll for new emails (seconds)
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,

    /// How often to check the archive queue (seconds)
    #[serde(default = "default_archive_check_interval")]
    pub archive_check_interval_secs: u64,

    /// Minimum seconds between polls (global)
    #[serde(default = "default_rate_limit")]
    pub rate_limit_secs: u64,

    /// Maximum emails to fetch per poll (global)
    #[serde(default = "default_max_fetch")]
    pub max_fetch_per_poll: u32,

    /// Gmail accounts to poll
    #[serde(default)]
    pub accounts: Vec<GmailConfig>,

    /// Optional calendar integration for adding detected events
    #[serde(default)]
    pub calendar: Option<CalendarConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmailConfig {
    /// Display name for the account
    pub name: String,

    /// Email address (used for identification in saved files)
    pub email: String,

    /// Path to Google OAuth client credentials JSON file
    pub credentials_path: String,

    /// Path to store the OAuth token cache for this account
    #[serde(default = "default_gmail_token_cache")]
    pub token_cache_path: String,
}

fn default_gmail_token_cache() -> String {
    "gmail_token_cache.json".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarConfig {
    /// Path to Google OAuth client credentials JSON file
    pub credentials_path: String,

    /// Path to store the OAuth token cache
    #[serde(default = "default_token_cache")]
    pub token_cache_path: String,

    /// Calendar name to add events to (e.g., "AI - Events")
    pub calendar_name: String,
}

fn default_token_cache() -> String {
    "calendar_token_cache.json".to_string()
}

fn default_poll_interval() -> u64 {
    300 // 5 minutes
}

fn default_archive_check_interval() -> u64 {
    30 // 30 seconds
}

fn default_rate_limit() -> u64 {
    60 // 1 minute minimum between polls
}

fn default_max_fetch() -> u32 {
    50
}

impl Config {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn example() -> Self {
        Config {
            inbox_dir: PathBuf::from("./emails/inbox"),
            archive_queue_dir: PathBuf::from("./emails/to_archive"),
            poll_interval_secs: 300,
            archive_check_interval_secs: 30,
            rate_limit_secs: 60,
            max_fetch_per_poll: 50,
            accounts: Vec::new(),
            calendar: None,
        }
    }
}
