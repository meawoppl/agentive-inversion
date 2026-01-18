use crate::gmail_client::EmailMessage;
use chrono::Utc;
use shared_types::Todo;
use uuid::Uuid;

/// Process an email into a todo item
///
/// Simple keyword-based heuristic. In production you'd want:
/// - AI/NLP to extract action items
/// - Better filtering of newsletters, spam, etc.
/// - Due date extraction from email text
pub fn process_email_to_todo(email: &EmailMessage) -> Option<Todo> {
    let subject_lower = email.subject.to_lowercase();

    // Check for actionable keywords in subject
    let is_actionable = subject_lower.contains("todo")
        || subject_lower.contains("action required")
        || subject_lower.contains("action needed")
        || subject_lower.contains("please review")
        || subject_lower.contains("urgent")
        || subject_lower.contains("asap")
        || subject_lower.contains("deadline")
        || subject_lower.contains("reminder");

    if !is_actionable {
        return None;
    }

    // Build todo title from subject
    let title = if email.subject.len() > 100 {
        format!("{}...", &email.subject[..97])
    } else {
        email.subject.clone()
    };

    // Build description with snippet and sender
    let description = if let Some(body) = &email.body {
        let truncated = if body.len() > 500 {
            format!("{}...", &body[..497])
        } else {
            body.clone()
        };
        Some(format!("{}\n\n---\nFrom: {}", truncated, email.from))
    } else {
        Some(format!("{}\n\n---\nFrom: {}", email.snippet, email.from))
    };

    let now = Utc::now();
    Some(Todo {
        id: Uuid::new_v4(),
        title,
        description,
        completed: false,
        source: "email".to_string(),
        source_id: Some(email.id.clone()),
        due_date: None,
        created_at: email.received_at.unwrap_or(now),
        updated_at: now,
        link: None,
        category_id: None,
        decision_id: None, // Will be set when created via agent decision flow
    })
}
