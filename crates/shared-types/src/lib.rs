use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

// ============================================================================
// Regex Cache for Rule Matching
// ============================================================================

/// Thread-safe cache for compiled regex patterns
static REGEX_CACHE: std::sync::LazyLock<Mutex<HashMap<String, regex::Regex>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Get a cached regex or compile and cache it
fn get_cached_regex(pattern: &str) -> Result<regex::Regex, regex::Error> {
    let mut cache = REGEX_CACHE.lock().unwrap();

    if let Some(re) = cache.get(pattern) {
        return Ok(re.clone());
    }

    let re = regex::Regex::new(pattern)?;
    cache.insert(pattern.to_string(), re.clone());
    Ok(re)
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

/// Calendar account with OAuth fields
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "diesel", derive(diesel::Queryable))]
pub struct CalendarAccount {
    pub id: Uuid,
    pub account_name: String,
    pub calendar_id: String,
    pub last_synced: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub email_address: Option<String>,
    pub oauth_refresh_token: Option<String>,
    pub oauth_access_token: Option<String>,
    pub oauth_token_expires_at: Option<DateTime<Utc>>,
    pub sync_token: Option<String>,
    pub sync_status: String,
    pub last_sync_error: Option<String>,
    pub is_active: bool,
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

    pub fn parse(s: &str) -> Option<Self> {
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
// Agent Rule Types
// ============================================================================

/// Source type for rules (what kind of items the rule applies to)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleSourceType {
    Email,
    Calendar,
    Any,
}

impl RuleSourceType {
    pub fn as_str(&self) -> &str {
        match self {
            RuleSourceType::Email => "email",
            RuleSourceType::Calendar => "calendar",
            RuleSourceType::Any => "any",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "email" => Some(RuleSourceType::Email),
            "calendar" => Some(RuleSourceType::Calendar),
            "any" => Some(RuleSourceType::Any),
            _ => None,
        }
    }
}

/// Type of matching the rule performs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleType {
    ExactMatch,
    Contains,
    Regex,
    Sender,
    Label,
    TimeBased,
}

impl RuleType {
    pub fn as_str(&self) -> &str {
        match self {
            RuleType::ExactMatch => "exact_match",
            RuleType::Contains => "contains",
            RuleType::Regex => "regex",
            RuleType::Sender => "sender",
            RuleType::Label => "label",
            RuleType::TimeBased => "time_based",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "exact_match" => Some(RuleType::ExactMatch),
            "contains" => Some(RuleType::Contains),
            "regex" => Some(RuleType::Regex),
            "sender" => Some(RuleType::Sender),
            "label" => Some(RuleType::Label),
            "time_based" => Some(RuleType::TimeBased),
            _ => None,
        }
    }
}

/// Action the rule takes when matched
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleAction {
    CreateTodo,
    Ignore,
    Archive,
    Categorize,
    SetDueDate,
    Defer,
}

impl RuleAction {
    pub fn as_str(&self) -> &str {
        match self {
            RuleAction::CreateTodo => "create_todo",
            RuleAction::Ignore => "ignore",
            RuleAction::Archive => "archive",
            RuleAction::Categorize => "categorize",
            RuleAction::SetDueDate => "set_due_date",
            RuleAction::Defer => "defer",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "create_todo" => Some(RuleAction::CreateTodo),
            "ignore" => Some(RuleAction::Ignore),
            "archive" => Some(RuleAction::Archive),
            "categorize" => Some(RuleAction::Categorize),
            "set_due_date" => Some(RuleAction::SetDueDate),
            "defer" => Some(RuleAction::Defer),
            _ => None,
        }
    }
}

/// A single condition clause in a rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleConditionClause {
    pub field: String,   // "from_address", "subject", "body", "labels", etc.
    pub matcher: String, // "equals", "contains", "regex", "starts_with", "ends_with"
    pub value: String,   // The pattern to match
    #[serde(default)]
    pub case_sensitive: bool,
}

/// Rule conditions with logical operator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleConditions {
    pub operator: String, // "AND" or "OR"
    pub clauses: Vec<RuleConditionClause>,
}

/// Action parameters for rules (varies by action type)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleActionParams {
    pub todo_title: Option<String>,
    pub todo_description: Option<String>,
    pub category_id: Option<Uuid>,
    pub due_date_offset_days: Option<i32>,
    pub priority: Option<String>,
}

