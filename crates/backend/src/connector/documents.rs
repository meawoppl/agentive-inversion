//! Read-only Google Docs access; provider JSON is passed through at the API boundary.
use super::auth::Principal;
use crate::{
    db,
    error::{ApiError, ApiResult},
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::header,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Deserialize;
use uuid::Uuid;

pub(crate) const DOCS_SCOPE: &str = "https://www.googleapis.com/auth/documents.readonly";
#[derive(Deserialize)]
pub(super) struct DocumentQuery {
    account_id: Uuid,
}

fn validate_document_id(id: &str) -> ApiResult<()> {
    if id.is_empty()
        || id.len() > 256
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err(ApiError::bad_request(
            "Expected a Google document ID, not a URL",
        ));
    }
    Ok(())
}
pub(super) async fn get(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<String>,
    Query(q): Query<DocumentQuery>,
) -> ApiResult<Response> {
    p.scope("sources:read")?;
    p.account(q.account_id)?;
    validate_document_id(&id)?;
    let mut conn = state.pool.get().await?;
    let account = db::google_accounts::get_by_id(&mut conn, q.account_id).await?;
    drop(conn);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(anyhow::Error::from)?;
    let response = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", state.auth_config.google_client_id.as_str()),
            (
                "client_secret",
                state.auth_config.google_client_secret.as_str(),
            ),
            ("refresh_token", account.refresh_token.as_str()),
        ])
        .send()
        .await
        .map_err(anyhow::Error::from)?;
    if !response.status().is_success() {
        return Err(ApiError::Conflict("Google authorization could not be refreshed. Reconnect this account on the hosted site.".into()));
    }
    #[derive(Deserialize)]
    struct AccessToken {
        access_token: String,
    }
    let token = response
        .json::<AccessToken>()
        .await
        .map_err(anyhow::Error::from)?;
    let response = client
        .get(format!("https://docs.googleapis.com/v1/documents/{id}"))
        .query(&[("includeTabsContent", "true")])
        .bearer_auth(token.access_token)
        .send()
        .await
        .map_err(anyhow::Error::from)?;
    match response.status().as_u16() {
        200 => {},
        403 => return Err(ApiError::Forbidden("Google denied document access. Reconnect the selected account to grant read-only Docs access, verify that it can open this document, and ensure the Google Docs API is enabled for the OAuth project.".into())),
        404 => return Err(ApiError::not_found("Google document")),
        status => return Err(ApiError::Conflict(format!("Google Docs is unavailable (HTTP {status}); retry later."))),
    }
    // Preserve complete structured documents and nested tabs without persisting bodies.
    let document: serde_json::Value = response.json().await.map_err(anyhow::Error::from)?;
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(document)).into_response())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn document_ids_cannot_change_the_provider_url() {
        assert!(validate_document_id("aB0_-123").is_ok());
        for id in [
            "",
            "../other",
            "https://docs.google.com/document/d/id",
            "a?fields=title",
            "a/b",
            "a#tab",
        ] {
            assert!(validate_document_id(id).is_err(), "{id}");
        }
    }
}
