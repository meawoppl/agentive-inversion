//! Authentication middleware layer for protecting routes.

use axum::{
    body::Body,
    extract::State,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

use crate::error::ErrorResponse;
use crate::AppState;

use super::jwt;
use super::types::{AuthConfig, AuthUser, Claims};

/// Middleware function that requires authentication.
///
/// This can be used with `axum::middleware::from_fn_with_state` to protect routes.
pub async fn require_auth(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let config = &state.auth_config;

    // Try to get token from cookie first, then Authorization header
    let token = extract_token_from_cookie(request.headers(), &config.cookie_name)
        .or_else(|| extract_token_from_header(request.headers()));

    let token = match token {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Missing authentication".to_string(),
                    details: None,
                }),
            )
                .into_response();
        }
    };

    let claims = match jwt::validate_token(config, &token) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Invalid or expired token".to_string(),
                    details: None,
                }),
            )
                .into_response();
        }
    };

    // Verify email is still allowed
    if !config.is_email_allowed(&claims.sub) {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Email not authorized".to_string(),
                details: None,
            }),
        )
            .into_response();
    }

    // If token should be refreshed, we could add a Set-Cookie header here
    // but for simplicity we just proceed with the request
    let response = next.run(request).await;

    // Optionally add refresh cookie if needed
    if jwt::should_refresh(&claims) {
        if let Ok(new_token) = jwt::create_token(config, &claims.sub, claims.name.clone()) {
            let cookie =
                build_auth_cookie(&config.cookie_name, &new_token, config.token_duration_days);
            // Add Set-Cookie header to response
            let (mut parts, body) = response.into_parts();
            if let Ok(cookie_value) = cookie.parse() {
                parts.headers.insert(header::SET_COOKIE, cookie_value);
            }
            return Response::from_parts(parts, body);
        }
    }

    response
}

fn extract_token_from_cookie(headers: &axum::http::HeaderMap, cookie_name: &str) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;

    for cookie_str in cookie_header.split(';') {
        if let Ok(cookie) = cookie::Cookie::parse(cookie_str.trim()) {
            if cookie.name() == cookie_name {
                return Some(cookie.value().to_string());
            }
        }
    }

    None
}

fn extract_token_from_header(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(|s| s.to_string())
}

/// Build an auth cookie string.
pub fn build_auth_cookie(name: &str, value: &str, days: i64) -> String {
    let max_age = days * 24 * 60 * 60;
    let secure = if std::env::var("RUST_ENV").unwrap_or_default() == "production" {
        "; Secure"
    } else {
        ""
    };
    format!(
        "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{}",
        name, value, max_age, secure
    )
}

/// Extract and validate user from request headers.
///
/// Returns the authenticated user if the token is valid and email is allowed.
pub fn extract_auth_user(
    headers: &axum::http::HeaderMap,
    config: &AuthConfig,
) -> Result<AuthUser, (StatusCode, Json<ErrorResponse>)> {
    let token = extract_token_from_cookie(headers, &config.cookie_name)
        .or_else(|| extract_token_from_header(headers))
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Missing authentication".to_string(),
                    details: None,
                }),
            )
        })?;

    let claims: Claims = jwt::validate_token(config, &token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Invalid or expired token".to_string(),
                details: None,
            }),
        )
    })?;

    if !config.is_email_allowed(&claims.sub) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Email not authorized".to_string(),
                details: None,
            }),
        ));
    }

    Ok(AuthUser {
        email: claims.sub,
        name: claims.name,
    })
}
