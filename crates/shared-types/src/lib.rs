use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "diesel")]
mod json_wrapper;
#[cfg(feature = "diesel")]
pub use json_wrapper::JsonWrapper;

// ============================================================================
// Typed JSON Field Aliases
// ============================================================================
//
// These type aliases provide type-safe access to JSON fields stored in TEXT
// columns. Use these in database models instead of raw String fields.

/// Typed wrapper for proposed todo action JSON field
#[cfg(feature = "diesel")]
pub type TypedProposedAction = JsonWrapper<ProposedTodoAction>;

/// Typed wrapper for calendar attendees JSON field
#[cfg(feature = "diesel")]
pub type TypedCalendarAttendees = JsonWrapper<Vec<CalendarAttendee>>;

// ============================================================================
// Health
// ============================================================================

/// Health check response from `/health` and `/api/health`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}

/// Todo struct matching database column order exactly
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "diesel", derive(diesel::Queryable))]
pub struct Todo {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub completed: bool,
    pub source: String, // stored as VARCHAR: "manual", "email", "calendar"
    pub source_id: Option<String>,
    pub due_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub link: Option<String>,
    pub category_id: Option<Uuid>,
    pub decision_id: Option<Uuid>, // FK to agent_decisions if created by agent
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "diesel", derive(diesel::AsExpression))]
#[cfg_attr(feature = "diesel", diesel(sql_type = diesel::sql_types::Text))]
pub enum TodoSource {
    Manual,
    Email { account_id: Uuid },
    Calendar { calendar_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTodoRequest {
    pub title: String,
    pub description: Option<String>,
    pub due_date: Option<DateTime<Utc>>,
    pub link: Option<String>,
    pub category_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTodoRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub completed: Option<bool>,
    pub due_date: Option<DateTime<Utc>>,
    pub link: Option<String>,
    pub category_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmailProvider {
    Gmail,
}

/// Google account with OAuth tokens
/// Used for both Gmail and Calendar API access via scopes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "diesel", derive(diesel::Queryable))]
pub struct GoogleAccount {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub refresh_token: String,
    pub access_token: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub last_sync_error: Option<String>,
    pub last_sync_error_at: Option<DateTime<Utc>>,
    pub consecutive_failures: i32,
}

/// API response for Google account (excludes sensitive tokens)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoogleAccountResponse {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub last_sync_error: Option<String>,
    pub last_sync_error_at: Option<DateTime<Utc>>,
    pub consecutive_failures: i32,
}

impl From<GoogleAccount> for GoogleAccountResponse {
    fn from(account: GoogleAccount) -> Self {
        GoogleAccountResponse {
            id: account.id,
            email: account.email,
            name: account.name,
            created_at: account.created_at,
            last_synced_at: account.last_synced_at,
            last_sync_error: account.last_sync_error,
            last_sync_error_at: account.last_sync_error_at,
            consecutive_failures: account.consecutive_failures,
        }
    }
}

/// Calendar event from Google Calendar
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "diesel", derive(diesel::Queryable))]
pub struct CalendarEvent {
    pub id: Uuid,
    pub account_id: Uuid,
    pub google_event_id: String,
    pub ical_uid: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub all_day: bool,
    pub recurring: bool,
    pub recurrence_rule: Option<String>,
    pub status: String,
    pub organizer_email: Option<String>,
    pub attendees: Option<String>, // JSON array stored as text
    pub conference_link: Option<String>,
    pub fetched_at: DateTime<Utc>,
    pub processed: bool,
    pub processed_at: Option<DateTime<Utc>>,
}

/// API response for calendar events
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarEventResponse {
    pub id: Uuid,
    pub account_id: Uuid,
    pub google_event_id: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub all_day: bool,
    pub recurring: bool,
    pub status: String,
    pub organizer_email: Option<String>,
    pub attendees: Vec<CalendarAttendee>,
    pub conference_link: Option<String>,
    pub processed: bool,
}

/// Calendar event attendee
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarAttendee {
    pub email: String,
    pub display_name: Option<String>,
    pub response_status: Option<String>, // accepted, declined, tentative, needsAction
    pub organizer: bool,
    pub self_: bool,
}