/// Agent rule record (database model)
/// JSON fields stored as strings (no JSONB in database)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "diesel", derive(diesel::Queryable))]
pub struct AgentRule {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub source_type: String,
    pub rule_type: String,
    pub conditions: String, // JSON string
    pub action: String,
    pub action_params: Option<String>, // JSON string
    pub priority: i32,
    pub is_active: bool,
    pub created_from_decision_id: Option<Uuid>,
    pub match_count: i32,
    pub last_matched_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// API response for agent rules (parses JSON fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRuleResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub source_type: String,
    pub rule_type: String,
    pub conditions: RuleConditions,
    pub action: String,
    pub action_params: Option<RuleActionParams>,
    pub priority: i32,
    pub is_active: bool,
    pub created_from_decision_id: Option<Uuid>,
    pub match_count: i32,
    pub last_matched_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<AgentRule> for AgentRuleResponse {
    type Error = serde_json::Error;

    fn try_from(rule: AgentRule) -> Result<Self, Self::Error> {
        let conditions: RuleConditions = serde_json::from_str(&rule.conditions)?;
        let action_params: Option<RuleActionParams> = rule
            .action_params
            .as_ref()
            .map(|s| serde_json::from_str(s))
            .transpose()?;

        Ok(AgentRuleResponse {
            id: rule.id,
            name: rule.name,
            description: rule.description,
            source_type: rule.source_type,
            rule_type: rule.rule_type,
            conditions,
            action: rule.action,
            action_params,
            priority: rule.priority,
            is_active: rule.is_active,
            created_from_decision_id: rule.created_from_decision_id,
            match_count: rule.match_count,
            last_matched_at: rule.last_matched_at,
            created_at: rule.created_at,
            updated_at: rule.updated_at,
        })
    }
}

/// Request to create a new agent rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentRuleRequest {
    pub name: String,
    pub description: Option<String>,
    pub source_type: String,
    pub rule_type: String,
    pub conditions: RuleConditions,
    pub action: String,
    pub action_params: Option<RuleActionParams>,
    pub priority: Option<i32>,
    pub is_active: Option<bool>,
    pub created_from_decision_id: Option<Uuid>,
}

/// Request to update an agent rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAgentRuleRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub source_type: Option<String>,
    pub rule_type: Option<String>,
    pub conditions: Option<RuleConditions>,
    pub action: Option<String>,
    pub action_params: Option<RuleActionParams>,
    pub priority: Option<i32>,
    pub is_active: Option<bool>,
}

/// Query parameters for listing rules
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuleListQuery {
    pub source_type: Option<String>,
    pub rule_type: Option<String>,
    pub is_active: Option<bool>,
}

// ============================================================================
// Rule Matching Engine
// ============================================================================

/// Input data for rule matching against emails
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailMatchInput {
    pub from_address: String,
    pub from_name: Option<String>,
    pub subject: String,
    pub body_text: Option<String>,
    pub snippet: Option<String>,
    pub labels: Vec<String>,
    pub to_addresses: Vec<String>,
    pub cc_addresses: Vec<String>,
}

/// Result of matching a rule against input
#[derive(Debug, Clone)]
pub struct RuleMatchResult {
    pub rule_id: Uuid,
    pub rule_name: String,
    pub matched: bool,
    pub matched_clauses: Vec<String>,
    pub action: String,
    pub action_params: Option<RuleActionParams>,
    pub priority: i32,
}

/// Rule matching engine for evaluating rules against input data
pub struct RuleEngine;

