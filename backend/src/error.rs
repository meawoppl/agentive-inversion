use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use shared::api::ErrorResponse;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Database(diesel::result::Error),
    NotFound(String),
    Validation(String),
    Unauthorized(String),
    Internal(anyhow::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(e) => write!(f, "Database error: {}", e),
            Self::NotFound(msg) => write!(f, "Not found: {}", msg),
            Self::Validation(msg) => write!(f, "Validation error: {}", msg),
            Self::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            Self::Internal(e) => write!(f, "Internal error: {}", e),
        }
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_response) = match self {
            Self::Database(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorResponse::new("database_error", e.to_string()),
            ),
            Self::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                ErrorResponse::new("not_found", msg),
            ),
            Self::Validation(msg) => (
                StatusCode::BAD_REQUEST,
                ErrorResponse::new("validation_error", msg),
            ),
            Self::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                ErrorResponse::new("unauthorized", msg),
            ),
            Self::Internal(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorResponse::new("internal_error", e.to_string()),
            ),
        };

        (status, Json(error_response)).into_response()
    }
}

impl From<diesel::result::Error> for AppError {
    fn from(err: diesel::result::Error) -> Self {
        Self::Database(err)
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err)
    }
}

pub type ApiResult<T> = Result<T, AppError>;
