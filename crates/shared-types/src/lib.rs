use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
#[cfg_attr(feature = "diesel", derive(diesel::Queryable))]
pub struct EmailAccount {
    pub id: Uuid,
    pub account_name: String,
    pub email_address: String,
    pub provider: String,
    pub last_synced: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub oauth_refresh_token: Option<String>,
    pub oauth_access_token: Option<String>,
    pub oauth_token_expires_at: Option<DateTime<Utc>>,
    pub last_message_id: Option<String>,
    pub sync_status: String,
    pub last_sync_error: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmailProvider {
    Gmail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncStatus {
    Pending,
    Syncing,
    Success,
    Failed,
    AuthRequired,
}

impl SyncStatus {
    pub fn as_str(&self) -> &str {
        match self {
            SyncStatus::Pending => "pending",
            SyncStatus::Syncing => "syncing",
            SyncStatus::Success => "success",
            SyncStatus::Failed => "failed",
            SyncStatus::AuthRequired => "auth_required",
        }
    }
}

// API Request/Response types for email account management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAccountResponse {
    pub id: Uuid,
    pub account_name: String,
    pub email_address: String,
    pub provider: String,
    pub last_synced: Option<DateTime<Utc>>,
    pub sync_status: String,
    pub last_sync_error: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl From<EmailAccount> for EmailAccountResponse {
    fn from(account: EmailAccount) -> Self {
        EmailAccountResponse {
            id: account.id,
            account_name: account.account_name,
            email_address: account.email_address,
            provider: account.provider,
            last_synced: account.last_synced,
            sync_status: account.sync_status,
            last_sync_error: account.last_sync_error,
            is_active: account.is_active,
            created_at: account.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectEmailAccountRequest {
    pub account_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthCallbackRequest {
    pub code: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarAccount {
    pub id: Uuid,
    pub account_name: String,
    pub calendar_id: String,
    pub last_synced: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
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

    pub fn from_str(s: &str) -> Option<Self> {
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
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "create_todo" => Some(DecisionType::CreateTodo),
            "ignore" => Some(DecisionType::Ignore),
            "archive" => Some(DecisionType::Archive),
            "defer" => Some(DecisionType::Defer),
            "categorize" => Some(DecisionType::Categorize),
            "set_due_date" => Some(DecisionType::SetDueDate),
            _ => None,
        }
    }
}

/// Status of an agent decision
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

    pub fn from_str(s: &str) -> Option<Self> {
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

/// Structured reasoning details for audit trail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningDetails {
    pub matched_keywords: Option<Vec<String>>,
    pub detected_deadline: Option<String>,
    pub sender_frequency: Option<i32>,
    pub thread_length: Option<i32>,
    pub heuristic_score: Option<f32>,
    pub llm_analysis: Option<String>,
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
    pub applied_rule_id: Option<Uuid>,
    pub result_todo_id: Option<Uuid>,
    pub user_feedback: Option<String>,
    pub created_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub executed_at: Option<DateTime<Utc>>,
}

/// API response for agent decisions (hides internal IDs, adds computed fields)
/// JSON fields are parsed for the API response
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub create_rule: Option<bool>,
    pub rule_name: Option<String>,
}

/// Request to reject a decision with feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectDecisionRequest {
    pub feedback: Option<String>,
    pub create_rule: Option<bool>,
    pub rule_action: Option<String>, // "ignore", "archive"
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
