use std::env;

use tower_http::cors::{AllowOrigin, CorsLayer};

use axum::http::{header, HeaderValue, Method};

/// Server configuration resolved from the environment.
///
/// Each value falls back to a sensible default and is logged at startup, so the
/// effective configuration is always visible in the logs.
#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    /// Origins allowed by CORS. Empty means "not configured" — permissive CORS
    /// is used, which is only appropriate for local development.
    pub cors_allowed_origins: Vec<String>,
}

impl Config {
    /// Read configuration from the environment, applying defaults and logging
    /// each resolved value.
    pub fn from_env() -> Self {
        let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var("PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3000);
        let cors_allowed_origins = env::var("CORS_ALLOWED_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();

        tracing::info!("Config: HOST={host}");
        tracing::info!("Config: PORT={port}");
        if cors_allowed_origins.is_empty() {
            tracing::warn!(
                "Config: CORS_ALLOWED_ORIGINS unset, using permissive CORS (not recommended for production)"
            );
        } else {
            tracing::info!("Config: CORS_ALLOWED_ORIGINS={cors_allowed_origins:?}");
        }

        Self {
            host,
            port,
            cors_allowed_origins,
        }
    }

    /// The `host:port` address to bind the server to.
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Build the CORS layer for the configured origins.
    ///
    /// Origins that fail to parse as header values are dropped with a warning;
    /// if that leaves nothing usable, CORS falls back to permissive.
    pub fn cors_layer(&self) -> CorsLayer {
        let origins: Vec<HeaderValue> = self
            .cors_allowed_origins
            .iter()
            .filter_map(|origin| match origin.parse::<HeaderValue>() {
                Ok(value) => Some(value),
                Err(_) => {
                    tracing::warn!("Ignoring unparseable CORS origin: {origin}");
                    None
                }
            })
            .collect();

        if origins.is_empty() {
            return CorsLayer::permissive();
        }

        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
            .allow_credentials(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_addr_joins_host_and_port() {
        let config = Config {
            host: "127.0.0.1".to_string(),
            port: 8080,
            cors_allowed_origins: vec![],
        };
        assert_eq!(config.bind_addr(), "127.0.0.1:8080");
    }
}
