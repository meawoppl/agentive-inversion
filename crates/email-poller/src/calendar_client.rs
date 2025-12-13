use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use google_calendar3::api::{Event, EventDateTime};
use google_calendar3::hyper_rustls::HttpsConnector;
use google_calendar3::CalendarHub;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::path::Path;

use crate::config::CalendarConfig;

/// Client for interacting with Google Calendar API
pub struct CalendarClient {
    hub: CalendarHub<HttpsConnector<HttpConnector>>,
    calendar_id: Option<String>,
    calendar_name: String,
}

/// Event to be created in the calendar
#[derive(Debug, Clone)]
pub struct CalendarEvent {
    pub summary: String,
    pub description: Option<String>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub location: Option<String>,
    /// Link back to the source email (Gmail URL)
    pub email_link: Option<String>,
}

impl CalendarClient {
    pub async fn new(config: CalendarConfig) -> Result<Self> {
        let secret =
            google_calendar3::yup_oauth2::read_application_secret(&config.credentials_path)
                .await
                .context("Failed to read OAuth credentials")?;

        let auth = google_calendar3::yup_oauth2::InstalledFlowAuthenticator::builder(
            secret,
            google_calendar3::yup_oauth2::InstalledFlowReturnMethod::HTTPRedirect,
        )
        .persist_tokens_to_disk(Path::new(&config.token_cache_path))
        .build()
        .await
        .context("Failed to build authenticator")?;

        let connector = google_calendar3::hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .context("Failed to load native TLS roots")?
            .https_or_http()
            .enable_http1()
            .build();

        let client = Client::builder(TokioExecutor::new()).build(connector);
        let hub = CalendarHub::new(client, auth);

        Ok(Self {
            hub,
            calendar_id: None,
            calendar_name: config.calendar_name,
        })
    }

    /// Find the calendar ID by name
    pub async fn find_calendar(&mut self) -> Result<String> {
        if let Some(ref id) = self.calendar_id {
            return Ok(id.clone());
        }

        let (_, calendar_list) = self
            .hub
            .calendar_list()
            .list()
            .doit()
            .await
            .context("Failed to list calendars")?;

        if let Some(items) = calendar_list.items {
            for calendar in items {
                if let Some(ref summary) = calendar.summary {
                    if summary == &self.calendar_name {
                        if let Some(id) = calendar.id {
                            tracing::info!(
                                "Found calendar '{}' with ID: {}",
                                self.calendar_name,
                                id
                            );
                            self.calendar_id = Some(id.clone());
                            return Ok(id);
                        }
                    }
                }
            }
        }

        anyhow::bail!("Calendar '{}' not found", self.calendar_name)
    }

    /// Create an event in the calendar
    pub async fn create_event(&mut self, event: &CalendarEvent) -> Result<()> {
        let calendar_id = self.find_calendar().await?;

        let mut description = event.description.clone().unwrap_or_default();
        if let Some(ref link) = event.email_link {
            if !description.is_empty() {
                description.push_str("\n\n");
            }
            description.push_str(&format!("Source email: {}", link));
        }

        let google_event = Event {
            summary: Some(event.summary.clone()),
            description: if description.is_empty() {
                None
            } else {
                Some(description)
            },
            location: event.location.clone(),
            start: Some(EventDateTime {
                date_time: Some(event.start),
                ..Default::default()
            }),
            end: Some(EventDateTime {
                date_time: Some(event.end),
                ..Default::default()
            }),
            ..Default::default()
        };

        let (_, created) = self
            .hub
            .events()
            .insert(google_event, &calendar_id)
            .doit()
            .await
            .context("Failed to create calendar event")?;

        tracing::info!(
            "Created calendar event: {} (id: {:?})",
            event.summary,
            created.id
        );
        Ok(())
    }
}
