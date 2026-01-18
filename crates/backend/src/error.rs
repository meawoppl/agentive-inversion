//! Unified error handling for the backend API.
//!
//! This module provides a centralized error type that implements `IntoResponse`,
//! allowing handlers to use `?` operator naturally while returning appropriate
//! HTTP status codes and error messages.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

/// API error response body
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Unified error type for API handlers
#[derive(Debug, Error)]
pub enum ApiError {
    /// Database connection pool error
    #[error("Database connection error")]
    ConnectionPool(#[source] diesel_async::pooled_connection::deadpool::PoolError),

    /// Database query error
    #[error("Database error: {0}")]
    Database(#[from] diesel::result::Error),

    /// Generic database/anyhow error
    #[error("{0}")]
    Internal(#[from] anyhow::Error),

    /// Resource not found
    #[error("{0} not found")]
    NotFound(String),

    /// Invalid request data
    #[error("Invalid request: {0}")]
    BadRequest(String),

    /// JSON parsing error
    #[error("Invalid JSON: {0}")]
    JsonParse(#[from] serde_json::Error),

    /// Environment variable missing
    #[error("Configuration error: {0}")]
    Config(String),

    /// Authentication required but not provided or invalid
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Authenticated but not permitted to access resource
    #[error("Forbidden: {0}")]
    Forbidden(String),
}

impl ApiError {
    /// Create a not found error with a custom message
    pub fn not_found(resource: impl Into<String>) -> Self {
        ApiError::NotFound(resource.into())
    }

    /// Create a bad request error
    pub fn bad_request(message: impl Into<String>) -> Self {
        ApiError::BadRequest(message.into())
    }

    /// Create a config error for missing env vars
    pub fn missing_env(var_name: &str) -> Self {
        ApiError::Config(format!("{} environment variable must be set", var_name))
    }
}

impl From<diesel_async::pooled_connection::deadpool::PoolError> for ApiError {
    fn from(err: diesel_async::pooled_connection::deadpool::PoolError) -> Self {
        ApiError::ConnectionPool(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message, details) = match &self {
            ApiError::ConnectionPool(e) => {
                tracing::error!("Connection pool error: {:?}", e);
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Database connection unavailable".to_string(),
                    None,
                )
            }
            ApiError::Database(e) => {
                tracing::error!("Database error: {:?}", e);
                match e {
                    diesel::result::Error::NotFound => (
                        StatusCode::NOT_FOUND,
                        "Resource not found".to_string(),
                        None,
                    ),
                    _ => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Database operation failed".to_string(),
                        None,
                    ),
                }
            }
            ApiError::Internal(e) => {
                tracing::error!("Internal error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                    Some(e.to_string()),
                )
            }
            ApiError::NotFound(resource) => (
                StatusCode::NOT_FOUND,
                format!("{} not found", resource),
                None,
            ),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone(), None),
            ApiError::JsonParse(e) => {
                tracing::warn!("JSON parse error: {:?}", e);
                (
                    StatusCode::BAD_REQUEST,
                    "Invalid JSON format".to_string(),
                    Some(e.to_string()),
                )
            }
            ApiError::Config(msg) => {
                tracing::error!("Configuration error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Server configuration error".to_string(),
                    None,
                )
            }
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone(), None),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone(), None),
        };

        let body = Json(ErrorResponse {
            error: error_message,
            details,
        });

        (status, body).into_response()
    }
}

/// Result type alias for API handlers
pub type ApiResult<T> = Result<T, ApiError>;