impl From<CalendarEvent> for CalendarEventResponse {
    fn from(event: CalendarEvent) -> Self {
        let attendees: Vec<CalendarAttendee> = event
            .attendees
            .as_ref()
            .and_then(|a| serde_json::from_str(a).ok())
            .unwrap_or_default();

        CalendarEventResponse {
            id: event.id,
            account_id: event.account_id,
            google_event_id: event.google_event_id,
            summary: event.summary,
            description: event.description,
            location: event.location,
            start_time: event.start_time,
            end_time: event.end_time,
            all_day: event.all_day,
            recurring: event.recurring,
            status: event.status,
            organizer_email: event.organizer_email,
            attendees,
            conference_link: event.conference_link,
            processed: event.processed,
        }
    }
}

/// Query parameters for calendar events
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CalendarEventQuery {
    pub account_id: Option<Uuid>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub processed: Option<bool>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "diesel", derive(diesel::Queryable))]
pub struct Category {
    pub id: Uuid,
    pub name: String,
    pub color: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCategoryRequest {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCategoryRequest {
    pub name: Option<String>,
    pub color: Option<String>,
}

/// API response for emails
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailResponse {
    pub id: Uuid,
    pub account_id: Uuid,
    pub gmail_id: String,
    pub thread_id: String,
    pub subject: String,
    pub from_address: String,
    pub from_name: Option<String>,
    pub to_addresses: Vec<String>,
    pub snippet: Option<String>,
    pub has_attachments: bool,
    pub received_at: DateTime<Utc>,
    pub processed: bool,
    pub archived_in_gmail: bool,
    pub triage_status: String,
}

/// Personal-context document the triage agent reads before proposing actions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AboutMeResponse {
    pub content: String,
    pub updated_at: DateTime<Utc>,
}

/// Request to replace the about-me document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAboutMeRequest {
    pub content: String,
}

/// Request to create an event on the agent calendar (executed by the backend
/// with the account's stored OAuth tokens)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCalendarEventRequest {
    pub account_email: String,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    /// Link back to the source email (Gmail URL)
    pub email_link: Option<String>,
    /// Target calendar name; defaults to "Agent"
    pub calendar_name: Option<String>,
}

/// Result of creating an event on the agent calendar
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCalendarEventResponse {
    pub google_event_id: String,
    pub html_link: Option<String>,
    pub calendar_id: String,
}

/// Calendar event proposed by the triage agent (stored as a gated decision's
/// The action payload of a forward_email decision (stored as JSON in
/// proposed_action; executed on approval). The destination comes from server
/// config, never from the agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProposedForwardAction {
    pub to_address: String,
    pub from_account: String,
    pub subject: String,
}

/// proposed_action; executed on approval)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProposedCalendarEventAction {
    pub account_email: String,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub email_link: Option<String>,
    pub calendar_name: Option<String>,
}

/// Disposition submitted by a triage agent session for one email
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriageDecideAction {
    /// Screening pass flagged this as probably-archivable (final call is a
    /// later determination pass)
    ArchiveCandidate,
    /// Archive now: label agent-archived and remove from inbox (auto-executed)
    Archive,
    /// Deliberately leave in the inbox
    Keep,
    /// Propose a calendar event (gated: lands in the decision inbox)
    Event {
        summary: String,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        description: Option<String>,
        location: Option<String>,
    },
    /// Propose a todo (gated: lands in the decision inbox)
    Todo {
        title: String,
        description: Option<String>,
        due_date: Option<DateTime<Utc>>,
    },
    /// Nothing actionable here
    Ignore,
    /// Send to the action queue for the deeper (Opus) pass
    QueueAction,
    /// Propose forwarding this email (e.g. a receipt to the expense system).
    /// The destination is server policy, never agent-chosen.
    Forward,
}

/// Request body for POST /api/triage/decisions (sent by agent-cli)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriageDecideRequest {
    pub email_id: Uuid,
    pub action: TriageDecideAction,
    pub reasoning: String,
    /// Model that made this call, for the audit trail
    pub model: Option<String>,
}

/// Response for a triage disposition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriageDecideResponse {
    pub email_id: Uuid,
    pub decision_id: Option<Uuid>,
    pub triage_status: String,
    /// Whether a side effect (e.g. Gmail archive) already ran
    pub executed: bool,
}

/// One triage_status bucket count for the pipeline display
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TriageStageCount {
    pub status: String,
    pub count: i64,
}

