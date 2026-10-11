//! Versioned connector contracts. Project knowledge and tasks live in Git.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairRequest {
    pub name: String,
    #[serde(default)]
    pub review: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairResponse {
    pub id: Uuid,
    pub device_secret: String,
    pub verification_url: String,
    pub expires_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PollRequest {
    pub device_secret: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PollResponse {
    pub status: String,
    pub token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PairInfo {
    pub id: Uuid,
    pub name: String,
    pub review: bool,
    pub expires_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovePair {
    pub account_ids: Vec<Uuid>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ConnectionInfo {
    pub id: Uuid,
    pub name: String,
    pub owner_email: String,
    pub account_ids: Vec<Uuid>,
    pub scopes: Vec<String>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Capabilities {
    pub api_version: String,
    pub mode: String,
    pub scopes: Vec<String>,
    pub account_ids: Vec<Uuid>,
    pub max_claim: u32,
    pub lease_seconds: u32,
    pub approval_required: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Source {
    pub id: Uuid,
    pub account_id: Uuid,
    pub account_email: String,
    pub gmail_id: String,
    pub thread_id: String,
    pub gmail_link: String,
    pub subject: String,
    pub from_address: String,
    pub received_at: DateTime<Utc>,
    pub snippet: Option<String>,
    pub body_text: Option<String>,
    pub archived: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourcePage {
    pub items: Vec<Source>,
    pub next_cursor: Option<Uuid>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimRequest {
    #[serde(default = "default_limit")]
    pub limit: u32,
}
fn default_limit() -> u32 {
    20
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClaimResponse {
    pub claim_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub items: Vec<Source>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompleteRequest {
    pub email_id: Uuid,
    pub disposition: String,
    pub repository_commit: String,
    pub repository_path: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    Archive,
    ForwardReceipt,
    CreateCalendarEvent {
        summary: String,
        description: Option<String>,
        location: Option<String>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    },
}
impl Effect {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Archive => "archive",
            Self::ForwardReceipt => "forward_receipt",
            Self::CreateCalendarEvent { .. } => "create_calendar_event",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ActionRequest {
    pub email_id: Uuid,
    pub claim_id: Uuid,
    pub reason: String,
    pub repository_commit: String,
    pub repository_path: String,
    pub effect: Effect,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ActionInfo {
    pub id: Uuid,
    pub connection_id: Uuid,
    pub email_id: Uuid,
    pub kind: String,
    pub status: String,
    pub request: ActionRequest,
    pub approved_by: Option<String>,
    pub forward_to: Option<String>,
    pub receipt: Option<Receipt>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Receipt {
    #[serde(default)]
    pub reused_existing: bool,
    pub provider_id: Option<String>,
    pub calendar_id: Option<String>,
    pub forwarded: bool,
    pub archived: bool,
}
