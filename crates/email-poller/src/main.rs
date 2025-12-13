use anyhow::{Context, Result};
use clap::Parser;
use email_poller::config::Config;
use email_poller::service::{
    create_archive_watcher, poll_account, process_archive_file, process_archive_queue, RateLimiter,
    UidTracker,
};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::interval;

#[derive(Parser)]
#[command(name = "email-poller")]
#[command(about = "Poll IMAP mailboxes for new emails and archive processed ones")]
struct Cli {
    /// Path to the TOML configuration file
    #[arg(short, long, default_value = "email-poller.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    // Load config
    let config = if cli.config.exists() {
        tracing::info!("Loading config from {}", cli.config.display());
        Config::load(&cli.config)?
    } else {
        anyhow::bail!(
            "Config file not found: {}. Create one or specify with --config",
            cli.config.display()
        );
    };

    tracing::info!("Email Poller Service starting");
    tracing::info!("  Inbox dir: {}", config.inbox_dir.display());
    tracing::info!(
        "  Archive queue dir: {}",
        config.archive_queue_dir.display()
    );
    tracing::info!("  Accounts: {}", config.accounts.len());
    tracing::info!(
        "  Rate limit: {}s, Max fetch: {}",
        config.rate_limit_secs,
        config.max_fetch_per_poll
    );
    for account in &config.accounts {
        tracing::info!("    - {} ({})", account.name, account.email);
    }

    // Create directories
    fs::create_dir_all(&config.inbox_dir).context("Failed to create inbox directory")?;
    fs::create_dir_all(&config.archive_queue_dir)
        .context("Failed to create archive queue directory")?;

    // Process any existing files in archive queue on startup
    match process_archive_queue(&config).await {
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
    let config_clone = config.clone();
    tokio::spawn(async move {
        loop {
            // Check for archive events every 500ms
            tokio::time::sleep(Duration::from_millis(500)).await;

            while let Ok(path) = archive_rx.try_recv() {
                tracing::debug!("Archive event: {}", path.display());
                // Small delay to ensure file is fully written
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

    // State
    let mut rate_limiter = RateLimiter::new();
    let mut uid_tracker = UidTracker::new();

    // Poll interval for fetching new emails
    let mut poll_interval = interval(Duration::from_secs(config.poll_interval_secs));

    tracing::info!(
        "Service running. Poll interval: {}s",
        config.poll_interval_secs
    );

    loop {
        poll_interval.tick().await;

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

            match poll_account(
                account,
                &config.inbox_dir,
                config.max_fetch_per_poll,
                &mut uid_tracker,
            )
            .await
            {
                Ok(count) => {
                    rate_limiter.record_poll(&account.email);
                    if count > 0 {
                        tracing::info!("Downloaded {} new emails from {}", count, account.email);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to poll {}: {}", account.email, e);
                    rate_limiter.record_poll(&account.email);
                }
            }
        }
    }
}
