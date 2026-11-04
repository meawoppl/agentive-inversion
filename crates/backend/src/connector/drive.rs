//! Account-scoped, read-only Google Drive discovery. No file contents are stored.
use super::{auth::Principal, documents::read_client};
use crate::{
    error::{ApiError, ApiResult},
    AppState,
};
use axum::{
    extract::{Query, State},
    http::header,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Deserialize;
use uuid::Uuid;

pub(crate) const DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive.metadata.readonly";
const FIELDS: &str = "nextPageToken,incompleteSearch,files(id,name,mimeType,description,webViewLink,createdTime,modifiedTime,size,parents,driveId,trashed,owners(displayName,emailAddress),shortcutDetails(targetId,targetMimeType))";

#[derive(Deserialize)]
pub(super) struct SearchQuery {
    account_id: Uuid,
    q: Option<String>,
    page_token: Option<String>,
    limit: Option<u32>,
    drive_id: Option<String>,
    #[serde(default)]
    all_drives: bool,
    #[serde(default)]
    include_trashed: bool,
}

fn parameters(q: &SearchQuery) -> ApiResult<Vec<(&'static str, String)>> {
    if q.q.as_ref().is_some_and(|s| s.len() > 4000)
        || q.page_token
            .as_ref()
            .is_some_and(|s| s.is_empty() || s.len() > 4096)
        || q.limit.is_some_and(|n| !(1..=100).contains(&n))
        || q.drive_id.as_ref().is_some_and(|s| {
            s.is_empty()
                || s.len() > 256
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        })
        || (q.all_drives && q.drive_id.is_some())
    {
        return Err(ApiError::bad_request("Invalid Drive search parameters: limit must be 1–100; use either drive_id or all_drives, with bounded query and page token"));
    }
    let query = q.q.as_deref().unwrap_or("").trim();
    let query = match (query.is_empty(), q.include_trashed) {
        (true, true) => String::new(),
        (true, false) => "trashed = false".into(),
        (false, true) => query.into(),
        (false, false) => format!("({query}) and trashed = false"),
    };
    let mut params = vec![
        ("q", query),
        ("pageSize", q.limit.unwrap_or(100).to_string()),
        ("fields", FIELDS.into()),
        ("supportsAllDrives", "true".into()),
        ("includeItemsFromAllDrives", "true".into()),
        (
            "corpora",
            if q.drive_id.is_some() {
                "drive"
            } else if q.all_drives {
                "allDrives"
            } else {
                "user"
            }
            .into(),
        ),
    ];
    if let Some(id) = &q.drive_id {
        params.push(("driveId", id.clone()));
    }
    if let Some(token) = &q.page_token {
        params.push(("pageToken", token.clone()));
    }
    Ok(params)
}

pub(super) async fn list(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Query(q): Query<SearchQuery>,
) -> ApiResult<Response> {
    p.scope("sources:read")?;
    p.account(q.account_id)?;
    let params = parameters(&q)?;
    let (client, token) = read_client(&state, q.account_id).await?;
    let response = client
        .get("https://www.googleapis.com/drive/v3/files")
        .query(&params)
        .bearer_auth(token.access_token)
        .send()
        .await
        .map_err(anyhow::Error::from)?;
    match response.status().as_u16() {
        200 => {},
        400 => return Err(ApiError::bad_request("Google rejected the Drive query or page token. Use Drive v3 query syntax; restart pagination if the token expired.")),
        401 | 403 => return Err(super::google_errors::failure(response, "Drive search", q.account_id, &state.auth_config.google_client_id).await),
        404 => return Err(ApiError::not_found("Google Drive resource")),
        status => return Err(ApiError::Conflict(format!("Google Drive is unavailable (HTTP {status}); retry later."))),
    }
    // Keep provider pagination and incompleteSearch intact, including empty pages.
    let page: serde_json::Value = response.json().await.map_err(anyhow::Error::from)?;
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(page)).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn query(value: serde_json::Value) -> SearchQuery {
        serde_json::from_value(value).unwrap()
    }
    #[test]
    fn preserves_query_and_pagination_without_broadening_trash_filter() {
        let q = query(
            serde_json::json!({"account_id":Uuid::nil(),"q":"name contains 'Kruze' or fullText contains 'tax'","page_token":"a+/=","drive_id":"shared_123"}),
        );
        let p = parameters(&q).unwrap();
        assert!(p.contains(&(
            "q",
            "(name contains 'Kruze' or fullText contains 'tax') and trashed = false".into()
        )));
        assert!(p.contains(&("pageToken", "a+/=".into())));
        assert!(p.contains(&("corpora", "drive".into())));
        assert!(p.contains(&("driveId", "shared_123".into())));
        assert!(FIELDS.contains("incompleteSearch"));
    }
    #[test]
    fn listing_and_all_drive_search_are_explicit() {
        let q = query(serde_json::json!({"account_id":Uuid::nil()}));
        let p = parameters(&q).unwrap();
        assert!(p.contains(&("q", "trashed = false".into())));
        assert!(p.contains(&("corpora", "user".into())));
        let q = query(
            serde_json::json!({"account_id":Uuid::nil(),"all_drives":true,"include_trashed":true}),
        );
        let p = parameters(&q).unwrap();
        assert!(p.contains(&("q", "".into())));
        assert!(p.contains(&("corpora", "allDrives".into())));
    }
    #[test]
    fn rejects_bad_parameters_before_provider_access() {
        for value in [
            serde_json::json!({"limit":0}),
            serde_json::json!({"limit":101}),
            serde_json::json!({"drive_id":"../bad"}),
            serde_json::json!({"all_drives":true,"drive_id":"abc"}),
            serde_json::json!({"page_token":""}),
            serde_json::json!({"q":"x".repeat(4001)}),
        ] {
            let mut value = value;
            value["account_id"] = serde_json::json!(Uuid::nil());
            assert!(parameters(&query(value)).is_err());
        }
    }
}
