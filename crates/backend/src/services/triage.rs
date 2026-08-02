//! Applies triage dispositions submitted by agent sessions via agent-cli.
//!
//! Policy lives here, server-side, not in the agents: archiving auto-executes
//! (reversible — labeled and searchable in Gmail), while calendar events and
//! todos land as proposed decisions gated on human approval.

use anyhow::{Context, Result};
use serde_json::json;
use uuid::Uuid;

use crate::db::{self, DbPool};
use crate::pollers::gmail_client::GmailClient;
use shared_types::{
    DecisionStatus, DecisionType, ProposedCalendarEventAction, ProposedTodoAction,
    TriageDecideAction, TriageDecideRequest, TriageDecideResponse,
};

/// Gmail label applied to everything the agent archives
pub const AGENT_ARCHIVE_LABEL: &str = "agent-archived";

/// Default calendar for agent-created events
pub const AGENT_CALENDAR_NAME: &str = "Agent";

/// Archive execution policy. "propose" (the default) records Sonnet's
/// determinations as gated proposals with full fidelity but never touches
/// Gmail — the dry-run mode. "execute" auto-archives on determination.
pub fn archive_mode() -> String {
    match std::env::var("TRIAGE_ARCHIVE_MODE").as_deref() {
        Ok("execute") => "execute".to_string(),
        _ => "propose".to_string(),
    }
}

/// Permalink to a message in the account's Gmail UI
pub fn gmail_permalink(account_email: &str, gmail_id: &str) -> String {
    format!("https://mail.google.com/mail/u/{account_email}/#all/{gmail_id}")
}

pub struct TriageService;