impl RuleEngine {
    /// Evaluate all rules against email input, returning matching rules sorted by priority (highest first)
    pub fn match_email(rules: &[AgentRule], input: &EmailMatchInput) -> Vec<RuleMatchResult> {
        let mut results: Vec<RuleMatchResult> = rules
            .iter()
            .filter_map(|rule| {
                // Parse conditions from JSON string
                let conditions: RuleConditions = match serde_json::from_str(&rule.conditions) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!("Failed to parse rule conditions for {}: {}", rule.id, e);
                        return None;
                    }
                };

                let action_params: Option<RuleActionParams> = rule
                    .action_params
                    .as_ref()
                    .and_then(|s| serde_json::from_str(s).ok());

                let (matched, matched_clauses) = Self::evaluate_conditions(&conditions, input);

                Some(RuleMatchResult {
                    rule_id: rule.id,
                    rule_name: rule.name.clone(),
                    matched,
                    matched_clauses,
                    action: rule.action.clone(),
                    action_params,
                    priority: rule.priority,
                })
            })
            .filter(|r| r.matched)
            .collect();

        // Sort by priority (highest first)
        results.sort_by(|a, b| b.priority.cmp(&a.priority));
        results
    }

    /// Evaluate rule conditions against email input
    fn evaluate_conditions(
        conditions: &RuleConditions,
        input: &EmailMatchInput,
    ) -> (bool, Vec<String>) {
        let mut matched_clauses = Vec::new();
        let is_and = conditions.operator.to_uppercase() == "AND";

        for clause in &conditions.clauses {
            let field_value = Self::get_field_value(&clause.field, input);
            let matches = Self::evaluate_clause(clause, &field_value);

            if matches {
                matched_clauses.push(format!(
                    "{} {} '{}'",
                    clause.field, clause.matcher, clause.value
                ));
            }

            // Short-circuit evaluation
            if is_and && !matches {
                return (false, matched_clauses);
            }
            if !is_and && matches {
                return (true, matched_clauses);
            }
        }

        // For AND: all must match (if we get here, all matched)
        // For OR: none matched (if we get here, none matched)
        (is_and, matched_clauses)
    }

    /// Get the value of a field from the email input
    fn get_field_value(field: &str, input: &EmailMatchInput) -> String {
        match field {
            "from_address" | "from" | "sender" => input.from_address.clone(),
            "from_name" | "sender_name" => input.from_name.clone().unwrap_or_default(),
            "subject" => input.subject.clone(),
            "body" | "body_text" => input.body_text.clone().unwrap_or_default(),
            "snippet" => input.snippet.clone().unwrap_or_default(),
            "to" | "to_addresses" => input.to_addresses.join(", "),
            "cc" | "cc_addresses" => input.cc_addresses.join(", "),
            "labels" => input.labels.join(", "),
            // Combined fields for easier matching
            "from_full" => {
                let name = input.from_name.clone().unwrap_or_default();
                if name.is_empty() {
                    input.from_address.clone()
                } else {
                    format!("{} <{}>", name, input.from_address)
                }
            }
            "content" => {
                // Combined subject + body for broad content matching
                let body = input.body_text.clone().unwrap_or_default();
                format!("{} {}", input.subject, body)
            }
            _ => String::new(),
        }
    }

    /// Evaluate a single clause against a field value
    fn evaluate_clause(clause: &RuleConditionClause, field_value: &str) -> bool {
        let (value, pattern) = if clause.case_sensitive {
            (field_value.to_string(), clause.value.clone())
        } else {
            (field_value.to_lowercase(), clause.value.to_lowercase())
        };

        match clause.matcher.as_str() {
            "equals" | "exact" => value == pattern,
            "contains" => value.contains(&pattern),
            "starts_with" => value.starts_with(&pattern),
            "ends_with" => value.ends_with(&pattern),
            "regex" => {
                match get_cached_regex(&clause.value) {
                    Ok(re) => re.is_match(field_value), // Use original value for regex
                    Err(e) => {
                        tracing::warn!("Invalid regex pattern '{}': {}", clause.value, e);
                        false
                    }
                }
            }
            "not_contains" => !value.contains(&pattern),
            "not_equals" => value != pattern,
            "is_empty" => value.is_empty(),
            "is_not_empty" => !value.is_empty(),
            _ => {
                tracing::warn!("Unknown matcher type: {}", clause.matcher);
                false
            }
        }
    }

    /// Get the highest priority matching rule (if any)
    pub fn get_best_match(rules: &[AgentRule], input: &EmailMatchInput) -> Option<RuleMatchResult> {
        Self::match_email(rules, input).into_iter().next()
    }
}

