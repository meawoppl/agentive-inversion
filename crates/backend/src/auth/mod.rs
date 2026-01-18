//! Authentication module for JWT-based auth with Google OAuth login.
//!
//! This module provides:
//! - JWT token creation and validation
//! - Google OAuth flow for user login
//! - `require_auth` middleware for protecting routes
//! - Email allowlist validation

mod handlers;
mod jwt;
mod middleware;
pub mod types;

pub use handlers::{auth_callback, auth_login, auth_logout, auth_me};
pub use middleware::{build_auth_cookie, extract_auth_user, require_auth};
