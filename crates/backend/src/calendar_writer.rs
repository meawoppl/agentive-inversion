//! Google Calendar event creation using stored OAuth tokens.
//!
//! Agent-created events land on a dedicated calendar (default "Agent") so
//! they never pollute the primary calendar and can be toggled or audited
//! as a group. Events link back to their source email.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use google_calendar3::api::{Calendar, Event, EventDateTime, EventSource};
use google_calendar3::hyper_rustls::HttpsConnector;
use google_calendar3::CalendarHub;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use shared_types::GoogleAccount;

pub struct CalendarWriter {
    hub: CalendarHub<HttpsConnector<HttpConnector>>,
}

/// Event to create on the agent calendar
#[derive(Debug, Clone)]
pub struct NewCalendarEvent {
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    /// Link back to the source email (Gmail URL)
    pub email_link: Option<String>,
}

/// Result of a successful event creation
#[derive(Debug, Clone)]
pub struct CreatedEvent {
    pub google_event_id: String,
    pub html_link: Option<String>,
    pub calendar_id: String,
}

impl CalendarWriter {
    /// Build a client from a GoogleAccount's stored refresh token
    pub async fn from_account(account: &GoogleAccount) -> Result<Self> {
        let client_id = std::env::var("GOOGLE_CLIENT_ID")
            .context("GOOGLE_CLIENT_ID environment variable must be set")?;
        let client_secret = std::env::var("GOOGLE_CLIENT_SECRET")
            .context("GOOGLE_CLIENT_SECRET environment variable must be set")?;

        let secret = google_calendar3::yup_oauth2::authorized_user::AuthorizedUserSecret {
            client_id,
            client_secret,
            refresh_token: account.refresh_token.clone(),
            key_type: "authorized_user".to_string(),
        };

        let auth = google_calendar3::yup_oauth2::AuthorizedUserAuthenticator::builder(secret)
            .build()
            .await
            .context("Failed to build authenticator from refresh token")?;

        let connector = google_calendar3::hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .context("Failed to load native TLS roots")?
            .https_or_http()
            .enable_http1()
            .build();

        let client = Client::builder(TokioExecutor::new()).build(connector);
        let hub = CalendarHub::new(client, auth);

        Ok(Self { hub })
    }

    /// Find a calendar by summary, creating it if missing. Returns its ID.
    pub async fn ensure_calendar(&self, name: &str) -> Result<String> {
        let (_, list) = self
            .hub
            .calendar_list()
            .list()
            .doit()
            .await
            .context("Failed to list calendars")?;

        if let Some(items) = list.items {
            for cal in items {
                if cal.summary.as_deref() == Some(name) {
                    return cal.id.context("Calendar has no ID");
                }
            }
        }

        let new_calendar = Calendar {
            summary: Some(name.to_string()),
            description: Some("Events created by the agentive-inversion triage agent".to_string()),
            ..Default::default()
        };

        let (_, created) = self
            .hub
            .calendars()
            .insert(new_calendar)
            .doit()
            .await
            .context("Failed to create calendar")?;

        tracing::info!("Created calendar '{}'", name);
        created.id.context("Created calendar has no ID")
    }

    /// Create an event, linking back to the source email in both the
    /// description and the event source field
    pub async fn create_event(
        &self,
        calendar_id: &str,
        event: NewCalendarEvent,
    ) -> Result<CreatedEvent> {
        let description = match (&event.description, &event.email_link) {
            (Some(d), Some(link)) => Some(format!("{d}\n\nSource email: {link}")),
            (None, Some(link)) => Some(format!("Source email: {link}")),
            (Some(d), None) => Some(d.clone()),
            (None, None) => None,
        };

        let source = event.email_link.as_ref().map(|link| EventSource {
            title: Some("Source email".to_string()),
            url: Some(link.clone()),
        });

        let api_event = Event {
            summary: Some(event.summary),
            description,
            location: event.location,
            start: Some(EventDateTime {
                date_time: Some(event.start),
                ..Default::default()
            }),
            end: Some(EventDateTime {
                date_time: Some(event.end),
                ..Default::default()
            }),
            source,
            ..Default::default()
        };

        let (_, created) = self
            .hub
            .events()
            .insert(api_event, calendar_id)
            .doit()
            .await
            .context("Failed to create event")?;

        Ok(CreatedEvent {
            google_event_id: created.id.context("Created event has no ID")?,
            html_link: created.html_link,
            calendar_id: calendar_id.to_string(),
        })
    }
}
