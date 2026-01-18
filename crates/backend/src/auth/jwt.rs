//! JWT token creation and validation.

use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};

use super::types::{AuthConfig, Claims};

/// Create a new JWT token for a user.
pub fn create_token(
    config: &AuthConfig,
    email: &str,
    name: Option<String>,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let exp = now + Duration::days(config.token_duration_days);

    let claims = Claims {
        sub: email.to_string(),
        name,
        iat: now.timestamp(),
        exp: exp.timestamp(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
}

/// Validate a JWT token and return claims.
pub fn validate_token(
    config: &AuthConfig,
    token: &str,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &Validation::default(),
    )?;

    Ok(token_data.claims)
}

/// Check if token should be refreshed (older than 1 day).
pub fn should_refresh(claims: &Claims) -> bool {
    let now = Utc::now().timestamp();
    let age_seconds = now - claims.iat;
    let one_day_seconds = 86400;
    age_seconds > one_day_seconds
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> AuthConfig {
        AuthConfig {
            jwt_secret: "test-secret-key-for-testing-only".to_string(),
            allowed_emails: vec!["test@example.com".to_string()],
            token_duration_days: 7,
            cookie_name: "auth_token".to_string(),
            google_client_id: "test".to_string(),
            google_client_secret: "test".to_string(),
            auth_redirect_uri: "http://localhost/callback".to_string(),
        }
    }

    #[test]
    fn test_create_and_validate_token() {
        let config = test_config();
        let token = create_token(&config, "test@example.com", Some("Test User".to_string()))
            .expect("should create token");

        let claims = validate_token(&config, &token).expect("should validate token");
        assert_eq!(claims.sub, "test@example.com");
        assert_eq!(claims.name, Some("Test User".to_string()));
    }

    #[test]
    fn test_invalid_token_rejected() {
        let config = test_config();
        let result = validate_token(&config, "invalid-token");
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_secret_rejected() {
        let config = test_config();
        let token = create_token(&config, "test@example.com", None).expect("should create token");

        let mut wrong_config = config;
        wrong_config.jwt_secret = "wrong-secret".to_string();

        let result = validate_token(&wrong_config, &token);
        assert!(result.is_err());
    }
}
