//! Google Calendar reads and writes using stored OAuth tokens.
//!
//! Agent-created events land on a dedicated calendar (default "Agent") so
//! they never pollute the primary calendar and can be toggled or audited
//! as a group. Events link back to their source email.
//!
//! Reads exist to answer one question before a proposal reaches the user:
//! is this already on a calendar? Most event emails ARE calendar invites,
//! and Google has usually put them on the calendar before the triage
//! pipeline ever sees the message.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use google_calendar3::api::{Calendar, Event, EventDateTime, EventSource};
use google_calendar3::hyper_rustls::HttpsConnector;
use google_calendar3::CalendarHub;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use shared_types::GoogleAccount;

pub struct CalendarClient {
    hub: CalendarHub<HttpsConnector<HttpConnector>>,
    /// Account this client speaks for, carried onto the events it reads so a
    /// duplicate found across several accounts can name the one it sits on
    account_email: String,
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

impl CalendarClient {
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

        Ok(Self {
            hub,
            account_email: account.email.clone(),
        })
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

    /// IDs of every calendar this account can see, including ones the user
    /// has unchecked in the Google UI. A hidden calendar still holds the
    /// event, so hiding it must not make a duplicate invisible to us.
    pub async fn list_calendar_ids(&self) -> Result<Vec<String>> {
        let (_, list) = self
            .hub
            .calendar_list()
            .list()
            .show_hidden(true)
            .doit()
            .await
            .context("Failed to list calendars")?;

        Ok(list
            .items
            .unwrap_or_default()
            .into_iter()
            .filter(|c| c.deleted != Some(true))
            .filter_map(|c| c.id)
            .collect())
    }

    /// Events on one calendar overlapping [from, to].
    ///
    /// `single_events(true)` expands recurrences, so a weekly standup is
    /// compared as the specific occurrence near the proposal rather than as
    /// its master record. Cancelled entries are excluded, but invitations the
    /// user has not answered are NOT — an unanswered invite is still the
    /// event sitting on their calendar.
    pub async fn list_events_between(
        &self,
        calendar_id: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<ExistingEvent>> {
        let (_, list) = self
            .hub
            .events()
            .list(calendar_id)
            .time_min(from)
            .time_max(to)
            .single_events(true)
            .show_deleted(false)
            .max_results(EVENT_PAGE_LIMIT)
            .doit()
            .await
            .with_context(|| format!("Failed to list events on calendar {calendar_id}"))?;

        Ok(list
            .items
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.status.as_deref() != Some("cancelled"))
            .filter_map(|e| ExistingEvent::from_api(&self.account_email, calendar_id, e))
            .collect())
    }
}

/// How many events one calendar-window query returns. The windows queried
/// here are hours wide, so this is a safety valve, not a paging strategy.
const EVENT_PAGE_LIMIT: i32 = 250;

/// An event already on one of the user's calendars, reduced to what
/// duplicate detection compares.
#[derive(Debug, Clone, PartialEq)]
pub struct ExistingEvent {
    pub account_email: String,
    pub calendar_id: String,
    pub event_id: String,
    pub summary: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    /// Description plus `source.url`, where a link back to the originating
    /// email would live if the agent created this event
    pub description: Option<String>,
    pub source_url: Option<String>,
    /// The user's own RSVP: "needsAction", "declined", "tentative",
    /// "accepted", or None when the event has no attendee list
    pub response_status: Option<String>,
    pub html_link: Option<String>,
}

impl ExistingEvent {
    /// Drop events we cannot compare on time. All-day events carry `date`
    /// rather than `dateTime`; those are matched on the day they start.
    fn from_api(account_email: &str, calendar_id: &str, event: Event) -> Option<Self> {
        let start = event.start.as_ref()?;
        let end = event.end.as_ref()?;
        let start_at = start.date_time.or_else(|| {
            start
                .date
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
        })?;
        let end_at = end
            .date_time
            .or_else(|| end.date.map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc()))?;

        Some(ExistingEvent {
            account_email: account_email.to_string(),
            calendar_id: calendar_id.to_string(),
            event_id: event.id.unwrap_or_default(),
            summary: event.summary.unwrap_or_default(),
            start: start_at,
            end: end_at,
            description: event.description,
            source_url: event.source.and_then(|s| s.url),
            response_status: event
                .attendees
                .unwrap_or_default()
                .into_iter()
                .find(|a| a.self_ == Some(true))
                .and_then(|a| a.response_status),
            html_link: event.html_link,
        })
    }
}
