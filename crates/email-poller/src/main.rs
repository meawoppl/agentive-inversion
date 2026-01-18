use anyhow::{Context, Result};
use clap::Parser;
use email_poller::config::Config;
use email_poller::service::{
    create_archive_watcher, is_auth_error, poll_account, process_archive_file,
    process_archive_queue, AccountState, RateLimiter,
};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

#[derive(Parser)]
#[command(name = "email-poller")]
#[command(about = "Poll Gmail accounts for new emails using OAuth")]
struct Cli {
    /// Path to the TOML configuration file
    #[arg(short, long, default_value = "email-poller.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Install rustls crypto provider
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    // Load config
    let config = if cli.config.exists() {
        tracing::info!("Loading config from {}", cli.config.display());
        Config::load(&cli.config)?
    } else {
        tracing::info!("No config file found, using defaults");
        Config::example()
    };

    let config = Arc::new(config);

    tracing::info!("Email Poller Service starting");
    tracing::info!("  Inbox dir: {}", config.inbox_dir.display());
    tracing::info!(
        "  Archive queue dir: {}",
        config.archive_queue_dir.display()
    );
    tracing::info!(
        "  Rate limit: {}s, Max fetch: {}",
        config.rate_limit_secs,
        config.max_fetch_per_poll
    );
    tracing::info!("  Configured accounts: {}", config.accounts.len());

    if config.accounts.is_empty() {
        tracing::warn!("No accounts configured. Add accounts to the config file.");
    }

    for account in &config.accounts {
        tracing::info!("    - {} ({})", account.name, account.email);
    }

    // Create directories
    fs::create_dir_all(&config.inbox_dir).context("Failed to create inbox directory")?;
    fs::create_dir_all(&config.archive_queue_dir)
        .context("Failed to create archive queue directory")?;

    // Process any existing files in archive queue on startup
    match process_archive_queue(&config, &config.archive_queue_dir).await {
        Ok(count) if count > 0 => {
            tracing::info!("Processed {} existing files in archive queue", count);
        }
        Err(e) => {
            tracing::error!("Failed to process existing archive queue: {}", e);
        }
        _ => {}
    }

    // Set up file watcher for archive queue
    let (_watcher, archive_rx) = create_archive_watcher(&config.archive_queue_dir)
        .context("Failed to create archive watcher")?;
    tracing::info!(
        "Watching {} for files to archive",
        config.archive_queue_dir.display()
    );

    // Spawn archive processor as a separate task
    let config_clone = Arc::clone(&config);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;

            while let Ok(path) = archive_rx.try_recv() {
                tracing::debug!("Archive event: {}", path.display());
                tokio::time::sleep(Duration::from_millis(100)).await;

                match process_archive_file(&config_clone, &path).await {
                    Ok(true) => {
                        tracing::info!("Archived: {}", path.display());
                    }
                    Ok(false) => {
                        tracing::debug!("Skipped: {}", path.display());
                    }
                    Err(e) => {
                        tracing::error!("Failed to archive {}: {}", path.display(), e);
                    }
                }
            }
        }
    });

    // State tracking for each account
    let mut account_states: HashMap<String, AccountState> = HashMap::new();
    let mut rate_limiter = RateLimiter::new();

    // Poll interval for fetching new emails
    let mut poll_interval = interval(Duration::from_secs(config.poll_interval_secs));

    tracing::info!(
        "Service running. Poll interval: {}s",
        config.poll_interval_secs
    );

    loop {
        poll_interval.tick().await;

        if config.accounts.is_empty() {
            tracing::debug!("No accounts configured");
            continue;
        }

        for account in &config.accounts {
            // Check rate limit
            if !rate_limiter.can_poll(&account.email, config.rate_limit_secs) {
                let wait =
                    rate_limiter.seconds_until_allowed(&account.email, config.rate_limit_secs);
                tracing::debug!(
                    "Rate limited: {} ({}s until next poll)",
                    account.email,
                    wait
                );
                continue;
            }

            // Get or create account state
            let state = account_states.entry(account.email.clone()).or_default();

            match poll_account(account, state, &config.inbox_dir, config.max_fetch_per_poll).await {
                Ok(result) => {
                    rate_limiter.record_poll(&account.email);

                    // Update state with new history ID
                    if let Some(history_id) = result.history_id {
                        state.last_history_id = Some(history_id);
                    }

                    if result.count > 0 {
                        tracing::info!(
                            "Downloaded {} new emails from {}",
                            result.count,
                            account.email
                        );
                    }
                }
                Err(e) => {
                    rate_limiter.record_poll(&account.email);

                    if is_auth_error(&e) {
                        tracing::error!(
                            "Auth error for {} - run the service interactively to re-authenticate: {}",
                            account.email,
                            e
                        );
                    } else {
                        tracing::error!("Failed to poll {}: {}", account.email, e);
                    }
                }
            }
        }
    }
}
