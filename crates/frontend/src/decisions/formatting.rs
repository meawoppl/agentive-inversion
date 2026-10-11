use chrono::{DateTime, Utc};
use shared_types::DecisionEmailContext;

/// The date a row should show: when the email arrived, not when the triage
/// pipeline got round to scraping it. A proposal made today about a message
/// from three weeks ago is a three-week-old thing, and dating it "today"
/// hides exactly the staleness the reviewer is judging.
pub(super) fn email_or_decision_date(
    email: Option<&DecisionEmailContext>,
    decided_at: DateTime<Utc>,
) -> DateTime<Utc> {
    email.map(|e| e.received_at).unwrap_or(decided_at)
}

/// Says which of the two dates is on screen, so the distinction is
/// discoverable rather than something the reviewer has to infer.
pub(super) fn date_tooltip(
    email: Option<&DecisionEmailContext>,
    decided_at: DateTime<Utc>,
) -> String {
    match email {
        Some(e) => format!(
            "Email received {}\nProposed {}",
            e.received_at.format("%Y-%m-%d %H:%M"),
            decided_at.format("%Y-%m-%d %H:%M")
        ),
        None => format!("Proposed {}", decided_at.format("%Y-%m-%d %H:%M")),
    }
}

/// Decision types are stored snake_case; the inbox shows them as prose
pub(super) fn humanize_type(decision_type: &str) -> String {
    match decision_type {
        "create_todo" => "Todos".to_string(),
        "forward_email" => "Forwards".to_string(),
        "create_calendar_event" => "Events".to_string(),
        "archive" => "Archives".to_string(),
        "ignore" => "Ignores".to_string(),
        "defer" => "Deferrals".to_string(),
        "categorize" => "Categorizations".to_string(),
        "set_due_date" => "Due dates".to_string(),
        other => other.replace('_', " "),
    }
}
