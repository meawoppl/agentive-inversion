// Diesel QueryableByName generates explicit field initializers; Clippy 1.99
// reports those expansion spans as redundant fields on these declarations.
#![allow(clippy::redundant_field_names)]

use chrono::{DateTime, Utc};
use diesel::{sql_types::*, QueryableByName};
use shared_types::connector::{ActionInfo, ConnectionInfo};
use uuid::Uuid as Id;

#[derive(QueryableByName)]
pub struct PairRow {
    #[diesel(sql_type=Bool)]
    pub requested_review: bool,
    #[diesel(sql_type=Uuid)]
    pub id: Id,
    #[diesel(sql_type=Text)]
    pub name: String,
    #[diesel(sql_type=Timestamptz)]
    pub expires_at: DateTime<Utc>,
    #[diesel(sql_type=Nullable<Uuid>)]
    pub connection_id: Option<Id>,
}
#[derive(QueryableByName, Clone)]
pub struct ConnectionRow {
    #[diesel(sql_type=Uuid)]
    pub id: Id,
    #[diesel(sql_type=Text)]
    pub name: String,
    #[diesel(sql_type=Text)]
    pub owner_email: String,
    #[diesel(sql_type=Array<Uuid>)]
    pub account_ids: Vec<Id>,
    #[diesel(sql_type=Array<Text>)]
    pub scopes: Vec<String>,
    #[diesel(sql_type=Timestamptz)]
    pub expires_at: DateTime<Utc>,
    #[diesel(sql_type=Nullable<Timestamptz>)]
    pub revoked_at: Option<DateTime<Utc>>,
}
impl From<ConnectionRow> for ConnectionInfo {
    fn from(r: ConnectionRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            owner_email: r.owner_email,
            account_ids: r.account_ids,
            scopes: r.scopes,
            expires_at: r.expires_at,
            revoked_at: r.revoked_at,
        }
    }
}
#[derive(QueryableByName)]
pub struct IdRow {
    #[diesel(sql_type=Uuid)]
    pub id: Id,
}
#[derive(QueryableByName, Debug)]
pub struct ActionRow {
    #[diesel(sql_type=Uuid)]
    pub id: Id,
    #[diesel(sql_type=Uuid)]
    pub connection_id: Id,
    #[diesel(sql_type=Uuid)]
    pub email_id: Id,
    #[diesel(sql_type=Text)]
    pub kind: String,
    #[diesel(sql_type=Text)]
    pub status: String,
    #[diesel(sql_type=Text)]
    pub request_json: String,
    #[diesel(sql_type=Nullable<Text>)]
    pub approved_by: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    pub forward_to: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    pub result_json: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    pub error: Option<String>,
    #[diesel(sql_type=Timestamptz)]
    pub created_at: DateTime<Utc>,
    #[diesel(sql_type=Timestamptz)]
    pub updated_at: DateTime<Utc>,
}
impl TryFrom<ActionRow> for ActionInfo {
    type Error = serde_json::Error;
    fn try_from(r: ActionRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: r.id,
            connection_id: r.connection_id,
            email_id: r.email_id,
            kind: r.kind,
            status: r.status,
            request: serde_json::from_str(&r.request_json)?,
            approved_by: r.approved_by,
            forward_to: r.forward_to,
            receipt: r
                .result_json
                .map(|s| serde_json::from_str(&s))
                .transpose()?,
            error: r.error,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
    }
}

#[derive(QueryableByName)]
pub struct DraftRow {
    #[diesel(sql_type = Uuid)]
    pub id: Id,
    #[diesel(sql_type = Uuid)]
    pub connection_id: Id,
    #[diesel(sql_type = Uuid)]
    pub email_id: Id,
    #[diesel(sql_type = Text)]
    pub request_hash: String,
    #[diesel(sql_type = Text)]
    pub thread_id: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub gmail_draft_id: Option<String>,
}