/// Response to starting the in-app Claude Code login flow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeAuthStartResponse {
    pub auth_url: String,
}

/// Code pasted back by the user to complete the Claude Code login
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeAuthCompleteRequest {
    pub code: String,
}

/// Whether a Claude Code credential is stored for the pipeline
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaudeAuthStatusResponse {
    pub connected: bool,
    pub updated_at: Option<DateTime<Utc>>,
}

/// One archive determination with its email context, for bulk audit
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveReviewItem {
    pub decision_id: Uuid,
    pub email_id: Uuid,
    pub status: String,
    pub confidence: f32,
    pub reasoning: String,
    pub decided_at: DateTime<Utc>,
    pub account_email: String,
    pub subject: String,
    pub from_address: String,
    pub from_name: Option<String>,
    pub received_at: DateTime<Utc>,
    pub snippet: Option<String>,
    /// Leading portion of the plain-text body, for hover preview in review
    pub body_preview: Option<String>,
}

/// Bulk view of archive determinations (dry-run audit surface)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveReviewResponse {
    pub archive_mode: String,
    pub items: Vec<ArchiveReviewItem>,
}

/// Typed pipeline/triage health status (stable shape; consumed by monitoring)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineStatsResponse {
    /// "agentic" or "disabled" (no API key / claude binary)
    pub mode: String,
    pub last_cycle_at: Option<DateTime<Utc>>,
    pub last_cycle_error: Option<String>,
    pub consecutive_failures: i32,
    pub stage_counts: Vec<TriageStageCount>,
}

/// Query parameters for listing emails
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmailListQuery {
    pub account_id: Option<Uuid>,
    pub processed: Option<bool>,
    pub from: Option<String>,
    pub subject: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ============================================================================
// Agent Decision Types
// ============================================================================

/// Source type for agent decisions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionSourceType {
    Email,
    Calendar,
    Manual,
}

impl DecisionSourceType {
    pub fn as_str(&self) -> &str {
        match self {
            DecisionSourceType::Email => "email",
            DecisionSourceType::Calendar => "calendar",
            DecisionSourceType::Manual => "manual",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "email" => Some(DecisionSourceType::Email),
            "calendar" => Some(DecisionSourceType::Calendar),
            "manual" => Some(DecisionSourceType::Manual),
            _ => None,
        }
    }
}

/// Types of decisions the agent can make
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionType {
    CreateTodo,
    Ignore,
    Archive,
    Defer,
    Categorize,
    SetDueDate,
    CreateCalendarEvent,
    ForwardEmail,
}

impl DecisionType {
    pub fn as_str(&self) -> &str {
        match self {
            DecisionType::CreateTodo => "create_todo",
            DecisionType::Ignore => "ignore",
            DecisionType::Archive => "archive",
            DecisionType::Defer => "defer",
            DecisionType::Categorize => "categorize",
            DecisionType::SetDueDate => "set_due_date",
            DecisionType::CreateCalendarEvent => "create_calendar_event",
            DecisionType::ForwardEmail => "forward_email",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "create_todo" => Some(DecisionType::CreateTodo),
            "ignore" => Some(DecisionType::Ignore),
            "archive" => Some(DecisionType::Archive),
            "defer" => Some(DecisionType::Defer),
            "categorize" => Some(DecisionType::Categorize),
            "set_due_date" => Some(DecisionType::SetDueDate),
            "create_calendar_event" => Some(DecisionType::CreateCalendarEvent),
            "forward_email" => Some(DecisionType::ForwardEmail),
            _ => None,
        }
    }
}

/// Status of an agent decision
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionStatus {
    Proposed,
    Approved,
    Rejected,
    AutoApproved,
    Executed,
    Failed,
}

impl DecisionStatus {
    pub fn as_str(&self) -> &str {
        match self {
            DecisionStatus::Proposed => "proposed",
            DecisionStatus::Approved => "approved",
            DecisionStatus::Rejected => "rejected",
            DecisionStatus::AutoApproved => "auto_approved",
            DecisionStatus::Executed => "executed",
            DecisionStatus::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "proposed" => Some(DecisionStatus::Proposed),
            "approved" => Some(DecisionStatus::Approved),
            "rejected" => Some(DecisionStatus::Rejected),
            "auto_approved" => Some(DecisionStatus::AutoApproved),
            "executed" => Some(DecisionStatus::Executed),
            "failed" => Some(DecisionStatus::Failed),
            _ => None,
        }
    }
}

