use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::models::{Priority, SourceType, TodoStatus};

// ============================================================================
// Todo API Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateTodoRequest {
    #[validate(length(min = 1, max = 500))]
    pub title: String,

    #[validate(length(max = 5000))]
    pub description: Option<String>,

    pub due_date: Option<DateTime<Utc>>,
    pub priority: Option<Priority>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateTodoRequest {
    #[validate(length(min = 1, max = 500))]
    pub title: Option<String>,

    #[validate(length(max = 5000))]
    pub description: Option<String>,

    pub due_date: Option<DateTime<Utc>>,
    pub priority: Option<Priority>,
    pub status: Option<TodoStatus>,
    pub completed: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TodoResponse {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub source_type: SourceType,
    pub source_id: Option<String>,
    pub source_url: Option<String>,
    pub due_date: Option<DateTime<Utc>>,
    pub priority: Priority,
    pub status: TodoStatus,
    pub completed: bool,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListTodosResponse {
    pub todos: Vec<TodoResponse>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
}

#[derive(Debug, Deserialize)]
pub struct ListTodosQuery {
    pub page: Option<usize>,
    pub per_page: Option<usize>,
    pub status: Option<TodoStatus>,
    pub source_type: Option<SourceType>,
    pub completed: Option<bool>,
}

// ============================================================================
// Source API Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateGmailSourceRequest {
    #[validate(length(min = 1, max = 200))]
    pub name: String,

    #[validate(email)]
    pub email: String,

    pub auth_code: String,
    pub polling_interval_seconds: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateCalendarSourceRequest {
    #[validate(length(min = 1, max = 200))]
    pub name: String,

    #[validate(email)]
    pub email: String,

    pub calendar_id: String,
    pub auth_code: String,
    pub polling_interval_seconds: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SourceResponse {
    pub id: Uuid,
    pub source_type: SourceType,
    pub name: String,
    pub email: Option<String>,
    pub polling_interval_seconds: i32,
    pub last_polled_at: Option<DateTime<Utc>>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListSourcesResponse {
    pub sources: Vec<SourceResponse>,
    pub total: usize,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateSourceRequest {
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,

    pub polling_interval_seconds: Option<i32>,
    pub enabled: Option<bool>,
}

// ============================================================================
// Sync API Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct TriggerSyncRequest {
    pub source_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TriggerSyncResponse {
    pub triggered: bool,
    pub message: String,
    pub source_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncStatusResponse {
    pub sources: Vec<SyncSourceStatus>,
    pub overall_status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncSourceStatus {
    pub source_id: Uuid,
    pub source_name: String,
    pub source_type: SourceType,
    pub last_sync: Option<DateTime<Utc>>,
    pub next_sync: Option<DateTime<Utc>>,
    pub status: String,
    pub error: Option<String>,
}

// ============================================================================
// Auth API Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct GoogleAuthUrlResponse {
    pub auth_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GoogleAuthCallbackQuery {
    pub code: String,
    pub state: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub details: Option<String>,
}

impl ErrorResponse {
    pub fn new(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(
        error: impl Into<String>,
        message: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
            details: Some(details.into()),
        }
    }
}
