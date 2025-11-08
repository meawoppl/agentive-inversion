use crate::gmail_client::EmailMessage;
use chrono::Utc;
use shared_types::{EmailAccount, Todo, TodoSource};
use uuid::Uuid;

/// Process an email into a todo item
///
/// This is a simple implementation that creates a todo for every email.
/// In production, you'd want more sophisticated logic to:
/// - Filter out newsletters, spam, etc.
/// - Parse action items from email content
/// - Detect due dates from email text
/// - Use AI/NLP to extract tasks
pub fn process_email_to_todo(email: &EmailMessage, account: &EmailAccount) -> Option<Todo> {
    // Simple heuristic: only create todos for emails with certain keywords
    let subject_lower = email.subject.to_lowercase();
    let is_actionable = subject_lower.contains("todo")
        || subject_lower.contains("action required")
        || subject_lower.contains("please")
        || subject_lower.contains("review")
        || subject_lower.contains("urgent")
        || subject_lower.contains("asap");

    if !is_actionable {
        return None;
    }

    // Create todo from email
    let title = if email.subject.len() > 100 {
        format!("{}...", &email.subject[..97])
    } else {
        email.subject.clone()
    };

    let description = if let Some(body) = &email.body {
        // Truncate body to reasonable length
        if body.len() > 500 {
            Some(format!("{}...\n\n---\nFrom: {}", &body[..497], email.from))
        } else {
            Some(format!("{}\n\n---\nFrom: {}", body, email.from))
        }
    } else {
        Some(format!("{}\n\n---\nFrom: {}", email.snippet, email.from))
    };

    Some(Todo {
        id: Uuid::new_v4(),
        title,
        description,
        completed: false,
        source: TodoSource::Email {
            account_id: account.id,
        },
        source_id: Some(email.id.clone()),
        due_date: None,
        created_at: email.received_at.unwrap_or_else(Utc::now),
        updated_at: Utc::now(),
    })
}