/// Proposed action details for creating a todo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedTodoAction {
    pub todo_title: String,
    pub todo_description: Option<String>,
    pub due_date: Option<DateTime<Utc>>,
    pub category_id: Option<Uuid>,
    pub priority: Option<String>,
}

/// Agent decision record
/// JSON fields stored as strings (no JSONB in database)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "diesel", derive(diesel::Queryable))]
pub struct AgentDecision {
    pub id: Uuid,
    pub source_type: String,
    pub source_id: Option<Uuid>,
    pub source_external_id: Option<String>,
    pub decision_type: String,
    pub proposed_action: String, // JSON string
    pub reasoning: String,
    pub reasoning_details: Option<String>, // JSON string
    pub confidence: f32,
    pub status: String,
    pub result_todo_id: Option<Uuid>,
    pub user_feedback: Option<String>,
    pub created_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub executed_at: Option<DateTime<Utc>>,
}

/// API response for agent decisions (hides internal IDs, adds computed fields)
/// JSON fields are parsed for the API response
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentDecisionResponse {
    pub id: Uuid,
    pub source_type: String,
    pub source_id: Option<Uuid>,
    pub source_external_id: Option<String>,
    pub decision_type: String,
    pub proposed_action: serde_json::Value, // Parsed JSON for API consumers
    pub reasoning: String,
    pub reasoning_details: Option<serde_json::Value>, // Parsed JSON for API consumers
    pub confidence: f32,
    pub confidence_level: String, // "high", "medium", "low"
    pub status: String,
    pub result_todo_id: Option<Uuid>,
    pub user_feedback: Option<String>,
    pub created_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub executed_at: Option<DateTime<Utc>>,
}

impl From<AgentDecision> for AgentDecisionResponse {
    fn from(decision: AgentDecision) -> Self {
        let confidence_level = if decision.confidence >= 0.8 {
            "high"
        } else if decision.confidence >= 0.5 {
            "medium"
        } else {
            "low"
        }
        .to_string();

        // Parse JSON strings for API response
        let proposed_action =
            serde_json::from_str(&decision.proposed_action).unwrap_or(serde_json::Value::Null);
        let reasoning_details = decision
            .reasoning_details
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok());

        AgentDecisionResponse {
            id: decision.id,
            source_type: decision.source_type,
            source_id: decision.source_id,
            source_external_id: decision.source_external_id,
            decision_type: decision.decision_type,
            proposed_action,
            reasoning: decision.reasoning,
            reasoning_details,
            confidence: decision.confidence,
            confidence_level,
            status: decision.status,
            result_todo_id: decision.result_todo_id,
            user_feedback: decision.user_feedback,
            created_at: decision.created_at,
            reviewed_at: decision.reviewed_at,
            executed_at: decision.executed_at,
        }
    }
}

/// Request to create a new agent decision
/// API accepts JSON values which are serialized to strings for storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentDecisionRequest {
    pub source_type: String,
    pub source_id: Option<Uuid>,
    pub source_external_id: Option<String>,
    pub decision_type: String,
    pub proposed_action: serde_json::Value, // Accepts JSON, serialized to string for storage
    pub reasoning: String,
    pub reasoning_details: Option<serde_json::Value>, // Accepts JSON, serialized to string for storage
    pub confidence: f32,
}

/// Request to approve a decision with optional modifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveDecisionRequest {
    pub modifications: Option<ProposedTodoAction>,
}

/// Request to reject a decision with feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectDecisionRequest {
    pub feedback: Option<String>,
}

/// Request to approve multiple decisions at once
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchApproveDecisionsRequest {
    pub decision_ids: Vec<Uuid>,
}

/// Request to reject multiple decisions at once
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRejectDecisionsRequest {
    pub decision_ids: Vec<Uuid>,
    pub feedback: Option<String>,
}

/// Response for batch operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchOperationResponse {
    pub successful: Vec<Uuid>,
    pub failed: Vec<BatchOperationFailure>,
}

/// Details about a failed batch operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchOperationFailure {
    pub id: Uuid,
    pub error: String,
}

/// Statistics about agent decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionStats {
    pub total: i64,
    pub pending: i64,
    pub approved: i64,
    pub rejected: i64,
    pub auto_approved: i64,
    pub average_confidence: f32,
}

