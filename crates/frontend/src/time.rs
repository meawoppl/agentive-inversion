/// Format a past timestamp as a short relative time like "3m ago"
pub(crate) fn format_relative_time(time: &chrono::DateTime<chrono::Utc>) -> String {
    let elapsed = chrono::Utc::now() - *time;
    if elapsed.num_seconds() < 60 {
        "just now".to_string()
    } else if elapsed.num_minutes() < 60 {
        format!("{}m ago", elapsed.num_minutes())
    } else if elapsed.num_hours() < 24 {
        format!("{}h ago", elapsed.num_hours())
    } else {
        format!("{}d ago", elapsed.num_days())
    }
}

/// Render a UTC instant in the viewer's own timezone. Server timestamps are
/// UTC, but nobody reads their own calendar in UTC — chrono's wasmbind
/// backend takes the zone from the browser.
pub(crate) fn format_local_time(time: &chrono::DateTime<chrono::Utc>) -> String {
    time.with_timezone(&chrono::Local)
        .format("%-I:%M:%S %p")
        .to_string()
}

/// Same, without seconds — for a time that is a schedule rather than a record
pub(crate) fn format_local_clock(time: &chrono::DateTime<chrono::Utc>) -> String {
    time.with_timezone(&chrono::Local)
        .format("%-I:%M %p")
        .to_string()
}

/// How long until a future instant, as "in 4m". A schedule that has come and
/// gone reads "due now" rather than a negative countdown: the cycle may be
/// running right now, or the last one may have overrun.
pub(crate) fn format_countdown(time: &chrono::DateTime<chrono::Utc>) -> String {
    let remaining = *time - chrono::Utc::now();
    if remaining.num_seconds() <= 0 {
        "due now".to_string()
    } else if remaining.num_seconds() < 60 {
        format!("in {}s", remaining.num_seconds())
    } else if remaining.num_minutes() < 60 {
        format!("in {}m", remaining.num_minutes())
    } else {
        format!(
            "in {}h{}m",
            remaining.num_hours(),
            remaining.num_minutes() % 60
        )
    }
}
