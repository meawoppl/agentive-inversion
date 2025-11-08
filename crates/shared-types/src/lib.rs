use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "diesel", derive(diesel::Queryable))]
pub struct Todo {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub completed: bool,
    pub source: TodoSource,
    pub source_id: Option<String>,
    pub due_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTodoRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub completed: Option<bool>,
    pub due_date: Option<DateTime<Utc>>,
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