// ============================================================================
// Chat Types
// ============================================================================

/// Role of a chat message participant
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatRole {
    User,
    Assistant,
}

impl ChatRole {
    pub fn as_str(&self) -> &str {
        match self {
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "user" => Some(ChatRole::User),
            "assistant" => Some(ChatRole::Assistant),
            _ => None,
        }
    }
}

/// Intent detected from user chat message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatIntent {
    CreateTodo,
    QueryTodos,
    MarkComplete,
    ModifyTodo,
    QueryEmails,
    QueryDecisions,
    ApproveDecision,
    RejectDecision,
    Help,
    General,
}

impl ChatIntent {
    pub fn as_str(&self) -> &str {
        match self {
            ChatIntent::CreateTodo => "create_todo",
            ChatIntent::QueryTodos => "query_todos",
            ChatIntent::MarkComplete => "mark_complete",
            ChatIntent::ModifyTodo => "modify_todo",
            ChatIntent::QueryEmails => "query_emails",
            ChatIntent::QueryDecisions => "query_decisions",
            ChatIntent::ApproveDecision => "approve_decision",
            ChatIntent::RejectDecision => "reject_decision",
            ChatIntent::Help => "help",
            ChatIntent::General => "general",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "create_todo" => Some(ChatIntent::CreateTodo),
            "query_todos" => Some(ChatIntent::QueryTodos),
            "mark_complete" => Some(ChatIntent::MarkComplete),
            "modify_todo" => Some(ChatIntent::ModifyTodo),
            "query_emails" => Some(ChatIntent::QueryEmails),
            "query_decisions" => Some(ChatIntent::QueryDecisions),
            "approve_decision" => Some(ChatIntent::ApproveDecision),
            "reject_decision" => Some(ChatIntent::RejectDecision),
            "help" => Some(ChatIntent::Help),
            "general" => Some(ChatIntent::General),
            _ => None,
        }
    }
}

/// Chat message database model
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "diesel", derive(diesel::Queryable))]
pub struct ChatMessage {
    pub id: Uuid,
    pub role: String,
    pub content: String,
    pub intent: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// API response for chat messages
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessageResponse {
    pub id: Uuid,
    pub role: String,
    pub content: String,
    pub intent: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<ChatMessage> for ChatMessageResponse {
    fn from(msg: ChatMessage) -> Self {
        ChatMessageResponse {
            id: msg.id,
            role: msg.role,
            content: msg.content,
            intent: msg.intent,
            created_at: msg.created_at,
        }
    }
}

/// Request to send a chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendChatMessageRequest {
    pub content: String,
}

/// Response from sending a chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub message: ChatMessageResponse,
    pub detected_intent: Option<String>,
    pub suggested_actions: Vec<SuggestedAction>,
}

/// Suggested action that can be taken from chat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedAction {
    pub label: String,
    pub action_type: String,
    pub payload: serde_json::Value,
}

/// Query parameters for chat history
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatHistoryQuery {
    pub limit: Option<i64>,
    pub before: Option<DateTime<Utc>>,
}

// ============================================================================
// Authentication Types
// ============================================================================

/// Response from /api/auth/me endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUserResponse {
    pub email: String,
    pub name: Option<String>,
}

/// Response from /api/auth/login endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginInitResponse {
    pub auth_url: String,
}

#[cfg(test)]
mod triage_type_tests {
    use super::*;

    #[test]
    fn triage_action_wire_format_is_stable() {
        let json = serde_json::to_value(&TriageDecideAction::ArchiveCandidate).unwrap();
        assert_eq!(json["type"], "archive_candidate");

        let event = TriageDecideAction::Event {
            summary: "Dinner".to_string(),
            start: "2026-08-09T18:00:00Z".parse().unwrap(),
            end: "2026-08-09T19:00:00Z".parse().unwrap(),
            description: None,
            location: Some("SF".to_string()),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "event");
        assert_eq!(json["summary"], "Dinner");

        let parsed: TriageDecideAction = serde_json::from_str(
            r#"{"type":"todo","title":"Reply","description":null,"due_date":null}"#,
        )
        .unwrap();
        assert!(matches!(parsed, TriageDecideAction::Todo { .. }));

        let json = serde_json::to_value(&TriageDecideAction::Forward).unwrap();
        assert_eq!(json["type"], "forward");
    }

