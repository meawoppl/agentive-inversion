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