impl TriageService {
    /// Apply one agent disposition. Returns what happened so the agent's CLI
    /// call gets a truthful, machine-readable answer.
    pub async fn apply(pool: &DbPool, req: TriageDecideRequest) -> Result<TriageDecideResponse> {
        let mut conn = pool.get().await.context("Failed to get DB connection")?;

        let email = db::emails::get_by_id(&mut conn, req.email_id)
            .await
            .context("Email not found")?;

        let reasoning_details = req
            .model
            .as_ref()
            .map(|m| json!({ "llm_analysis": format!("model: {m}") }).to_string());

        match req.action {
            TriageDecideAction::ArchiveCandidate => {
                db::emails::set_triage_status(&mut conn, email.id, "archive_candidate").await?;
                Ok(TriageDecideResponse {
                    email_id: email.id,
                    decision_id: None,
                    triage_status: "archive_candidate".to_string(),
                    executed: false,
                })
            }

            TriageDecideAction::Keep => {
                db::emails::set_triage_status(&mut conn, email.id, "kept").await?;
                db::emails::mark_processed(&mut conn, email.id).await?;
                Ok(TriageDecideResponse {
                    email_id: email.id,
                    decision_id: None,
                    triage_status: "kept".to_string(),
                    executed: false,
                })
            }

            TriageDecideAction::Ignore => {
                let decision_id = db::decisions::create_with_status(
                    &mut conn,
                    "email",
                    Some(email.id),
                    Some(&email.gmail_id),
                    DecisionType::Ignore.as_str(),
                    "{}",
                    &req.reasoning,
                    reasoning_details.as_deref(),
                    0.9,
                    DecisionStatus::AutoApproved.as_str(),
                    None,
                )
                .await?;
                db::emails::set_triage_status(&mut conn, email.id, "ignored").await?;
                db::emails::mark_processed(&mut conn, email.id).await?;
                Ok(TriageDecideResponse {
                    email_id: email.id,
                    decision_id: Some(decision_id),
                    triage_status: "ignored".to_string(),
                    executed: false,
                })
            }

            TriageDecideAction::QueueAction => {
                db::emails::set_triage_status(&mut conn, email.id, "action_queued").await?;
                Ok(TriageDecideResponse {
                    email_id: email.id,
                    decision_id: None,
                    triage_status: "action_queued".to_string(),
                    executed: false,
                })
            }

            TriageDecideAction::Archive => {
                // Dry-run mode: identical determination, recorded as a gated
                // proposal; approval executes. Same prompts, same model, same
                // wire path — only this final side effect differs.
                if archive_mode() != "execute" {
                    let decision_id = db::decisions::create_with_status(
                        &mut conn,
                        "email",
                        Some(email.id),
                        Some(&email.gmail_id),
                        DecisionType::Archive.as_str(),
                        "{}",
                        &req.reasoning,
                        reasoning_details.as_deref(),
                        0.9,
                        DecisionStatus::Proposed.as_str(),
                        None,
                    )
                    .await?;
                    db::emails::set_triage_status(&mut conn, email.id, "archive_proposed").await?;
                    db::emails::mark_processed(&mut conn, email.id).await?;
                    return Ok(TriageDecideResponse {
                        email_id: email.id,
                        decision_id: Some(decision_id),
                        triage_status: "archive_proposed".to_string(),
                        executed: false,
                    });
                }

                let account = db::google_accounts::get_by_id(&mut conn, email.account_id)
                    .await
                    .context("Account for email not found")?;

                let decision_id = db::decisions::create_with_status(
                    &mut conn,
                    "email",
                    Some(email.id),
                    Some(&email.gmail_id),
                    DecisionType::Archive.as_str(),
                    "{}",
                    &req.reasoning,
                    reasoning_details.as_deref(),
                    0.9,
                    DecisionStatus::AutoApproved.as_str(),
                    None,
                )
                .await?;

                // Execute against Gmail. Failure leaves the email pending so a
                // later cycle retries; the decision records the failure.
                let gmail_result = async {
                    let client = GmailClient::from_account(&account).await?;
                    let label_id = client.ensure_label(AGENT_ARCHIVE_LABEL).await?;
                    client.archive_with_label(&email.gmail_id, &label_id).await
                }
                .await;

                match gmail_result {
                    Ok(()) => {
                        db::emails::mark_archived_in_gmail(&mut conn, email.id).await?;
                        db::emails::set_triage_status(&mut conn, email.id, "archived").await?;
                        db::emails::mark_processed(&mut conn, email.id).await?;
                        db::decisions::mark_executed(&mut conn, decision_id).await?;
                        Ok(TriageDecideResponse {
                            email_id: email.id,
                            decision_id: Some(decision_id),
                            triage_status: "archived".to_string(),
                            executed: true,
                        })
                    }
                    Err(e) => {
                        db::decisions::mark_failed(&mut conn, decision_id, "gmail archive failed")
                            .await?;
                        Err(e.context("Gmail archive failed; email left for retry"))
                    }
                }
            }

            TriageDecideAction::Event {
                summary,
                start,
                end,
                description,
                location,
            } => {
                let account = db::google_accounts::get_by_id(&mut conn, email.account_id)
                    .await
                    .context("Account for email not found")?;

                let action = ProposedCalendarEventAction {
                    account_email: account.email.clone(),
                    summary,
                    description,
                    location,
                    start,
                    end,
                    email_link: Some(gmail_permalink(&account.email, &email.gmail_id)),
                    calendar_name: Some(AGENT_CALENDAR_NAME.to_string()),
                };

                let decision_id = db::decisions::create_with_status(
                    &mut conn,
                    "email",
                    Some(email.id),
                    Some(&email.gmail_id),
                    DecisionType::CreateCalendarEvent.as_str(),
                    &serde_json::to_string(&action)?,
                    &req.reasoning,
                    reasoning_details.as_deref(),
                    0.8,
                    DecisionStatus::Proposed.as_str(),
                    None,
                )
                .await?;
                db::emails::set_triage_status(&mut conn, email.id, "event_proposed").await?;
                db::emails::mark_processed(&mut conn, email.id).await?;
                Ok(TriageDecideResponse {
                    email_id: email.id,
                    decision_id: Some(decision_id),
                    triage_status: "event_proposed".to_string(),
                    executed: false,
                })
            }

            TriageDecideAction::Todo {
                title,
                description,
                due_date,
            } => {
                let action = ProposedTodoAction {
                    todo_title: title,
                    todo_description: description,
                    due_date,
                    category_id: None,
                    priority: None,
                };

                let decision_id = db::decisions::create_with_status(
                    &mut conn,
                    "email",
                    Some(email.id),
                    Some(&email.gmail_id),
                    DecisionType::CreateTodo.as_str(),
                    &serde_json::to_string(&action)?,
                    &req.reasoning,
                    reasoning_details.as_deref(),
                    0.8,
                    DecisionStatus::Proposed.as_str(),
                    None,
                )
                .await?;
                db::emails::set_triage_status(&mut conn, email.id, "todo_proposed").await?;
                db::emails::mark_processed(&mut conn, email.id).await?;
                Ok(TriageDecideResponse {
                    email_id: email.id,
                    decision_id: Some(decision_id),
                    triage_status: "todo_proposed".to_string(),
                    executed: false,
                })
            }
        }
    }