    #[test]
    fn forward_decision_type_round_trips() {
        assert_eq!(DecisionType::ForwardEmail.as_str(), "forward_email");
        assert_eq!(
            DecisionType::parse("forward_email"),
            Some(DecisionType::ForwardEmail)
        );
        let action = ProposedForwardAction {
            to_address: "receipts@ramp.com".to_string(),
            from_account: "user@example.com".to_string(),
            subject: "Your receipt".to_string(),
        };
        let parsed: ProposedForwardAction =
            serde_json::from_str(&serde_json::to_string(&action).unwrap()).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn calendar_decision_type_round_trips() {
        assert_eq!(
            DecisionType::CreateCalendarEvent.as_str(),
            "create_calendar_event"
        );
        assert_eq!(
            DecisionType::parse("create_calendar_event"),
            Some(DecisionType::CreateCalendarEvent)
        );
    }
}

// ============================================================================
// Serialization Roundtrip Tests
// ============================================================================
//
// Every type that crosses the backend/frontend boundary gets a roundtrip test,
// and the enums additionally pin their wire form. These enums carry *two*
// string representations that are easy to confuse:
//
//   * serde/JSON uses the Rust variant name verbatim  -> "CreateTodo"
//   * `as_str()`/`parse()` are the database form      -> "create_todo"
//
// Neither is derived from the other, so both are asserted explicitly.

#[cfg(test)]
mod serde_roundtrip_tests {
    use super::*;

    fn roundtrip<T>(value: &T) -> T
    where
        T: Serialize + serde::de::DeserializeOwned,
    {
        let json = serde_json::to_string(value).expect("serialize");
        serde_json::from_str(&json).expect("deserialize")
    }

    fn json_of<T: Serialize>(value: &T) -> String {
        serde_json::to_string(value).expect("serialize")
    }

    // ---- Enums: JSON wire form -------------------------------------------

