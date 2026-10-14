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
        self.create_event_with_id(calendar_id, event, None).await
    }

    /// Stable provider IDs make an interrupted insert safe to reconcile/retry.
    pub async fn create_event_with_id(
        &self,
        calendar_id: &str,
        event: NewCalendarEvent,
        event_id: Option<String>,
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
            id: event_id.clone(),
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

        let result = self
            .hub
            .events()
            .insert(api_event, calendar_id)
            .doit()
            .await;
        let created = match result {
            Ok((_, event)) => event,
            Err(error) => {
                if let Some(id) = event_id {
                    // This includes a conflict after an earlier successful insert,
                    // and a timeout whose response was lost. Never generate a new ID.
                    match self.hub.events().get(calendar_id, &id).doit().await {
                        Ok((_, event)) => event,
                        Err(_) => return Err(error).context("Calendar outcome uncertain"),
                    }
                } else {
                    return Err(error).context("Failed to create event");
                }
            }
        };

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
        let mut events = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut call = self
                .hub
                .events()
                .list(calendar_id)
                .time_min(from)
                .time_max(to)
                .single_events(true)
                .show_deleted(false)
                .max_results(EVENT_PAGE_SIZE);
            if let Some(token) = &page_token {
                call = call.page_token(token);
            }

            let result = call.doit().await;
            if let Err(error) = &result {
                if unavailable_holiday_calendar(calendar_id, error) {
                    tracing::warn!(
                        calendar_id,
                        "Skipping unavailable public holiday calendar (404)"
                    );
                    return Ok(events);
                }
            }
            let (_, list) = result
                .with_context(|| format!("Failed to list events on calendar {calendar_id}"))?;

            events.extend(
                list.items
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|e| e.status.as_deref() != Some("cancelled"))
                    .filter_map(|e| ExistingEvent::from_api(&self.account_email, calendar_id, e)),
            );

            // A rescan spans every queued proposal, so the window can be
            // months wide and a single page will not cover it.
            page_token = list.next_page_token;
            if page_token.is_none() || events.len() >= MAX_EVENTS_PER_CALENDAR {
                if page_token.is_some() {
                    tracing::warn!(
                        "Calendar {calendar_id} returned more than {MAX_EVENTS_PER_CALENDAR} \
                         events in the window; duplicate detection saw only the first page(s)"
                    );
                }
                break;
            }
        }

        Ok(events)
    }
}

/// Events per API page
const EVENT_PAGE_SIZE: i32 = 250;

/// Ceiling on how much of one calendar a single window pulls in. A busy
/// calendar over a months-wide rescan window would otherwise page forever.
const MAX_EVENTS_PER_CALENDAR: usize = 2_500;

/// An event already on one of the user's calendars, reduced to what
/// duplicate detection compares.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
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

// Public Google holiday feeds cannot contain a user's appointments. A stale
// subscription must not block deduplication against their actual calendars.
// Do not hide errors on personal/shared calendars or authorization failures.
fn unavailable_holiday_calendar(calendar_id: &str, error: &google_calendar3::Error) -> bool {
    if !calendar_id.ends_with("#holiday@group.v.calendar.google.com") {
        return false;
    }
    match error {
        google_calendar3::Error::BadRequest(body) => {
            body.pointer("/error/code").and_then(|code| code.as_u64()) == Some(404)
        }
        google_calendar3::Error::Failure(response) => response.status().as_u16() == 404,
        _ => false,
    }
}

#[cfg(test)]
mod unavailable_calendar_tests {
    use super::*;
    #[test]
    fn only_missing_public_holiday_feeds_are_skipped() {
        let missing =
            google_calendar3::Error::BadRequest(serde_json::json!({"error":{"code":404}}));
        let denied = google_calendar3::Error::BadRequest(serde_json::json!({"error":{"code":403}}));
        let holiday = "en.usa#holiday@group.v.calendar.google.com";
        assert!(unavailable_holiday_calendar(holiday, &missing));
        assert!(!unavailable_holiday_calendar("primary", &missing));
        assert!(!unavailable_holiday_calendar(
            "shared@group.calendar.google.com",
            &missing
        ));
        assert!(!unavailable_holiday_calendar(holiday, &denied));
    }
}
