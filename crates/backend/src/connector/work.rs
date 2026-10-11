use super::{auth::Principal, models::IdRow};
use crate::{
    db,
    error::{ApiError, ApiResult},
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
    sql_types::{Array, Integer, Text, Timestamptz, Uuid as SqlUuid},
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
    let rows=diesel::sql_query("WITH candidates AS (SELECT e.id FROM emails e LEFT JOIN connector_claims w ON w.email_id=e.id WHERE e.account_id=ANY($1) AND NOT e.archived_in_gmail AND (NOT e.processed OR EXISTS (SELECT 1 FROM agent_decisions d WHERE d.source_type='email' AND d.source_id=e.id AND d.status='proposed')) AND (w.email_id IS NULL OR (w.completed_at IS NULL AND w.expires_at<now())) ORDER BY e.received_at DESC,e.id LIMIT $2 FOR UPDATE OF e SKIP LOCKED) INSERT INTO connector_claims(email_id,claim_id,connection_id,expires_at) SELECT id,$3,$4,$5 FROM candidates ON CONFLICT(email_id) DO UPDATE SET claim_id=excluded.claim_id,connection_id=excluded.connection_id,expires_at=excluded.expires_at WHERE connector_claims.completed_at IS NULL AND connector_claims.expires_at<now() RETURNING email_id AS id")
 .bind::<Array<SqlUuid>,_>(&p.account_ids).bind::<Integer,_>(input.limit.clamp(1,50) as i32).bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(cid).bind::<Timestamptz,_>(expires).load::<IdRow>(&mut c).await?;
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
    fn references_require_full_revision_and_relative_path() {
        assert!(validate_reference(&"a".repeat(40), "tasks/task.json").is_ok());
        for p in ["", "../secret", "/etc/passwd", "a/../../b", "a\\b"] {
            assert!(validate_reference(&"a".repeat(40), p).is_err());
        }
        assert!(validate_reference("main", "task.json").is_err());
    }
}
