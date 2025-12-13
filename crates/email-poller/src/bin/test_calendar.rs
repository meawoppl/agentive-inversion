use anyhow::Result;
use chrono::{Duration, Utc};
use email_poller::calendar_client::{CalendarClient, CalendarEvent};
use email_poller::config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install crypto provider");
    tracing_subscriber::fmt::init();

    let config = Config::load(std::path::Path::new("mrg-setup.toml"))?;

    let calendar_config = config
        .calendar
        .expect("No calendar config found in mrg-setup.toml");

    println!("Testing calendar: {}", calendar_config.calendar_name);
    println!("Credentials: {}", calendar_config.credentials_path);

    let mut client = CalendarClient::new(calendar_config).await?;

    // Find the calendar first
    println!("Looking up calendar...");
    match client.find_calendar().await {
        Ok(id) => println!("Found calendar ID: {}", id),
        Err(e) => {
            eprintln!("Failed to find calendar: {}", e);
            return Err(e);
        }
    }

    // Create a test event
    let now = Utc::now();
    let event = CalendarEvent {
        summary: "Test Event from Email Poller".to_string(),
        description: Some(
            "This is a test event created by the email-poller calendar integration.".to_string(),
        ),
        start: now + Duration::hours(1),
        end: now + Duration::hours(2),
        location: Some("San Francisco".to_string()),
        email_link: Some("https://mail.google.com/mail/u/0/#inbox/test123".to_string()),
    };

    println!("Creating test event: {}", event.summary);
    match client.create_event(&event).await {
        Ok(()) => println!("Event created successfully!"),
        Err(e) => {
            eprintln!("Failed to create event: {}", e);
            return Err(e);
        }
    }

    println!("Calendar test complete!");
    Ok(())
}
