use super::models::{ConnectionRow, PairRow};
use crate::{
    auth::{jwt, middleware::extract_cookie_value},
    error::{ApiError, ApiResult},
    AppState,
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, Request},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension, Json,
};
use chrono::{Duration, Utc};
use diesel::{
    sql_query,
    sql_types::{Array, Bool, Text, Timestamptz, Uuid as SqlUuid},
    OptionalExtension,
};
use diesel_async::{AsyncConnection, RunQueryDsl};
use sha2::{Digest, Sha256};
use shared_types::connector::*;
use uuid::Uuid;

#[derive(Clone)]
pub struct Principal {
    pub email: String,
    pub connection_id: Option<Uuid>,
    pub account_ids: Vec<Uuid>,
    pub scopes: Vec<String>,
}
impl Principal {
    pub fn admin(&self) -> ApiResult<()> {
        if self.connection_id.is_some() {
            return Err(ApiError::Forbidden(
                "This operation requires approval on the hosted site".into(),
            ));
        }
        Ok(())
    }
    pub fn scope(&self, s: &str) -> ApiResult<()> {
        if self.connection_id.is_none() || self.scopes.iter().any(|v| v == s) {
            Ok(())
        } else {
            Err(ApiError::Forbidden(format!("Missing scope: {s}")))
        }
    }
    pub fn connection(&self) -> ApiResult<Uuid> {
        self.connection_id
            .ok_or_else(|| ApiError::Forbidden("Use a paired agent connection".into()))
    }
    pub fn account(&self, id: Uuid) -> ApiResult<()> {
        if self.account_ids.contains(&id) {
            Ok(())
        } else {
            Err(ApiError::Forbidden(
                "Account is outside this connection's grant".into(),
            ))
        }
    }
}
pub fn hash(secret: &str) -> String {
    format!("{:x}", Sha256::digest(secret.as_bytes()))
}
pub fn site_url(state: &AppState) -> ApiResult<reqwest::Url> {
    reqwest::Url::parse(&state.auth_config.auth_redirect_uri)
        .map_err(|_| ApiError::Config("Invalid OAuth redirect URI".into()))
}

