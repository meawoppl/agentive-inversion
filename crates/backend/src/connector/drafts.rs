//! User-requested drafts use existing proposal grants. Saving is not sending.
use super::{
    auth::{hash, Principal},
    models::DraftRow,
};
use crate::{
    db,
    error::{ApiError, ApiResult},
    pollers::{
        gmail_client::GmailClient,
        gmail_drafts::{header, reply_mime},
    },
    AppState,
};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Extension, Json,
};
use diesel::{
    sql_query,
    sql_types::{Text, Uuid as SqlUuid},
    OptionalExtension,
};
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DraftRequest {
    pub body_text: String,
    #[serde(default)]
    pub reply_all: bool,
    /// Defaults to the latest non-draft message in the live conversation.
    pub reply_to_message_id: Option<String>,
}
impl DraftRequest {
    fn validate(&self) -> ApiResult<()> {
        if self.body_text.trim().is_empty() || self.body_text.len() > 200_000 {
            return Err(ApiError::bad_request(
                "Draft body must contain 1–200000 bytes",
            ));
        }
        if self.reply_to_message_id.as_ref().is_some_and(|id| {
            id.is_empty() || id.len() > 128 || !id.bytes().all(|c| c.is_ascii_hexdigit())
        }) {
            return Err(ApiError::bad_request("Invalid Gmail reply message ID"));
        }
        Ok(())
    }
}

#[derive(Serialize)]
pub struct DraftResponse {
    pub id: Uuid,
    pub source_id: Uuid,
    pub account_email: String,
    pub draft_id: String,
    pub message_id: String,
    pub thread_id: String,
    pub gmail_link: String,
    pub status: &'static str,
    pub to: String,
    pub cc: String,
    pub subject: String,
    pub body_text: Option<String>,
}

fn authorize(p: &Principal) -> ApiResult<Uuid> {
    p.scope("sources:read")?;
    p.scope("actions:propose")?;
    p.connection()
}

pub async fn create(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(source_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<DraftRequest>,
) -> ApiResult<Json<DraftResponse>> {
    let connection = authorize(&p)?;
    input.validate()?;
    let key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 200
                && s.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c))
        })
        .ok_or_else(|| ApiError::bad_request("A stable Idempotency-Key is required"))?;
    let request_hash = hash(&serde_json::to_string(&(source_id, &input))?);
    let mut c = state.pool.get().await?;
    let source = db::emails::get_by_id(&mut c, source_id).await?;
    p.account(source.account_id)?;
    let prior =
        sql_query("SELECT * FROM connector_drafts WHERE connection_id=$1 AND request_key=$2")
            .bind::<SqlUuid, _>(connection)
            .bind::<Text, _>(key)
            .get_result::<DraftRow>(&mut c)
            .await
            .optional()?;
    if let Some(row) = prior {
        if row.request_hash != request_hash {
            return Err(ApiError::Conflict(
                "Idempotency key already used for a different draft request".into(),
            ));
        }
        drop(c);
        return read_live(&state, &p, row).await.map(Json);
    }
    let account = db::google_accounts::get_by_id(&mut c, source.account_id).await?;
    drop(c);
    let gmail = GmailClient::from_account(&account).await?;
    let (anchor, own) = gmail
        .draft_reply_context(&source.thread_id, input.reply_to_message_id.as_deref())
        .await?;
    let id = Uuid::new_v4();
    let mime = reply_mime(
        &anchor,
        &account.email,
        &own,
        &input.body_text,
        input.reply_all,
        id,
    )
    .map_err(|e| ApiError::bad_request(e.to_string()))?;
    // Reserve before crossing the provider boundary. Concurrent retries cannot create.
    let mut c = state.pool.get().await?;
    let inserted = sql_query("INSERT INTO connector_drafts(id,connection_id,email_id,request_key,request_hash,thread_id) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(connection_id,request_key) DO NOTHING")
        .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(connection).bind::<SqlUuid,_>(source_id)
        .bind::<Text,_>(key).bind::<Text,_>(&request_hash).bind::<Text,_>(&source.thread_id).execute(&mut c).await?;
    let row = sql_query("SELECT * FROM connector_drafts WHERE connection_id=$1 AND request_key=$2")
        .bind::<SqlUuid, _>(connection)
        .bind::<Text, _>(key)
        .get_result::<DraftRow>(&mut c)
        .await?;
    drop(c);
    if row.request_hash != request_hash {
        return Err(ApiError::Conflict(
            "Idempotency key already used for a different draft request".into(),
        ));
    }
    if inserted == 1 {
        // Errors/connection loss leave the reservation in place. Never auto-repeat creation.
        let draft = gmail.create_reply_draft(&source.thread_id, mime).await.map_err(|_| ApiError::Conflict(format!("Draft creation outcome uncertain. Retry the SAME request/key or GET /drafts/{id} to reconcile; do not create with a new key.")))?;
        if let Some(draft_id) = draft.id {
            save_receipt(&state, id, &draft_id).await?;
        }
    }
    let mut c = state.pool.get().await?;
    let row = sql_query("SELECT * FROM connector_drafts WHERE id=$1")
        .bind::<SqlUuid, _>(row.id)
        .get_result::<DraftRow>(&mut c)
        .await?;
    drop(c);
    read_live(&state, &p, row).await.map(Json)
}