/// Build a ProposedTodoAction from rule action params and email input
pub fn build_todo_action_from_rule(
    params: &Option<RuleActionParams>,
    input: &EmailMatchInput,
) -> ProposedTodoAction {
    let default_title = if input.subject.len() > 100 {
        format!("{}...", &input.subject[..97])
    } else {
        input.subject.clone()
    };

    let default_description = format!(
        "{}\n\n---\nFrom: {} <{}>",
        input.snippet.clone().unwrap_or_default(),
        input.from_name.clone().unwrap_or_default(),
        input.from_address
    );

    match params {
        Some(p) => ProposedTodoAction {
            todo_title: p.todo_title.clone().unwrap_or(default_title),
            todo_description: p.todo_description.clone().or(Some(default_description)),
            due_date: None, // TODO: Calculate from due_date_offset_days
            category_id: p.category_id,
            priority: p.priority.clone(),
        },
        None => ProposedTodoAction {
            todo_title: default_title,
            todo_description: Some(default_description),
            due_date: None,
            category_id: None,
            priority: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_email() -> EmailMatchInput {
        EmailMatchInput {
            from_address: "sender@example.com".to_string(),
            from_name: Some("John Sender".to_string()),
            subject: "Action Required: Please review the document".to_string(),
            body_text: Some("Please review the attached document by Friday.".to_string()),
            snippet: Some("Please review the attached...".to_string()),
            labels: vec!["INBOX".to_string(), "IMPORTANT".to_string()],
            to_addresses: vec!["me@example.com".to_string()],
            cc_addresses: vec![],
        }
    }

    fn make_test_rule(
        name: &str,
        conditions: RuleConditions,
        action: &str,
        priority: i32,
    ) -> AgentRule {
        AgentRule {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: None,
            source_type: "email".to_string(),
            rule_type: "contains".to_string(),
            conditions: serde_json::to_string(&conditions).unwrap(),
            action: action.to_string(),
            action_params: None,
            priority,
            is_active: true,
            created_from_decision_id: None,
            match_count: 0,
            last_matched_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_contains_matcher() {
        let input = make_test_email();
        let conditions = RuleConditions {
            operator: "AND".to_string(),
            clauses: vec![RuleConditionClause {
                field: "subject".to_string(),
                matcher: "contains".to_string(),
                value: "action required".to_string(),
                case_sensitive: false,
            }],
        };

        let rule = make_test_rule("Test Rule", conditions, "create_todo", 10);
        let results = RuleEngine::match_email(&[rule], &input);

        assert_eq!(results.len(), 1);
        assert!(results[0].matched);
    }

    #[test]
    fn test_sender_matcher() {
        let input = make_test_email();
        let conditions = RuleConditions {
            operator: "AND".to_string(),
            clauses: vec![RuleConditionClause {
                field: "from_address".to_string(),
                matcher: "ends_with".to_string(),
                value: "@example.com".to_string(),
                case_sensitive: false,
            }],
        };

        let rule = make_test_rule("Sender Rule", conditions, "ignore", 5);
        let results = RuleEngine::match_email(&[rule], &input);

        assert_eq!(results.len(), 1);
        assert!(results[0].matched);
        assert_eq!(results[0].action, "ignore");
    }

    #[test]
    fn test_and_operator() {
        let input = make_test_email();
        let conditions = RuleConditions {
            operator: "AND".to_string(),
            clauses: vec![
                RuleConditionClause {
                    field: "subject".to_string(),
                    matcher: "contains".to_string(),
                    value: "action required".to_string(),
                    case_sensitive: false,
                },
                RuleConditionClause {
                    field: "from_address".to_string(),
                    matcher: "contains".to_string(),
                    value: "example.com".to_string(),
                    case_sensitive: false,
                },
            ],
        };

        let rule = make_test_rule("AND Rule", conditions, "create_todo", 10);
        let results = RuleEngine::match_email(&[rule], &input);

        assert_eq!(results.len(), 1);
        assert!(results[0].matched);
    }

    #[test]
    fn test_and_operator_fails_partial() {
        let input = make_test_email();
        let conditions = RuleConditions {
            operator: "AND".to_string(),
            clauses: vec![
                RuleConditionClause {
                    field: "subject".to_string(),
                    matcher: "contains".to_string(),
                    value: "action required".to_string(),
                    case_sensitive: false,
                },
                RuleConditionClause {
                    field: "from_address".to_string(),
                    matcher: "contains".to_string(),
                    value: "notfound.com".to_string(),
                    case_sensitive: false,
                },
            ],
        };

        let rule = make_test_rule("AND Rule", conditions, "create_todo", 10);
        let results = RuleEngine::match_email(&[rule], &input);

        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_or_operator() {
        let input = make_test_email();
        let conditions = RuleConditions {
            operator: "OR".to_string(),
            clauses: vec![
                RuleConditionClause {
                    field: "subject".to_string(),
                    matcher: "contains".to_string(),
                    value: "notfound".to_string(),
                    case_sensitive: false,
                },
                RuleConditionClause {
                    field: "from_address".to_string(),
                    matcher: "contains".to_string(),
                    value: "example.com".to_string(),
                    case_sensitive: false,
                },
            ],
        };

        let rule = make_test_rule("OR Rule", conditions, "archive", 10);
        let results = RuleEngine::match_email(&[rule], &input);

        assert_eq!(results.len(), 1);
        assert!(results[0].matched);
    }

    #[test]
    fn test_priority_sorting() {
        let input = make_test_email();

        let conditions1 = RuleConditions {
            operator: "AND".to_string(),
            clauses: vec![RuleConditionClause {
                field: "subject".to_string(),
                matcher: "contains".to_string(),
                value: "action".to_string(),
                case_sensitive: false,
            }],
        };

        let conditions2 = RuleConditions {
            operator: "AND".to_string(),
            clauses: vec![RuleConditionClause {
                field: "subject".to_string(),
                matcher: "contains".to_string(),
                value: "required".to_string(),
                case_sensitive: false,
            }],
        };

        let rule1 = make_test_rule("Low Priority", conditions1, "ignore", 5);
        let rule2 = make_test_rule("High Priority", conditions2, "create_todo", 100);

        let results = RuleEngine::match_email(&[rule1, rule2], &input);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].rule_name, "High Priority");
        assert_eq!(results[0].priority, 100);
        assert_eq!(results[1].rule_name, "Low Priority");
        assert_eq!(results[1].priority, 5);
    }

    #[test]
    fn test_regex_matcher() {
        let input = make_test_email();
        let conditions = RuleConditions {
            operator: "AND".to_string(),
            clauses: vec![RuleConditionClause {
                field: "from_address".to_string(),
                matcher: "regex".to_string(),
                value: r".*@example\.com$".to_string(),
                case_sensitive: false,
            }],
        };

        let rule = make_test_rule("Regex Rule", conditions, "create_todo", 10);
        let results = RuleEngine::match_email(&[rule], &input);

        assert_eq!(results.len(), 1);
        assert!(results[0].matched);
    }

    #[test]
    fn test_case_sensitive() {
        let input = make_test_email();

        // Case insensitive should match
        let conditions1 = RuleConditions {
            operator: "AND".to_string(),
            clauses: vec![RuleConditionClause {
                field: "subject".to_string(),
                matcher: "contains".to_string(),
                value: "ACTION REQUIRED".to_string(),
                case_sensitive: false,
            }],
        };

        // Case sensitive should not match (subject has different case)
        let conditions2 = RuleConditions {
            operator: "AND".to_string(),
            clauses: vec![RuleConditionClause {
                field: "subject".to_string(),
                matcher: "contains".to_string(),
                value: "ACTION REQUIRED".to_string(),
                case_sensitive: true,
            }],
        };

        let rule1 = make_test_rule("Insensitive", conditions1, "create_todo", 10);
        let rule2 = make_test_rule("Sensitive", conditions2, "create_todo", 10);

        let results1 = RuleEngine::match_email(&[rule1], &input);
        let results2 = RuleEngine::match_email(&[rule2], &input);

        assert_eq!(results1.len(), 1);
        assert_eq!(results2.len(), 0);
    }

    #[test]
    fn test_regex_caching() {
        // Test that the same regex pattern is cached and reused
        let input = make_test_email();
        let conditions = RuleConditions {
            operator: "AND".to_string(),
            clauses: vec![RuleConditionClause {
                field: "from_address".to_string(),
                matcher: "regex".to_string(),
                value: r".*@example\.com$".to_string(),
                case_sensitive: false,
            }],
        };

        let rule = make_test_rule("Regex Cache Test", conditions.clone(), "create_todo", 10);

        // Run multiple times to exercise cache
        for _ in 0..10 {
            let results = RuleEngine::match_email(std::slice::from_ref(&rule), &input);
            assert_eq!(results.len(), 1);
            assert!(results[0].matched);
        }

        // Verify cache contains the pattern
        let cache = REGEX_CACHE.lock().unwrap();
        assert!(cache.contains_key(r".*@example\.com$"));
    }
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
