//! Screens proposed calendar events against what is already on the calendars.
//!
//! Most emails that look like events ARE calendar invites, and Google has
//! usually already put them on a calendar by the time triage reads the
//! message. Proposing "create this event" for something the user can already
//! see is pure noise, so a proposal that matches an existing entry never
//! reaches the review inbox.
//!
//! An invitation the user has not answered still counts as already on the
//! calendar: it is visible, it holds the slot, and re-proposing it asks the
//! same question twice. So does a declined one — declining IS the user's
//! answer, and re-proposing it overrides a decision they already made.
//!
//! The matching is pure so it can be tested without a calendar. Google
//! access lives in `calendar_client`.

use chrono::{DateTime, Duration, Utc};

use crate::calendar_client::{CalendarClient, ExistingEvent};
use crate::db::{self, DbPool};

/// How far apart two starts can be and still be the same event. Agents infer
/// times from prose ("Thursday evening") and get them approximately right,
/// so the window is generous — but it is paired with a title match, never
/// used alone.
const START_TOLERANCE_MINUTES: i64 = 120;

/// Share of the shorter title's significant words that must appear in the
/// other title
const TITLE_OVERLAP_RATIO: f32 = 0.6;

/// Significant words two titles must share before overlap counts. One shared
/// word is too weak: "Team lunch" and "Team offsite" share "team".
const MIN_SHARED_WORDS: usize = 2;

/// Words carrying no identifying signal in an event title
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "at", "for", "from", "in", "into", "my", "of", "on", "or", "our", "re",
    "fwd", "the", "to", "with", "your",
];

/// The proposed event, reduced to what matching compares
#[derive(Debug, Clone)]
pub struct CandidateEvent {
    pub summary: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    /// Permalink to the email this was extracted from. An existing event
    /// carrying the same link was created from the same message.
    pub source_link: Option<String>,
}

/// Why a proposal was judged to already exist
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchReason {
    /// An existing event links back to the same source email
    SameSourceEmail,
    /// Titles agree and the starts are within the tolerance
    TitleAndTime,
    /// Titles are identical after normalisation and fall on the same day
    TitleAndDay,
}

impl MatchReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            MatchReason::SameSourceEmail => "same_source_email",
            MatchReason::TitleAndTime => "title_and_time",
            MatchReason::TitleAndDay => "title_and_day",
        }
    }
}

/// A proposal that already exists, and the entry it matched
#[derive(Debug, Clone)]
pub struct DuplicateMatch {
    pub existing: ExistingEvent,
    pub reason: MatchReason,
}

impl DuplicateMatch {
    /// One-line description for logs and the audit trail
    pub fn describe(&self) -> String {
        let rsvp = self
            .existing
            .response_status
            .as_deref()
            .map(|s| format!(", rsvp={s}"))
            .unwrap_or_default();
        format!(
            "\"{}\" on {} calendar {} at {} (match={}{})",
            self.existing.summary,
            self.existing.account_email,
            self.existing.calendar_id,
            self.existing.start.to_rfc3339(),
            self.reason.as_str(),
            rsvp,
        )
    }
}

/// The first existing event that means the proposal is redundant, if any.
///
/// Checked strongest-evidence-first so the reported reason is the most
/// defensible one available, not merely the first row scanned.
pub fn find_duplicate(
    candidate: &CandidateEvent,
    existing: &[ExistingEvent],
) -> Option<DuplicateMatch> {
    if let Some(link) = candidate.source_link.as_deref().filter(|l| !l.is_empty()) {
        if let Some(found) = existing.iter().find(|e| references_link(e, link)) {
            return Some(DuplicateMatch {
                existing: found.clone(),
                reason: MatchReason::SameSourceEmail,
            });
        }
    }

    // Titles alone are not enough — two different "Team sync" entries are two
    // events — so the same title must also land at the same time: starts close
    // together, or spans that overlap when the agent guessed a long window.
    let tolerance = Duration::minutes(START_TOLERANCE_MINUTES);
    if let Some(found) = existing.iter().find(|e| {
        titles_match(&candidate.summary, &e.summary)
            && ((candidate.start - e.start).abs() <= tolerance
                || (candidate.start < e.end && e.start < candidate.end))
    }) {
        return Some(DuplicateMatch {
            existing: found.clone(),
            reason: MatchReason::TitleAndTime,
        });
    }

    existing
        .iter()
        .find(|e| {
            candidate.start.date_naive() == e.start.date_naive()
                && normalize(&candidate.summary) == normalize(&e.summary)
                && !normalize(&e.summary).is_empty()
        })
        .map(|found| DuplicateMatch {
            existing: found.clone(),
            reason: MatchReason::TitleAndDay,
        })
}