pub async fn require(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    match authenticate(&state, req.headers().clone(), req.method().clone()).await {
        Ok(p) => {
            let changed = req.method() == axum::http::Method::POST;
            req.extensions_mut().insert(p.clone());
            let response = next.run(req).await;
            if changed && response.status().is_success() {
                state.review_events.publish(p.account_ids, p.connection_id);
            }
            response
        }
        Err(e) => e.into_response(),
    }
}
pub(super) async fn authenticate(
    state: &AppState,
    headers: axum::http::HeaderMap,
    method: axum::http::Method,
) -> ApiResult<Principal> {
    if let Some(value) = headers.get(header::AUTHORIZATION) {
        let secret = value
            .to_str()
            .ok()
            .and_then(|s| s.strip_prefix("Bearer agt_"))
            .ok_or_else(|| ApiError::Unauthorized("Invalid connector credential".into()))?;
        let mut conn = state.pool.get().await?;
        let grant=sql_query("SELECT * FROM connector_connections WHERE token_hash=$1 AND revoked_at IS NULL AND expires_at>now()")
   .bind::<Text,_>(hash(secret)).get_result::<ConnectionRow>(&mut conn).await.optional()?.ok_or_else(||ApiError::Unauthorized("Expired or revoked connector credential".into()))?;
        if !state.auth_config.is_email_allowed(&grant.owner_email) {
            return Err(ApiError::Forbidden(
                "Connection owner is no longer authorized".into(),
            ));
        }
        return Ok(Principal {
            email: grant.owner_email,
            connection_id: Some(grant.id),
            account_ids: grant.account_ids,
            scopes: grant.scopes,
        });
    }
    let token = extract_cookie_value(&headers, &state.auth_config.cookie_name)
        .ok_or_else(|| ApiError::Unauthorized("Login required".into()))?;
    let claims = jwt::validate_token(&state.auth_config, &token)
        .map_err(|_| ApiError::Unauthorized("Invalid session".into()))?;
    if !state.auth_config.is_email_allowed(&claims.sub) {
        return Err(ApiError::Forbidden("User is no longer authorized".into()));
    }
    if !matches!(method, axum::http::Method::GET | axum::http::Method::HEAD) {
        let origin = site_url(state)?.origin().ascii_serialization();
        if headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) != Some(origin.as_str())
            || headers
                .get("x-agentive-request")
                .and_then(|v| v.to_str().ok())
                != Some("1")
        {
            return Err(ApiError::Forbidden(
                "Same-origin review request required".into(),
            ));
        }
    }
    let mut conn = state.pool.get().await?;
    let accounts = crate::db::google_accounts::list_all(&mut conn).await?;
    Ok(Principal {
        email: claims.sub,
        connection_id: None,
        account_ids: accounts.into_iter().map(|a| a.id).collect(),
        scopes: vec![],
    })
}
pub async fn start(
    State(state): State<AppState>,
    Json(input): Json<PairRequest>,
) -> ApiResult<Json<PairResponse>> {
    if input.name.trim().is_empty() || input.name.len() > 100 {
        return Err(ApiError::bad_request("Connection name must be 1–100 bytes"));
    }
    let id = Uuid::new_v4();
    let secret = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let expires = Utc::now() + Duration::minutes(10);
    let mut conn = state.pool.get().await?;
    // Keep a bounded number of pending challenges, including under anonymous load.
    sql_query("DELETE FROM connector_pairings WHERE expires_at<now()")
        .execute(&mut conn)
        .await?;
    conn.transaction::<(), ApiError, _>(async |c| {
        // Short transaction; serializes the admission count across server replicas.
        sql_query("SELECT pg_advisory_xact_lock(20261011)").execute(&mut *c).await?;
        let admitted = sql_query("INSERT INTO connector_pairings(id,secret_hash,name,expires_at,requested_review) SELECT $1,$2,$3,$4,$5 WHERE (SELECT count(*) FROM connector_pairings)<1000")
            .bind::<SqlUuid,_>(id).bind::<Text,_>(hash(&secret))
            .bind::<Text,_>(input.name.trim()).bind::<Timestamptz,_>(expires)
            .bind::<Bool,_>(input.review).execute(&mut *c).await?;
        if admitted == 0 { return Err(ApiError::Conflict("Too many pairing requests; try again later".into())); }
        Ok(())
    }).await?;
    let mut url = site_url(&state)?;
    url.set_path("/");
    url.set_query(Some(&format!("connect={id}")));
    Ok(Json(PairResponse {
        id,
        device_secret: secret,
        verification_url: url.to_string(),
        expires_at: expires,
    }))
}
pub async fn poll(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<PollRequest>,
) -> ApiResult<Json<PollResponse>> {
    let mut conn = state.pool.get().await?;
    let pair = sql_query(
        "SELECT * FROM connector_pairings WHERE id=$1 AND secret_hash=$2 AND expires_at>now()",
    )
    .bind::<SqlUuid, _>(id)
    .bind::<Text, _>(hash(&input.device_secret))
    .get_result::<PairRow>(&mut conn)
    .await
    .optional()?
    .ok_or_else(|| ApiError::Gone("Pairing expired or invalid".into()))?;
    if let Some(cid) = pair.connection_id {
        let c=sql_query("SELECT * FROM connector_connections WHERE id=$1 AND revoked_at IS NULL AND expires_at>now()")
  .bind::<SqlUuid,_>(cid).get_result::<ConnectionRow>(&mut conn).await?;
        if !state.auth_config.is_email_allowed(&c.owner_email) {
            return Err(ApiError::Forbidden("Owner is no longer authorized".into()));
        }
        Ok(Json(PollResponse {
            status: "connected".into(),
            token: Some(format!("agt_{}", input.device_secret)),
            expires_at: Some(c.expires_at),
        }))
    } else {
        Ok(Json(PollResponse {
            status: "pending".into(),
            token: None,
            expires_at: None,
        }))
    }
}
pub async fn info(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<PairInfo>> {
    p.admin()?;
    let mut conn = state.pool.get().await?;
    let row=sql_query("SELECT * FROM connector_pairings WHERE id=$1 AND expires_at>now() AND connection_id IS NULL").bind::<SqlUuid,_>(id).get_result::<PairRow>(&mut conn).await?;
    Ok(Json(PairInfo {
        id: row.id,
        name: row.name,
        review: row.requested_review,
        expires_at: row.expires_at,
    }))
}
pub async fn approve(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<Uuid>,
    Json(input): Json<ApprovePair>,
) -> ApiResult<Json<ConnectionInfo>> {
    p.admin()?;
    if input.account_ids.is_empty() {
        return Err(ApiError::bad_request("Select at least one account"));
    }
    for a in &input.account_ids {
        p.account(*a)?;
    }
    let mut conn = state.pool.get().await?;
    let result=conn.transaction::<ConnectionRow,ApiError,_>(async move |c| {
  let pair=sql_query("SELECT * FROM connector_pairings WHERE id=$1 AND expires_at>now() FOR UPDATE").bind::<SqlUuid,_>(id).get_result::<PairRow>(&mut *c).await?;
  if pair.connection_id.is_some(){return Err(ApiError::Conflict("Already approved".into()));}
  let cid=Uuid::new_v4();
  let row=sql_query("INSERT INTO connector_connections(id,token_hash,name,owner_email,account_ids,scopes,expires_at) SELECT $2,secret_hash,name,$3,$4,CASE WHEN requested_review THEN ARRAY['sources:read','actions:propose','actions:approve'] ELSE ARRAY['sources:read','work:write','actions:propose','legacy:export'] END,now()+interval '90 days' FROM connector_pairings WHERE id=$1 RETURNING *")
   .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(cid).bind::<Text,_>(p.email).bind::<Array<SqlUuid>,_>(input.account_ids).get_result::<ConnectionRow>(&mut *c).await?;
  sql_query("UPDATE connector_pairings SET connection_id=$2 WHERE id=$1").bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(cid).execute(&mut *c).await?;Ok(row)
 }).await?;
    Ok(Json(result.into()))
}
pub async fn connections(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
) -> ApiResult<Json<Vec<ConnectionInfo>>> {
    p.admin()?;
    let mut c = state.pool.get().await?;
    let rows = sql_query("SELECT * FROM connector_connections ORDER BY created_at DESC")
        .load::<ConnectionRow>(&mut c)
        .await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}
pub async fn revoke(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> ApiResult<axum::http::StatusCode> {
    p.admin()?;
    let mut c = state.pool.get().await?;
    sql_query("UPDATE connector_connections SET revoked_at=now() WHERE id=$1")
        .bind::<SqlUuid, _>(id)
        .execute(&mut c)
        .await?;
    sql_query("UPDATE connector_claims SET expires_at=now() WHERE connection_id=$1 AND completed_at IS NULL")
        .bind::<SqlUuid,_>(id).execute(&mut c).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}
