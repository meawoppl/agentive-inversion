//! Auth-related types and configuration.

use serde::{Deserialize, Serialize};

// Re-export shared types for convenience
pub use shared_types::{AuthUserResponse, LoginInitResponse};

/// JWT Claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user email)
    pub sub: String,
    /// User display name from Google
    pub name: Option<String>,
    /// Issued at timestamp
    pub iat: i64,
    /// Expiration timestamp
    pub exp: i64,
}

/// Validated user from JWT
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub email: String,
    pub name: Option<String>,
}

/// Auth configuration loaded from environment
#[derive(Clone)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub allowed_emails: Vec<String>,
    pub token_duration_days: i64,
    pub cookie_name: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub auth_redirect_uri: String,
}

impl AuthConfig {
    /// Load auth configuration from environment variables.
    ///
    /// Required env vars:
    /// - `JWT_SECRET`: Secret key for signing JWTs
    /// - `ALLOWED_EMAILS`: Comma-separated list of allowed email addresses
    /// - `GOOGLE_CLIENT_ID`: Google OAuth client ID
    /// - `GOOGLE_CLIENT_SECRET`: Google OAuth client secret
    /// - `AUTH_REDIRECT_URI`: OAuth callback URI for user login
    pub fn from_env() -> Result<Self, String> {
        let jwt_secret =
            std::env::var("JWT_SECRET").map_err(|_| "JWT_SECRET must be set".to_string())?;

        let allowed_emails: Vec<String> = std::env::var("ALLOWED_EMAILS")
            .map_err(|_| "ALLOWED_EMAILS must be set".to_string())?
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        if allowed_emails.is_empty() {
            return Err("ALLOWED_EMAILS cannot be empty".to_string());
        }

        Ok(Self {
            jwt_secret,
            allowed_emails,
            token_duration_days: 7,
            cookie_name: "auth_token".to_string(),
            google_client_id: std::env::var("GOOGLE_CLIENT_ID")
                .map_err(|_| "GOOGLE_CLIENT_ID must be set".to_string())?,
            google_client_secret: std::env::var("GOOGLE_CLIENT_SECRET")
                .map_err(|_| "GOOGLE_CLIENT_SECRET must be set".to_string())?,
            auth_redirect_uri: std::env::var("AUTH_REDIRECT_URI")
                .map_err(|_| "AUTH_REDIRECT_URI must be set".to_string())?,
        })
    }

    /// Check if an email address is in the allowed list.
    pub fn is_email_allowed(&self, email: &str) -> bool {
        self.allowed_emails.contains(&email.to_lowercase())
    }
}