/// Whether an existing event points back at the given email. The agent
/// records the permalink in `source.url` and repeats it in the description,
/// so either is proof.
fn references_link(event: &ExistingEvent, link: &str) -> bool {
    event.source_url.as_deref() == Some(link)
        || event
            .description
            .as_deref()
            .is_some_and(|d| d.contains(link))
}

/// Significant lowercase words of a title, in order, without duplicates
fn normalize(title: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for word in title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1)
        .filter(|w| !STOPWORDS.contains(w))
    {
        let word = word.to_string();
        if !words.contains(&word) {
            words.push(word);
        }
    }
    words
}

/// Whether two titles name the same thing: identical after normalisation, or
/// sharing enough significant words that one is a fuller form of the other
/// ("Dentist" vs "Dentist cleaning appointment").
fn titles_match(a: &str, b: &str) -> bool {
    let (a, b) = (normalize(a), normalize(b));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    if a == b {
        return true;
    }

    let shared = a.iter().filter(|w| b.contains(w)).count();
    if shared < MIN_SHARED_WORDS {
        return false;
    }
    let ratio = shared as f32 / a.len().min(b.len()) as f32;
    ratio >= TITLE_OVERLAP_RATIO
}

/// Whether the screen runs at all. Set `TRIAGE_EVENT_DEDUPE=off` to send
/// every proposal through untouched — the escape hatch for a heuristic that
/// is suppressing events it should not.
pub fn enabled() -> bool {
    !matches!(
        std::env::var("TRIAGE_EVENT_DEDUPE").as_deref(),
        Ok("off") | Ok("false") | Ok("0")
    )
}

/// Search every calendar of every connected account for the proposal.
///
/// **Fails open.** A calendar we cannot read cannot prove anything is a
/// duplicate, so a Google outage or a revoked token lets proposals through
/// rather than silently swallowing the user's events. Errors are logged and
/// then dropped for the same reason.
pub async fn find_existing(pool: &DbPool, candidate: &CandidateEvent) -> Option<DuplicateMatch> {
    if !enabled() {
        return None;
    }

    let accounts = match load_accounts(pool).await {
        Ok(accounts) => accounts,
        Err(e) => {
            tracing::warn!(
                "Event dedupe: could not load accounts, letting proposal through: {e:#}"
            );
            return None;
        }
    };

    // One window covering both matching rules: the candidate's whole UTC day
    // (for the identical-title rule) widened by the start tolerance (for the
    // approximate-time rule, which can cross midnight).
    let margin = Duration::minutes(START_TOLERANCE_MINUTES);
    let day_start = candidate.start.date_naive().and_hms_opt(0, 0, 0)?.and_utc();
    let from = day_start - margin;
    let to = day_start + Duration::days(1) + margin;

    let mut seen: Vec<ExistingEvent> = Vec::new();
    for account in accounts {
        let client = match CalendarClient::from_account(&account).await {
            Ok(client) => client,
            Err(e) => {
                tracing::warn!(
                    "Event dedupe: no calendar access for {}: {e:#}",
                    account.email
                );
                continue;
            }
        };

        let calendar_ids = match client.list_calendar_ids().await {
            Ok(ids) => ids,
            Err(e) => {
                tracing::warn!(
                    "Event dedupe: could not list calendars for {}: {e:#}",
                    account.email
                );
                continue;
            }
        };

        for calendar_id in calendar_ids {
            match client.list_events_between(&calendar_id, from, to).await {
                Ok(events) => seen.extend(events),
                Err(e) => tracing::warn!(
                    "Event dedupe: could not read calendar {calendar_id} for {}: {e:#}",
                    account.email
                ),
            }
        }
    }

    find_duplicate(candidate, &seen)
}