    /// Execute an approved archive decision: label + archive in Gmail and
    /// bring the email's pipeline state up to date
    pub async fn execute_archive_decision(pool: &DbPool, decision_id: Uuid) -> Result<()> {
        let mut conn = pool.get().await.context("Failed to get DB connection")?;

        let decision = db::decisions::get_by_id(&mut conn, decision_id)
            .await
            .context("Decision not found")?;
        let email_id = decision
            .source_id
            .context("Archive decision has no source email")?;
        let email = db::emails::get_by_id(&mut conn, email_id).await?;
        let account = db::google_accounts::get_by_id(&mut conn, email.account_id).await?;
        drop(conn);

        let client = GmailClient::from_account(&account).await?;
        let label_id = client.ensure_label(AGENT_ARCHIVE_LABEL).await?;
        client
            .archive_with_label(&email.gmail_id, &label_id)
            .await?;

        let mut conn = pool.get().await.context("Failed to get DB connection")?;
        db::emails::mark_archived_in_gmail(&mut conn, email.id).await?;
        db::emails::set_triage_status(&mut conn, email.id, "archived").await?;
        db::decisions::mark_executed(&mut conn, decision_id).await?;

        Ok(())
    }

    /// Execute an approved calendar-event decision (the gated half of the
    /// event flow). Returns the created event's ID.
    pub async fn execute_calendar_decision(pool: &DbPool, decision_id: Uuid) -> Result<String> {
        use crate::calendar_writer::{CalendarWriter, NewCalendarEvent};

        let mut conn = pool.get().await.context("Failed to get DB connection")?;

        let decision = db::decisions::get_by_id(&mut conn, decision_id)
            .await
            .context("Decision not found")?;

        let action: ProposedCalendarEventAction =
            serde_json::from_str(&decision.proposed_action)
                .context("Decision proposed_action is not a calendar event")?;

        let account = db::google_accounts::get_by_email(&mut conn, &action.account_email)
            .await?
            .context("Account for calendar decision not found")?;
        drop(conn);

        let writer = CalendarWriter::from_account(&account).await?;
        let calendar_name = action
            .calendar_name
            .as_deref()
            .unwrap_or(AGENT_CALENDAR_NAME);
        let calendar_id = writer.ensure_calendar(calendar_name).await?;

        let created = writer
            .create_event(
                &calendar_id,
                NewCalendarEvent {
                    summary: action.summary,
                    description: action.description,
                    location: action.location,
                    start: action.start,
                    end: action.end,
                    email_link: action.email_link,
                },
            )
            .await?;

        let mut conn = pool.get().await.context("Failed to get DB connection")?;
        db::decisions::mark_executed(&mut conn, decision_id).await?;

        Ok(created.google_event_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_mode_defaults_to_propose() {
        // Absent or unrecognized values must fall back to the safe dry-run
        // mode; only the literal "execute" enables Gmail mutation
        std::env::remove_var("TRIAGE_ARCHIVE_MODE");
        assert_eq!(archive_mode(), "propose");
    }

    #[test]
    fn permalink_targets_the_account_mailbox() {
        let link = gmail_permalink("matt@exclosure.io", "18c2f0a");
        assert_eq!(
            link,
            "https://mail.google.com/mail/u/matt@exclosure.io/#all/18c2f0a"
        );
    }
}
