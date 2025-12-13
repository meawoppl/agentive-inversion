use anyhow::Result;
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use clap::Parser;
use email_poller::calendar_client::{CalendarClient, CalendarEvent};
use email_poller::config::Config;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "add-event")]
#[command(about = "Add calendar events from the command line")]
struct Cli {
    /// Path to the TOML configuration file
    #[arg(short, long, default_value = "mrg-setup.toml")]
    config: PathBuf,

    /// Event title/summary
    #[arg(short, long)]
    summary: String,

    /// Event description
    #[arg(short, long)]
    description: Option<String>,

    /// Start time in format: "YYYY-MM-DD HH:MM"
    #[arg(long)]
    start: String,

    /// End time in format: "YYYY-MM-DD HH:MM"
    #[arg(long)]
    end: String,

    /// Location
    #[arg(short, long)]
    location: Option<String>,

    /// Email search link
    #[arg(short = 'e', long)]
    email_link: Option<String>,

    /// Timezone (default: America/Los_Angeles)
    #[arg(short = 'z', long, default_value = "America/Los_Angeles")]
    timezone: String,
}

fn parse_datetime(s: &str, tz: Tz) -> Result<DateTime<Utc>> {
    let naive = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S"))?;

    let local_time = tz
        .from_local_datetime(&naive)
        .single()
        .ok_or_else(|| anyhow::anyhow!("Ambiguous or invalid local time: {}", s))?;

    Ok(local_time.with_timezone(&Utc))
}

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install crypto provider");
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let tz: Tz = cli
        .timezone
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid timezone: {}", cli.timezone))?;

    let config = Config::load(&cli.config)?;
    let calendar_config = config
        .calendar
        .expect("No calendar config found in config file");

    let mut client = CalendarClient::new(calendar_config).await?;

    let start = parse_datetime(&cli.start, tz)?;
    let end = parse_datetime(&cli.end, tz)?;

    let event = CalendarEvent {
        summary: cli.summary.clone(),
        description: cli.description,
        start,
        end,
        location: cli.location,
        email_link: cli.email_link,
    };

    println!("Adding event: {}", cli.summary);
    println!("  Start: {} {} -> {} UTC", cli.start, cli.timezone, start);
    println!("  End:   {} {} -> {} UTC", cli.end, cli.timezone, end);

    client.create_event(&event).await?;
    println!("Event added!");

    Ok(())
}