async fn load_accounts(pool: &DbPool) -> anyhow::Result<Vec<shared_types::GoogleAccount>> {
    let mut conn = pool.get().await?;
    db::google_accounts::list_all(&mut conn).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(rfc3339: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(rfc3339)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn existing(summary: &str, start: &str) -> ExistingEvent {
        ExistingEvent {
            account_email: "matt@example.com".to_string(),
            calendar_id: "primary".to_string(),
            event_id: "evt-1".to_string(),
            summary: summary.to_string(),
            start: at(start),
            end: at(start) + Duration::hours(1),
            description: None,
            source_url: None,
            response_status: None,
            html_link: None,
        }
    }

    fn candidate(summary: &str, start: &str) -> CandidateEvent {
        CandidateEvent {
            summary: summary.to_string(),
            start: at(start),
            end: at(start) + Duration::hours(1),
            source_link: None,
        }
    }

    #[test]
    fn an_empty_calendar_never_matches() {
        assert!(find_duplicate(&candidate("Dentist", "2026-09-10T17:00:00Z"), &[]).is_none());
    }

    #[test]
    fn the_same_event_at_the_same_time_is_a_duplicate() {
        let found = find_duplicate(
            &candidate("Dentist appointment", "2026-09-10T17:00:00Z"),
            &[existing("Dentist appointment", "2026-09-10T17:00:00Z")],
        )
        .expect("should match");
        assert_eq!(found.reason, MatchReason::TitleAndTime);
    }

    #[test]
    fn a_fuller_title_still_matches() {
        let found = find_duplicate(
            &candidate("Dentist appointment", "2026-09-10T17:00:00Z"),
            &[existing(
                "Dentist appointment with Dr. Lee",
                "2026-09-10T17:30:00Z",
            )],
        )
        .expect("should match");
        assert_eq!(found.reason, MatchReason::TitleAndTime);
    }

    #[test]
    fn an_agent_inferred_time_inside_the_tolerance_matches() {
        // The agent read "Thursday evening" and guessed 18:00
        let found = find_duplicate(
            &candidate("Book club meeting", "2026-09-10T18:00:00Z"),
            &[existing("Book club meeting", "2026-09-10T19:30:00Z")],
        )
        .expect("should match");
        assert_eq!(found.reason, MatchReason::TitleAndTime);
    }

    #[test]
    fn a_wide_guessed_span_overlapping_the_real_one_matches() {
        // The agent read "Saturday evening" and blocked 17:00-23:00; the real
        // invitation is 20:00. The starts are 3h apart, past the tolerance,
        // but the spans overlap and the titles agree.
        let mut proposal = candidate("Housewarming party", "2026-09-12T17:00:00Z");
        proposal.end = at("2026-09-12T23:00:00Z");

        let found = find_duplicate(
            &proposal,
            &[existing("Housewarming party", "2026-09-12T20:00:00Z")],
        )
        .expect("should match");
        assert_eq!(found.reason, MatchReason::TitleAndTime);
    }

    #[test]
    fn one_shared_word_is_not_enough() {
        // Two genuinely different team events on the same afternoon
        assert!(find_duplicate(
            &candidate("Team lunch", "2026-09-10T17:00:00Z"),
            &[existing("Team offsite", "2026-09-10T17:00:00Z")],
        )
        .is_none());
    }

    #[test]
    fn the_same_title_far_apart_in_the_day_is_not_a_duplicate_by_time() {
        // Standup this morning is not standup tomorrow morning
        let found = find_duplicate(
            &candidate("Engineering standup", "2026-09-10T09:00:00Z"),
            &[existing("Engineering standup", "2026-09-10T21:00:00Z")],
        )
        .expect("same day and identical title");
        assert_eq!(found.reason, MatchReason::TitleAndDay);
    }

    #[test]
    fn an_identical_title_on_a_different_day_is_a_separate_event() {
        assert!(find_duplicate(
            &candidate("Engineering standup", "2026-09-10T09:00:00Z"),
            &[existing("Engineering standup", "2026-09-11T09:00:00Z")],
        )
        .is_none());
    }

    #[test]
    fn a_link_back_to_the_same_email_wins_over_any_title() {
        let link = "https://mail.google.com/mail/u/matt@example.com/#all/abc123";
        let mut prior = existing("Whatever the agent called it", "2026-12-01T00:00:00Z");
        prior.source_url = Some(link.to_string());

        let mut proposal = candidate("A completely different title", "2026-09-10T17:00:00Z");
        proposal.source_link = Some(link.to_string());

        let found = find_duplicate(&proposal, &[prior]).expect("should match");
        assert_eq!(found.reason, MatchReason::SameSourceEmail);
    }

    #[test]
    fn the_source_link_is_also_found_in_the_description() {
        let link = "https://mail.google.com/mail/u/matt@example.com/#all/abc123";
        let mut prior = existing("Renamed by the user", "2026-12-01T00:00:00Z");
        prior.description = Some(format!("Some notes\n\nSource email: {link}"));

        let mut proposal = candidate("Original title", "2026-09-10T17:00:00Z");
        proposal.source_link = Some(link.to_string());

        assert_eq!(
            find_duplicate(&proposal, &[prior]).unwrap().reason,
            MatchReason::SameSourceEmail
        );
    }

    #[test]
    fn an_unanswered_invite_counts_as_already_on_the_calendar() {
        let mut invite = existing("Quarterly board meeting", "2026-09-10T17:00:00Z");
        invite.response_status = Some("needsAction".to_string());

        let found = find_duplicate(
            &candidate("Quarterly board meeting", "2026-09-10T17:00:00Z"),
            &[invite],
        )
        .expect("an unanswered invite is still on the calendar");
        assert!(found.describe().contains("rsvp=needsAction"));
    }

    #[test]
    fn a_declined_invite_counts_too() {
        let mut declined = existing("Quarterly board meeting", "2026-09-10T17:00:00Z");
        declined.response_status = Some("declined".to_string());

        assert!(find_duplicate(
            &candidate("Quarterly board meeting", "2026-09-10T17:00:00Z"),
            &[declined],
        )
        .is_some());
    }

    #[test]
    fn punctuation_and_case_do_not_defeat_matching() {
        assert!(find_duplicate(
            &candidate("SARAH'S WEDDING -- Reception!", "2026-09-10T17:00:00Z"),
            &[existing("sarahs wedding reception", "2026-09-10T17:00:00Z")],
        )
        .is_some());
    }

    #[test]
    fn stopwords_alone_never_match() {
        // Titles that reduce to nothing must not collapse into each other
        assert!(find_duplicate(
            &candidate("The", "2026-09-10T17:00:00Z"),
            &[existing("An", "2026-09-10T17:00:00Z")],
        )
        .is_none());
    }

    #[test]
    fn the_matching_entry_is_reported_not_merely_the_first_one() {
        let events = vec![
            existing("Unrelated block", "2026-09-10T17:00:00Z"),
            existing("Dentist appointment", "2026-09-10T17:00:00Z"),
        ];
        let found = find_duplicate(
            &candidate("Dentist appointment", "2026-09-10T17:00:00Z"),
            &events,
        )
        .expect("should match");
        assert_eq!(found.existing.summary, "Dentist appointment");
    }
}
