//! Authentication HTTP handlers.

use axum::extract::Query;
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use serde::Deserialize;

use crate::error::{ApiError, ApiResult};
use crate::AppState;

use super::{
    build_auth_cookie, extract_auth_user, jwt,
    types::{AuthUserResponse, LoginInitResponse},
};

/// Start Google OAuth login flow.
///
/// Returns a URL that the frontend should redirect the user to.
pub async fn auth_login(State(state): State<AppState>) -> ApiResult<Json<LoginInitResponse>> {
    let config = &state.auth_config;

    // Generate state parameter (for CSRF protection in production, you'd want to store this)
    let csrf_state = uuid::Uuid::new_v4().to_string();

    // Request scopes for login (openid, email, profile) plus Gmail and Calendar access
    let scopes = [
        "openid",
        "email",
        "profile",
        "https://www.googleapis.com/auth/gmail.modify",
        "https://www.googleapis.com/auth/calendar",
    ]
    .join(" ");

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?\
         client_id={}&\
         redirect_uri={}&\
         response_type=code&\
         scope={}&\
         access_type=offline&\
         prompt=consent&\
         state={}",
        urlencoding::encode(&config.google_client_id),
        urlencoding::encode(&config.auth_redirect_uri),
        urlencoding::encode(&scopes),
        csrf_state
    );

    Ok(Json(LoginInitResponse { auth_url }))
}

#[derive(Debug, Deserialize)]
pub struct AuthCallbackParams {
    pub code: String,
    #[allow(dead_code)]
    pub state: String,
}

#[derive(Debug, Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    email: String,
    name: Option<String>,
}

/// Handle Google OAuth callback.
///
/// Exchanges the authorization code for tokens, validates the user's email
/// against the allowlist, and sets an auth cookie on success.
/// Also creates/updates email and calendar accounts with OAuth tokens.
pub async fn auth_callback(
    State(state): State<AppState>,
    Query(params): Query<AuthCallbackParams>,
) -> Response {
    match handle_callback_inner(&state, params).await {
        Ok(response) => response,
        Err(e) => {
            tracing::error!("Auth callback error: {:?}", e);
            Redirect::to("/?auth_error=auth_failed").into_response()
        }
    }
}

async fn handle_callback_inner(
    state: &AppState,
    params: AuthCallbackParams,
) -> Result<Response, ApiError> {
    let config = &state.auth_config;

    // Exchange code for access token
    let client = reqwest::Client::new();

    #[derive(serde::Serialize)]
    struct TokenRequest {
        code: String,
        client_id: String,
        client_secret: String,
        redirect_uri: String,
        grant_type: String,
    }

    let token_response = client
        .post("https://oauth2.googleapis.com/token")
        .form(&TokenRequest {
            code: params.code,
            client_id: config.google_client_id.clone(),
            client_secret: config.google_client_secret.clone(),
            redirect_uri: config.auth_redirect_uri.clone(),
            grant_type: "authorization_code".to_string(),
        })
        .send()
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Token exchange failed: {}", e)))?;

    if !token_response.status().is_success() {
        let status = token_response.status();
        let body = token_response.text().await.unwrap_or_default();
        tracing::error!("Token exchange failed: {} - {}", status, body);
        return Ok(Redirect::to("/?auth_error=token_exchange_failed").into_response());
    }

    let tokens: GoogleTokenResponse = token_response
        .json()
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Invalid token response: {}", e)))?;

    // Get user info
    let user_info: GoogleUserInfo = client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(&tokens.access_token)
        .send()
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to get user info: {}", e)))?
        .json()
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Invalid user info response: {}", e)))?;

    tracing::info!("OAuth login attempt from: {}", user_info.email);

    // Check if email is allowed
    if !config.is_email_allowed(&user_info.email) {
        tracing::warn!("Unauthorized login attempt from: {}", user_info.email);
        return Ok(Redirect::to("/?auth_error=unauthorized_email").into_response());
    }

    // Store OAuth tokens for email and calendar access if we got a refresh token
    if let Some(ref refresh_token) = tokens.refresh_token {
        if let Err(e) = store_oauth_tokens(
            &state.pool,
            &user_info.email,
            user_info.name.clone(),
            refresh_token,
            &tokens.access_token,
            tokens.expires_in,
        )
        .await
        {
            tracing::error!("Failed to store OAuth tokens: {:?}", e);
            // Continue with login even if token storage fails
        }
    } else {
        tracing::warn!("No refresh token received - email/calendar access may not work");
    }

    // Create JWT
    let token = jwt::create_token(config, &user_info.email, user_info.name.clone())
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create token: {}", e)))?;

    // Build cookie
    let cookie = build_auth_cookie(&config.cookie_name, &token, config.token_duration_days);

    tracing::info!("Successful login for: {}", user_info.email);

    // Redirect to app with cookie
    Ok((
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, "/"),
            (header::SET_COOKIE, cookie.as_str()),
        ],
    )
        .into_response())
}

/// Store OAuth tokens in google_accounts table
///
/// Both email and calendar pollers will look up tokens from google_accounts,
/// since we request all scopes (gmail.modify, calendar) in a single OAuth flow.
async fn store_oauth_tokens(
    pool: &crate::db::DbPool,
    email: &str,
    name: Option<String>,
    refresh_token: &str,
    access_token: &str,
    expires_in: Option<i64>,
) -> anyhow::Result<()> {
    use crate::db::{get_conn, google_accounts};
    use chrono::{Duration, Utc};

    let mut conn = get_conn(pool).await?;
    let expires_at = expires_in.map(|secs| Utc::now() + Duration::seconds(secs));

    // Upsert google account with OAuth tokens
    google_accounts::upsert(
        &mut conn,
        email,
        name.as_deref(),
        refresh_token,
        Some(access_token),
        expires_at,
    )
    .await?;

    tracing::info!(
        "Successfully stored OAuth tokens for: {} (grants Gmail and Calendar access)",
        email
    );
    Ok(())
}

/// Get current authenticated user info.
pub async fn auth_me(State(state): State<AppState>, headers: HeaderMap) -> Response {
    match extract_auth_user(&headers, &state.auth_config) {
        Ok(user) => Json(AuthUserResponse {
            email: user.email,
            name: user.name,
        })
        .into_response(),
        Err(err) => err.into_response(),
    }
}

/// Logout - clear auth cookie.
pub async fn auth_logout() -> impl IntoResponse {
    let cookie = "auth_token=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0";

    (
        StatusCode::SEE_OTHER,
        [(header::LOCATION, "/"), (header::SET_COOKIE, cookie)],
    )
}
