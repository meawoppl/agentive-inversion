use super::{
    auth::Principal,
    models::{ActionRow, IdRow},
};
use crate::{
    db,
    error::{ApiError, ApiResult},
    pollers::gmail_client::{EmailMessage, GmailClient},
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use chrono::{Duration, Utc};
use diesel::{
    prelude::*,
    sql_types::{Array, Bool, Integer, Nullable, Text, Timestamptz, Uuid as SqlUuid},
};
use diesel_async::{AsyncConnection, RunQueryDsl};
use serde::Deserialize;
use shared_types::connector::*;
use uuid::Uuid;

pub fn source(e: db::emails::Email, account_email: String, body: bool) -> Source {
    let gmail_link = format!(
        "https://mail.google.com/mail/u/{}/#all/{}",
        urlencoding::encode(&account_email),
        e.gmail_id
    );
    Source {
        id: e.id,
        account_id: e.account_id,
        account_email,
        gmail_id: e.gmail_id,
        thread_id: e.thread_id,
        gmail_link,
        subject: e.subject,
        from_address: e.from_address,
        to_addresses: e.to_addresses.into_iter().flatten().collect(),
        cc_addresses: e
            .cc_addresses
            .unwrap_or_default()
            .into_iter()
            .flatten()
            .collect(),
        received_at: e.received_at,
        snippet: e.snippet,
        body_text: if body { e.body_text } else { None },
        archived: e.archived_in_gmail,
    }
}
#[derive(Deserialize, Default)]
pub struct SourceQuery {
    pub cursor: Option<Uuid>,
    pub limit: Option<i64>,
    pub body: Option<bool>,
    pub account_id: Option<Uuid>,
}
pub async fn sources(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Query(q): Query<SourceQuery>,
) -> ApiResult<Json<SourcePage>> {
    p.scope("sources:read")?;
    if let Some(a) = q.account_id {
        p.account(a)?;
    }
    use crate::schema::{emails as e, google_accounts as a};
    let mut c = state.pool.get().await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let mut query = e::table
        .inner_join(a::table)
        .filter(e::account_id.eq_any(&p.account_ids))
        .order(e::id.asc())
        .select((db::emails::Email::as_select(), a::email))
        .into_boxed();
    if let Some(cursor) = q.cursor {
        query = query.filter(e::id.gt(cursor));
    }
    if let Some(account) = q.account_id {
        query = query.filter(e::account_id.eq(account));
    }
    let mut rows = query
        .limit(limit + 1)
        .load::<(db::emails::Email, String)>(&mut c)
        .await?;
    let more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let cursor = if more {
        rows.last().map(|(e, _)| e.id)
    } else {
        None
    };
    Ok(Json(SourcePage {
        items: rows.into_iter().map(|(e, a)| source(e, a, false)).collect(),
        next_cursor: cursor,
    }))
}
pub async fn get_source(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<Uuid>,
    Query(q): Query<SourceQuery>,
) -> ApiResult<Json<Source>> {
    p.scope("sources:read")?;
    let mut c = state.pool.get().await?;
    let e = crate::schema::emails::table
        .find(id)
        .select(db::emails::Email::as_select())
        .first::<db::emails::Email>(&mut c)
        .await?;
    p.account(e.account_id)?;
    let account = db::google_accounts::get_by_id(&mut c, e.account_id).await?;
    Ok(Json(source(e, account.email, q.body.unwrap_or(false))))
}
pub async fn get_thread(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<SourceThread>> {
    p.scope("sources:read")?;
    let mut c = state.pool.get().await?;
    let source = db::emails::get_by_id(&mut c, id).await?;
    p.account(source.account_id)?;
    let account = db::google_accounts::get_by_id(&mut c, source.account_id).await?;
    let rows = diesel::sql_query("SELECT a.* FROM connector_actions a JOIN emails e ON e.id=a.email_id WHERE e.account_id=$1 AND e.thread_id=$2 ORDER BY a.created_at,a.id")
        .bind::<SqlUuid,_>(source.account_id).bind::<Text,_>(&source.thread_id)
        .load::<ActionRow>(&mut c).await?;
    drop(c);
    let gmail = GmailClient::from_account(&account).await?;
    let thread = match gmail.get_thread(&source.thread_id).await {
        Ok(thread) => {
            if let Some(cid) = p.connection_id {
                let mut c = state.pool.get().await?;
                diesel::sql_query("UPDATE connector_claims SET read_retry_after=NULL,last_read_status=NULL WHERE email_id=$1 AND connection_id=$2 AND completed_at IS NULL")
                    .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(cid).execute(&mut c).await?;
            }
            thread
        }
        Err(error) => {
            let status = gmail_read_status(&error);
            if let Some(cid) = p.connection_id {
                defer_failed_read(&state, id, cid, status).await?;
            }
            if status == Some(404) {
                return Err(ApiError::NotFound(
                    "Live Gmail thread (triage deferred for 24 hours)".into(),
                ));
            }
            return Err(error.into());
        }
    };
    let messages = thread
        .into_iter()
        .map(|m| thread_message(m, &account.email))
        .collect();
    Ok(Json(SourceThread {
        source_id: id,
        account_id: source.account_id,
        account_email: account.email,
        thread_id: source.thread_id,
        messages,
        actions: rows
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?,
    }))
}

fn gmail_read_status(error: &anyhow::Error) -> Option<i32> {
    match error.downcast_ref::<google_gmail1::Error>()? {
        google_gmail1::Error::BadRequest(body) => body
            .pointer("/error/code")?
            .as_i64()
            .and_then(|n| i32::try_from(n).ok()),
        google_gmail1::Error::Failure(response) => Some(i32::from(response.status().as_u16())),
        _ => None,
    }
}

pub(super) async fn defer_failed_read(
    state: &AppState,
    email_id: Uuid,
    connection_id: Uuid,
    status: Option<i32>,
) -> ApiResult<()> {
    let retry_at = Utc::now() + Duration::minutes(if status == Some(404) { 1440 } else { 15 });
    let mut c = state.pool.get().await?;
    // Keep this separate from the lease: release must not erase the backoff.
    // Only this connection's unfinished work is affected. No completion/effect.
    diesel::sql_query("UPDATE connector_claims SET read_retry_after=$3,last_read_status=$4 WHERE email_id=$1 AND connection_id=$2 AND completed_at IS NULL")
        .bind::<SqlUuid,_>(email_id).bind::<SqlUuid,_>(connection_id)
        .bind::<Timestamptz,_>(retry_at).bind::<Nullable<Integer>,_>(status)
        .execute(&mut c).await?;
    Ok(())
}

fn thread_message(m: EmailMessage, account_email: &str) -> ThreadMessage {
    ThreadMessage {
        gmail_link: format!(
            "https://mail.google.com/mail/u/{}/#all/{}",
            urlencoding::encode(account_email),
            m.id
        ),
        gmail_id: m.id,
        subject: m.subject,
        from_address: m.from,
        to_addresses: m.to,
        cc_addresses: m.cc,
        received_at: m.received_at,
        labels: m.labels,
        snippet: m.snippet,
        body_text: m.body_text,
        body_html: m.body_html,
    }
}
#[derive(Deserialize)]
pub struct MailSearchQuery {
    pub account_id: Uuid,
    pub q: String,
    pub page_token: Option<String>,
    pub limit: Option<u32>,
}
pub async fn search_mail(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Query(q): Query<MailSearchQuery>,
) -> ApiResult<Json<MailSearchPage>> {
    p.scope("sources:read")?;
    p.account(q.account_id)?;
    if q.q.trim().is_empty()
        || q.q.len() > 2000
        || q.page_token.as_ref().is_some_and(|t| t.len() > 2048)
    {
        return Err(ApiError::bad_request(
            "A search query of 1–2000 bytes and a bounded page token are required",
        ));
    }
    let mut c = state.pool.get().await?;
    let account = db::google_accounts::get_by_id(&mut c, q.account_id).await?;
    drop(c);
    let gmail = GmailClient::from_account(&account).await?;
    let (ids, next_page_token) = gmail
        .search_threads(
            &q.q,
            q.page_token.as_deref(),
            q.limit.unwrap_or(5).clamp(1, 10),
        )
        .await?;
    let mut threads = Vec::new();
    for thread_id in ids {
        let messages = gmail
            .get_thread(&thread_id)
            .await?
            .into_iter()
            .map(|m| thread_message(m, &account.email))
            .collect();
        threads.push(MailThread {
            thread_id,
            messages,
        });
    }
    Ok(Json(MailSearchPage {
        account_id: account.id,
        account_email: account.email,
        threads,
        next_page_token,
    }))
}

pub fn validate_reference(commit: &str, path: &str) -> ApiResult<()> {
    if !matches!(commit.len(), 40 | 64) || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ApiError::bad_request(
            "repository_commit must be a full Git SHA",
        ));
    }
    if path.is_empty()
        || path.len() > 1024
        || path.starts_with('/')
        || path.split('/').any(|p| p == "..")
        || path.chars().any(|c| c.is_control() || c == '\\')
    {
        return Err(ApiError::bad_request(
            "repository_path must be a safe relative path",
        ));
    }
    Ok(())
}
pub async fn claim(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Json(input): Json<ClaimRequest>,
) -> ApiResult<Json<ClaimResponse>> {
    p.scope("work:write")?;
    let cid = p.connection()?;
    let id = Uuid::new_v4();
    let expires = Utc::now() + Duration::minutes(30);
    let mut c = state.pool.get().await?;
    // Row locks on emails serialize competing claims; expired claims are replaced
    // with a new batch UUID, fencing every mutation by its old holder.
    if input.source_ids.len() > 50 {
        return Err(ApiError::bad_request(
            "At most 50 source_ids may be selected",
        ));
    }
    // Reconsider only sources with no action history: approvals, rejections and
    // uncertain/succeeded effects must never be silently recreated. A new claim
    // generation fences the old completion and resets its provenance atomically.
    let rows = diesel::sql_query(
        r#"
        WITH candidates AS (
            SELECT e.id FROM emails e
            LEFT JOIN connector_claims w ON w.email_id=e.id
            WHERE e.account_id=ANY($1) AND NOT e.archived_in_gmail
              AND (cardinality($6::uuid[])=0 OR e.id=ANY($6))
              AND (
                (NOT $7 AND (w.email_id IS NULL OR
                    (w.completed_at IS NULL AND w.expires_at<now()
                     AND (w.read_retry_after IS NULL OR w.read_retry_after<=now())))
                 AND (w.email_id IS NOT NULL OR NOT EXISTS (
                    SELECT 1 FROM agent_decisions d WHERE d.source_type='email'
                    AND d.source_id=e.id AND d.status='rejected')))
                OR
                (w.completed_at IS NOT NULL
                 AND NOT EXISTS (SELECT 1 FROM connector_actions a WHERE a.email_id=e.id)
                 AND (cardinality($6::uuid[])>0 OR w.completed_at<now()-interval '24 hours'
                      OR EXISTS (SELECT 1 FROM emails reply
                          WHERE reply.account_id=e.account_id AND reply.thread_id=e.thread_id
                          AND reply.received_at>w.completed_at)))
              )
            ORDER BY w.completed_at ASC NULLS FIRST,e.received_at DESC,e.id
            LIMIT $2 FOR UPDATE OF e SKIP LOCKED
        )
        INSERT INTO connector_claims(email_id,claim_id,connection_id,expires_at)
        SELECT id,$3,$4,$5 FROM candidates
        ON CONFLICT(email_id) DO UPDATE SET
            claim_id=excluded.claim_id, connection_id=excluded.connection_id,
            expires_at=excluded.expires_at, completed_at=NULL, disposition=NULL,
            repository_commit=NULL, repository_path=NULL,
            read_retry_after=NULL,last_read_status=NULL
        WHERE connector_claims.completed_at IS NOT NULL OR connector_claims.expires_at<now()
        RETURNING email_id AS id
    "#,
    )
    .bind::<Array<SqlUuid>, _>(&p.account_ids)
    .bind::<Integer, _>(input.limit.clamp(1, 50) as i32)
    .bind::<SqlUuid, _>(id)
    .bind::<SqlUuid, _>(cid)
    .bind::<Timestamptz, _>(expires)
    .bind::<Array<SqlUuid>, _>(&input.source_ids)
    .bind::<Bool, _>(input.reconsider)
    .load::<IdRow>(&mut c)
    .await?;
    let mut items = vec![];
    for r in rows {
        let e = db::emails::get_by_id(&mut c, r.id).await?;
        let a = db::google_accounts::get_by_id(&mut c, e.account_id).await?;
        items.push(source(e, a.email, false));
    }
    Ok(Json(ClaimResponse {
        claim_id: id,
        expires_at: expires,
        items,
    }))
}
pub async fn renew(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    p.scope("work:write")?;
    let mut c = state.pool.get().await?;
    let n=diesel::sql_query("UPDATE connector_claims SET expires_at=now()+interval '30 minutes' WHERE claim_id=$1 AND connection_id=$2 AND expires_at>now() AND completed_at IS NULL")
 .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(p.connection()?).execute(&mut c).await?;
    if n == 0 {
        return Err(ApiError::Conflict("Claim expired or completed".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}
pub async fn release(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    p.scope("work:write")?;
    let mut c = state.pool.get().await?;
    diesel::sql_query("UPDATE connector_claims SET expires_at=now() WHERE claim_id=$1 AND connection_id=$2 AND completed_at IS NULL")
 .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(p.connection()?).execute(&mut c).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn complete(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<Uuid>,
    Json(input): Json<CompleteRequest>,
) -> ApiResult<StatusCode> {
    p.scope("work:write")?;
    let cid = p.connection()?;
    validate_reference(&input.repository_commit, &input.repository_path)?;
    if !["recorded", "kept", "ignored"].contains(&input.disposition.as_str()) {
        return Err(ApiError::bad_request(
            "disposition must be recorded, kept, or ignored",
        ));
    }
    let mut c = state.pool.get().await?;
    c.transaction::<(),ApiError,_>(async move |c| {
  let n=diesel::sql_query("UPDATE connector_claims SET completed_at=COALESCE(completed_at,now()),disposition=$4,repository_commit=$5,repository_path=$6 WHERE email_id=$1 AND claim_id=$2 AND connection_id=$3 AND ((completed_at IS NULL AND expires_at>now()) OR (completed_at IS NOT NULL AND disposition=$4 AND repository_commit=$5 AND repository_path=$6))")
   .bind::<SqlUuid,_>(input.email_id).bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(cid).bind::<Text,_>(&input.disposition).bind::<Text,_>(&input.repository_commit).bind::<Text,_>(&input.repository_path).execute(&mut *c).await?;
  if n!=1{return Err(ApiError::Conflict("Claim expired, replaced, or completed differently".into()));}
  diesel::sql_query("UPDATE emails SET processed=true,processed_at=now(),triage_status='repository_recorded',triaged_at=now() WHERE id=$1").bind::<SqlUuid,_>(input.email_id).execute(&mut *c).await?;Ok(())
 }).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_thread_status_survives_anyhow_context() {
        let error = anyhow::Error::new(google_gmail1::Error::BadRequest(
            serde_json::json!({"error":{"code":404}}),
        ))
        .context("Failed to get Gmail thread");
        assert_eq!(gmail_read_status(&error), Some(404));
        assert_eq!(
            gmail_read_status(&anyhow::anyhow!("transport failed")),
            None
        );
    }
    #[test]
    fn references_require_full_revision_and_relative_path() {
        assert!(validate_reference(&"a".repeat(40), "tasks/task.json").is_ok());
        for p in ["", "../secret", "/etc/passwd", "a/../../b", "a\\b"] {
            assert!(validate_reference(&"a".repeat(40), p).is_err());
        }
        assert!(validate_reference("main", "task.json").is_err());
    }
}
