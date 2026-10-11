//! Hosted connector: source access, delegated credentials, work leases and effects.
mod actions;
mod auth;
pub(crate) mod documents;
mod models;
#[cfg(test)]
mod tests;
mod work;
use crate::{db, error::ApiResult, AppState};
pub use actions::run;
use auth::Principal;
use axum::{
    extract::{Query, State},
    middleware,
    routing::{get, post},
    Extension, Json, Router,
};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use shared_types::{connector::Capabilities, AgentDecision, Category, GoogleAccountResponse, Todo};

pub fn routes(state: AppState) -> Router<AppState> {
    let protected = Router::new()
        .route("/capabilities", get(capabilities))
        .route("/accounts", get(accounts))
        .route("/pairings/:id", get(auth::info))
        .route("/pairings/:id/approve", post(auth::approve))
        .route("/connections", get(auth::connections))
        .route("/connections/:id/revoke", post(auth::revoke))
        .route("/calendar/events", get(calendar_events))
        .route("/documents/:id", get(documents::get))
        .route("/sources", get(work::sources))
        .route("/sources/:id", get(work::get_source))
        .route("/work/claims", post(work::claim))
        .route("/work/claims/:id/renew", post(work::renew))
        .route("/work/claims/:id/release", post(work::release))
        .route("/work/claims/:id/complete", post(work::complete))
        .route("/actions", get(actions::list).post(actions::submit))
        .route("/actions/:id", get(actions::get))
        .route("/actions/:id/approve", post(actions::approve))
        .route("/actions/:id/reject", post(actions::reject))
        .route("/actions/:id/retry", post(actions::retry))
        .route("/legacy/export", get(export))
        .layer(middleware::from_fn_with_state(state, auth::require));
    Router::new()
        .merge(protected)
        .route("/pairings", post(auth::start))
        .route("/pairings/:id/poll", post(auth::poll))
}
async fn capabilities(Extension(p): Extension<Principal>) -> Json<Capabilities> {
    Json(Capabilities {
        api_version: "1".into(),
        mode: "portal_connector".into(),
        scopes: p.scopes,
        account_ids: p.account_ids,
        max_claim: 50,
        lease_seconds: 1800,
        approval_required: true,
    })
}
async fn accounts(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
) -> ApiResult<Json<Vec<GoogleAccountResponse>>> {
    p.scope("sources:read")?;
    let mut c = state.pool.get().await?;
    Ok(Json(
        db::google_accounts::list_all(&mut c)
            .await?
            .into_iter()
            .filter(|a| p.account_ids.contains(&a.id))
            .map(Into::into)
            .collect(),
    ))
}
#[derive(Deserialize)]
struct ExportQuery {
    #[serde(default)]
    offset: i64,
}
#[derive(Serialize)]
struct LegacyExport {
    schema_version: u32,
    about_me: String,
    todos: Vec<Todo>,
    categories: Vec<Category>,
    decisions: Vec<AgentDecision>,
    next_offset: Option<i64>,
}
async fn export(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Query(q): Query<ExportQuery>,
) -> ApiResult<Json<LegacyExport>> {
    p.scope("legacy:export")?;
    let mut c = state.pool.get().await?;
    // Existing project records belong to one shared workspace. Account grants
    // restrict sources; export requires a distinct scope shown during pairing.
    use crate::schema::agent_decisions::dsl as d;
    let offset = q.offset.max(0);
    let mut rows = d::agent_decisions
        .order(d::id.asc())
        .offset(offset)
        .limit(501)
        .load::<crate::models::AgentDecisionRow>(&mut c)
        .await?;
    let more = rows.len() > 500;
    rows.truncate(500);
    Ok(Json(LegacyExport {
        schema_version: 1,
        about_me: db::about_me::get(&mut c).await?.content,
        todos: db::todos::list_all(&mut c).await?,
        categories: db::categories::list_all(&mut c).await?,
        decisions: rows.into_iter().map(Into::into).collect(),
        next_offset: more.then_some(offset + 500),
    }))
}

#[derive(Deserialize)]
struct CalendarQuery {
    account_id: uuid::Uuid,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
}
async fn calendar_events(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Query(q): Query<CalendarQuery>,
) -> ApiResult<Json<Vec<crate::calendar_client::ExistingEvent>>> {
    p.scope("sources:read")?;
    p.account(q.account_id)?;
    if q.to <= q.from || q.to - q.from > chrono::Duration::days(31) {
        return Err(crate::error::ApiError::bad_request(
            "Calendar window must be positive and at most 31 days",
        ));
    }
    let mut c = state.pool.get().await?;
    let account = db::google_accounts::get_by_id(&mut c, q.account_id).await?;
    drop(c);
    let client = crate::calendar_client::CalendarClient::from_account(&account).await?;
    let mut events = vec![];
    for id in client.list_calendar_ids().await? {
        events.extend(client.list_events_between(&id, q.from, q.to).await?);
    }
    Ok(Json(events))
}
