// Database models for Diesel
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

/// Database representation of agent_decisions
/// Uses TEXT fields for JSON data (stored as JSON strings, not JSONB)
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = crate::schema::agent_decisions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AgentDecisionRow {
    pub id: Uuid,
    pub source_type: String,
    pub source_id: Option<Uuid>,
    pub source_external_id: Option<String>,
    pub decision_type: String,
    pub proposed_action: String, // JSON stored as TEXT
    pub reasoning: String,
    pub reasoning_details: Option<String>, // JSON stored as TEXT
    pub confidence: f32,
    pub status: String,
    pub applied_rule_id: Option<Uuid>,
    pub result_todo_id: Option<Uuid>,
    pub user_feedback: Option<String>,
    pub created_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub executed_at: Option<DateTime<Utc>>,
}

impl From<AgentDecisionRow> for shared_types::AgentDecision {
    fn from(row: AgentDecisionRow) -> Self {
        shared_types::AgentDecision {
            id: row.id,
            source_type: row.source_type,
            source_id: row.source_id,
            source_external_id: row.source_external_id,
            decision_type: row.decision_type,
            proposed_action: row.proposed_action,
            reasoning: row.reasoning,
            reasoning_details: row.reasoning_details,
            confidence: row.confidence,
            status: row.status,
            applied_rule_id: row.applied_rule_id,
            result_todo_id: row.result_todo_id,
            user_feedback: row.user_feedback,
            created_at: row.created_at,
            reviewed_at: row.reviewed_at,
            executed_at: row.executed_at,
        }
    }
}

/// Insertable struct for new emails
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = crate::schema::emails)]
pub struct NewEmail {
    pub account_id: Uuid,
    pub gmail_id: String,
    pub thread_id: String,
    pub history_id: Option<i64>,
    pub subject: String,
    pub from_address: String,
    pub from_name: Option<String>,
    pub to_addresses: Vec<Option<String>>,
    pub cc_addresses: Option<Vec<Option<String>>>,
    pub snippet: Option<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub labels: Option<Vec<Option<String>>>,
    pub has_attachments: bool,
    pub received_at: DateTime<Utc>,
}