async fn save_receipt(state: &AppState, id: Uuid, draft_id: &str) -> ApiResult<()> {
    let mut c = state.pool.get().await?;
    sql_query(
        "UPDATE connector_drafts SET gmail_draft_id=$2 WHERE id=$1 AND gmail_draft_id IS NULL",
    )
    .bind::<SqlUuid, _>(id)
    .bind::<Text, _>(draft_id)
    .execute(&mut c)
    .await?;
    Ok(())
}

pub async fn get(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<DraftResponse>> {
    let connection = authorize(&p)?;
    let mut c = state.pool.get().await?;
    let row = sql_query("SELECT * FROM connector_drafts WHERE id=$1 AND connection_id=$2")
        .bind::<SqlUuid, _>(id)
        .bind::<SqlUuid, _>(connection)
        .get_result::<DraftRow>(&mut c)
        .await?;
    drop(c);
    read_live(&state, &p, row).await.map(Json)
}

async fn read_live(state: &AppState, p: &Principal, row: DraftRow) -> ApiResult<DraftResponse> {
    if p.connection()? != row.connection_id {
        return Err(ApiError::Forbidden(
            "Draft belongs to another connection".into(),
        ));
    }
    let mut c = state.pool.get().await?;
    let source = db::emails::get_by_id(&mut c, row.email_id).await?;
    p.account(source.account_id)?;
    let account = db::google_accounts::get_by_id(&mut c, source.account_id).await?;
    drop(c);
    let gmail = GmailClient::from_account(&account).await?;
    let draft = if let Some(id) = &row.gmail_draft_id {
        gmail.read_draft(id).await?
    } else {
        let found = gmail.find_reply_draft(row.id, &row.thread_id).await?
                .ok_or_else(|| ApiError::Conflict(format!("Draft {} is in progress or outcome is unknown. Retry this operation later; a missing search result is NOT permission to create another draft.", row.id)))?;
        if let Some(id) = found.id.as_deref() {
            save_receipt(state, row.id, id).await?;
        }
        found
    };
    let draft_id = draft
        .id
        .ok_or_else(|| ApiError::Conflict("Gmail omitted draft ID".into()))?;
    let message = draft
        .message
        .ok_or_else(|| ApiError::Conflict("Gmail omitted draft message".into()))?;
    if message.thread_id.as_deref() != Some(&row.thread_id)
        || !message
            .label_ids
            .as_ref()
            .is_some_and(|ids| ids.iter().any(|id| id == "DRAFT"))
    {
        return Err(ApiError::Conflict(
            "Saved draft no longer matches the expected thread/draft state; inspect Gmail".into(),
        ));
    }
    let to = header(&message, "To")?;
    let cc = header(&message, "Cc")?;
    let parsed = GmailClient::parse_message(message);
    Ok(DraftResponse {
        id: row.id,
        source_id: row.email_id,
        account_email: account.email.clone(),
        draft_id,
        message_id: parsed.id,
        thread_id: row.thread_id.clone(),
        gmail_link: format!(
            "https://mail.google.com/mail/u/?authuser={}#all/{}",
            urlencoding::encode(&account.email),
            row.thread_id
        ),
        status: "draft",
        to,
        cc,
        subject: parsed.subject,
        body_text: parsed.body_text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drafts_use_existing_grants_without_approval_or_new_scope() {
        let mut p = Principal {
            email: "owner@example.com".into(),
            connection_id: Some(Uuid::new_v4()),
            account_ids: vec![],
            scopes: vec!["sources:read".into(), "actions:propose".into()],
        };
        assert!(authorize(&p).is_ok());
        assert!(p.scope("actions:approve").is_err());
        p.scopes.pop();
        assert!(authorize(&p).is_err());
    }
    #[test]
    fn rejects_empty_oversize_and_unknown_send_fields() {
        for body in ["".to_string(), "x".repeat(200001)] {
            assert!(DraftRequest {
                body_text: body,
                reply_all: false,
                reply_to_message_id: None
            }
            .validate()
            .is_err());
        }
        assert!(serde_json::from_value::<DraftRequest>(
            serde_json::json!({"body_text":"hi","send":true})
        )
        .is_err());
    }
}