    #[test]
    fn enum_json_is_the_pascal_case_variant_name() {
        assert_eq!(json_of(&DecisionSourceType::Email), r#""Email""#);
        assert_eq!(json_of(&DecisionType::CreateTodo), r#""CreateTodo""#);
        assert_eq!(json_of(&DecisionStatus::AutoApproved), r#""AutoApproved""#);
        assert_eq!(json_of(&ChatRole::Assistant), r#""Assistant""#);
        assert_eq!(json_of(&ChatIntent::QueryTodos), r#""QueryTodos""#);
        assert_eq!(json_of(&EmailProvider::Gmail), r#""Gmail""#);
    }

    #[test]
    fn enum_db_form_differs_from_the_json_form() {
        assert_eq!(DecisionType::CreateTodo.as_str(), "create_todo");
        assert_eq!(DecisionStatus::AutoApproved.as_str(), "auto_approved");
        assert_eq!(ChatIntent::QueryTodos.as_str(), "query_todos");
    }

    // ---- Enums: serde and as_str/parse roundtrips -------------------------

    #[test]
    fn decision_source_type_roundtrips() {
        for value in [
            DecisionSourceType::Email,
            DecisionSourceType::Calendar,
            DecisionSourceType::Manual,
        ] {
            assert_eq!(roundtrip(&value), value);
            assert_eq!(
                DecisionSourceType::parse(value.as_str()),
                Some(value.clone())
            );
        }
    }

    #[test]
    fn decision_type_roundtrips() {
        for value in [
            DecisionType::CreateTodo,
            DecisionType::Ignore,
            DecisionType::Archive,
            DecisionType::Defer,
            DecisionType::Categorize,
            DecisionType::SetDueDate,
            DecisionType::CreateCalendarEvent,
            DecisionType::ForwardEmail,
        ] {
            assert_eq!(roundtrip(&value), value);
            assert_eq!(DecisionType::parse(value.as_str()), Some(value.clone()));
        }
    }

    #[test]
    fn decision_status_roundtrips() {
        for value in [
            DecisionStatus::Proposed,
            DecisionStatus::Approved,
            DecisionStatus::Rejected,
            DecisionStatus::AutoApproved,
            DecisionStatus::Executed,
            DecisionStatus::Failed,
        ] {
            assert_eq!(roundtrip(&value), value);
            assert_eq!(DecisionStatus::parse(value.as_str()), Some(value));
        }
    }

    #[test]
    fn chat_role_roundtrips() {
        for value in [ChatRole::User, ChatRole::Assistant] {
            assert_eq!(roundtrip(&value), value);
            assert_eq!(ChatRole::parse(value.as_str()), Some(value.clone()));
        }
    }

    #[test]
    fn chat_intent_roundtrips() {
        for value in [
            ChatIntent::CreateTodo,
            ChatIntent::QueryTodos,
            ChatIntent::MarkComplete,
            ChatIntent::ModifyTodo,
            ChatIntent::QueryEmails,
            ChatIntent::QueryDecisions,
            ChatIntent::ApproveDecision,
            ChatIntent::RejectDecision,
            ChatIntent::Help,
            ChatIntent::General,
        ] {
            assert_eq!(roundtrip(&value), value);
            assert_eq!(ChatIntent::parse(value.as_str()), Some(value.clone()));
        }
    }

    #[test]
    fn todo_source_roundtrips() {
        let account_id = Uuid::new_v4();
        for value in [
            TodoSource::Manual,
            TodoSource::Email { account_id },
            TodoSource::Calendar {
                calendar_id: "primary".to_string(),
            },
        ] {
            assert_eq!(roundtrip(&value), value);
        }
    }

    // ---- Structs ----------------------------------------------------------

    #[test]
    fn health_response_roundtrips() {
        let value = HealthResponse {
            status: "ok".to_string(),
        };
        assert_eq!(roundtrip(&value), value);
        assert_eq!(json_of(&value), r#"{"status":"ok"}"#);
    }

    #[test]
    fn todo_roundtrips() {
        let value = Todo {
            id: Uuid::new_v4(),
            title: "Pay the water bill".to_string(),
            description: Some("Due at the end of the month".to_string()),
            completed: false,
            source: "email".to_string(),
            source_id: Some("gmail-123".to_string()),
            due_date: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            link: Some("https://example.com".to_string()),
            category_id: Some(Uuid::new_v4()),
            decision_id: Some(Uuid::new_v4()),
        };
        assert_eq!(roundtrip(&value), value);
    }

    #[test]
    fn create_todo_request_roundtrips() {
        let value = CreateTodoRequest {
            title: "Book the dentist".to_string(),
            description: None,
            due_date: Some(Utc::now()),
            link: None,
            category_id: None,
        };
        let parsed = roundtrip(&value);
        assert_eq!(parsed.title, value.title);
        assert_eq!(parsed.due_date, value.due_date);
    }

    #[test]
    fn update_todo_request_roundtrips() {
        let value = UpdateTodoRequest {
            title: Some("Renamed".to_string()),
            description: None,
            completed: Some(true),
            due_date: None,
            link: None,
            category_id: None,
        };
        let parsed = roundtrip(&value);
        assert_eq!(parsed.title, value.title);
        assert_eq!(parsed.completed, value.completed);
    }

    #[test]
    fn category_roundtrips() {
        let value = Category {
            id: Uuid::new_v4(),
            name: "Finance".to_string(),
            color: Some("#7aa2f7".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert_eq!(roundtrip(&value), value);
    }

    #[test]
    fn create_category_request_roundtrips() {
        let value = CreateCategoryRequest {
            name: "Travel".to_string(),
            color: None,
        };
        let parsed = roundtrip(&value);
        assert_eq!(parsed.name, value.name);
        assert_eq!(parsed.color, value.color);
    }

    #[test]
    fn proposed_todo_action_roundtrips() {
        let value = ProposedTodoAction {
            todo_title: "Renew passport".to_string(),
            todo_description: Some("Expires in March".to_string()),
            due_date: Some(Utc::now()),
            category_id: Some(Uuid::new_v4()),
            priority: Some("high".to_string()),
        };
        let parsed = roundtrip(&value);
        assert_eq!(parsed.todo_title, value.todo_title);
        assert_eq!(parsed.priority, value.priority);
    }

    #[test]
    fn decision_stats_roundtrips() {
        let value = DecisionStats {
            total: 100,
            pending: 13,
            approved: 60,
            rejected: 20,
            auto_approved: 7,
            average_confidence: 0.82,
        };
        let parsed = roundtrip(&value);
        assert_eq!(parsed.total, value.total);
        assert_eq!(parsed.average_confidence, value.average_confidence);
    }

    #[test]
    fn batch_operation_response_roundtrips() {
        let value = BatchOperationResponse {
            successful: vec![Uuid::new_v4()],
            failed: vec![BatchOperationFailure {
                id: Uuid::new_v4(),
                error: "already executed".to_string(),
            }],
        };
        let parsed = roundtrip(&value);
        assert_eq!(parsed.successful, value.successful);
        assert_eq!(parsed.failed.len(), 1);
        assert_eq!(parsed.failed[0].error, "already executed");
    }

    #[test]
    fn calendar_attendee_roundtrips() {
        let value = CalendarAttendee {
            email: "someone@example.com".to_string(),
            display_name: Some("Someone".to_string()),
            response_status: Some("accepted".to_string()),
            organizer: false,
            self_: true,
        };
        let parsed = roundtrip(&value);
        assert_eq!(parsed.email, value.email);
        assert!(parsed.self_);
        // No serde rename, so the trailing underscore is part of the wire form.
        assert!(json_of(&value).contains(r#""self_":true"#));
    }

    #[test]
    fn email_list_query_roundtrips() {
        let value = EmailListQuery {
            account_id: Some(Uuid::new_v4()),
            processed: Some(false),
            from: Some("bank@example.com".to_string()),
            subject: None,
            since: Some(Utc::now()),
            until: None,
            limit: Some(50),
            offset: Some(0),
        };
        let parsed = roundtrip(&value);
        assert_eq!(parsed.account_id, value.account_id);
        assert_eq!(parsed.limit, value.limit);
    }

    #[test]
    fn send_chat_message_request_roundtrips() {
        let value = SendChatMessageRequest {
            content: "what is pending?".to_string(),
        };
        let parsed = roundtrip(&value);
        assert_eq!(parsed.content, value.content);
    }

    #[test]
    fn about_me_roundtrips() {
        let value = AboutMeResponse {
            content: "Two Gmail accounts, overwhelmed by life admin.".to_string(),
            updated_at: Utc::now(),
        };
        assert_eq!(roundtrip(&value), value);

        let request = UpdateAboutMeRequest {
            content: "Updated context".to_string(),
        };
        assert_eq!(roundtrip(&request).content, request.content);
    }

    #[test]
    fn create_calendar_event_request_roundtrips() {
        let value = CreateCalendarEventRequest {
            account_email: "matt@example.com".to_string(),
            summary: "Dentist".to_string(),
            description: Some("Six month cleaning".to_string()),
            location: Some("123 Main St".to_string()),
            start: Utc::now(),
            end: Utc::now(),
            email_link: Some("https://mail.google.com/mail/u/0/#inbox/abc".to_string()),
            calendar_name: None,
        };
        let parsed = roundtrip(&value);
        assert_eq!(parsed.account_email, value.account_email);
        assert_eq!(parsed.summary, value.summary);
        assert_eq!(parsed.email_link, value.email_link);
        assert_eq!(parsed.calendar_name, None);
    }

    #[test]
    fn create_calendar_event_response_roundtrips() {
        let value = CreateCalendarEventResponse {
            google_event_id: "evt-123".to_string(),
            html_link: Some("https://calendar.google.com/event?eid=abc".to_string()),
            calendar_id: "agent@group.calendar.google.com".to_string(),
        };
        let parsed = roundtrip(&value);
        assert_eq!(parsed.google_event_id, value.google_event_id);
        assert_eq!(parsed.calendar_id, value.calendar_id);
    }

    #[test]
    fn claude_auth_start_response_roundtrips() {
        let value = ClaudeAuthStartResponse {
            auth_url: "https://claude.ai/oauth/authorize?code=abc".to_string(),
        };
        let parsed = roundtrip(&value);
        assert_eq!(parsed.auth_url, value.auth_url);
    }

    #[test]
    fn claude_auth_complete_request_roundtrips() {
        let value = ClaudeAuthCompleteRequest {
            code: "pasted-code-123".to_string(),
        };
        let parsed = roundtrip(&value);
        assert_eq!(parsed.code, value.code);
    }

    #[test]
    fn claude_auth_status_response_roundtrips() {
        let value = ClaudeAuthStatusResponse {
            connected: true,
            updated_at: Some(Utc::now()),
        };
        let parsed = roundtrip(&value);
        assert_eq!(parsed.connected, value.connected);
        assert!(parsed.updated_at.is_some());
    }
}
