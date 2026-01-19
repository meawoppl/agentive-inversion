use axum::{
    http::{header, Method, StatusCode},
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use std::{net::SocketAddr, sync::Arc};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    services::{ServeDir, ServeFile},
};

mod auth;
mod db;
pub mod error;
mod handlers;
mod models;
mod pollers;
pub mod repository;
mod schema;
mod services;

use auth::types::AuthConfig;

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    pub pool: db::DbPool,
    pub auth_config: Arc<AuthConfig>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Install rustls crypto provider before any TLS operations
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    tracing_subscriber::fmt::init();

    dotenvy::dotenv().ok();

    // Establish database connection pool
    let pool = db::establish_connection_pool()?;

    // Load auth configuration once at startup
    let auth_config =
        Arc::new(AuthConfig::from_env().map_err(|e| anyhow::anyhow!("Auth config error: {}", e))?);
    tracing::info!(
        "Auth configured for {} allowed email(s)",
        auth_config.allowed_emails.len()
    );

    let app_state = AppState {
        pool: pool.clone(),
        auth_config,
    };

    // Start email polling background task
    let email_poll_pool = pool.clone();
    tokio::spawn(async move {
        pollers::start_email_polling_task(email_poll_pool).await;
    });

    // Start calendar polling background task (stub - not yet implemented)
    let calendar_poll_pool = pool.clone();
    tokio::spawn(async move {
        pollers::start_calendar_polling_task(calendar_poll_pool).await;
    });

    // Protected API routes (require authentication)
    let protected_routes = Router::new()
        // Todo routes
        .route("/todos", get(handlers::list_todos))
        .route("/todos", post(handlers::create_todo))
        .route("/todos/:id", put(handlers::update_todo))
        .route("/todos/:id", delete(handlers::delete_todo))
        // Email account routes
        .route("/email-accounts", get(handlers::list_email_accounts))
        .route("/email-accounts", post(handlers::start_gmail_oauth))
        .route(
            "/email-accounts/:id",
            delete(handlers::delete_email_account),
        )
        .route(
            "/email-accounts/oauth/callback",
            get(handlers::gmail_oauth_callback),
        )
        // Category routes
        .route("/categories", get(handlers::list_categories))
        .route("/categories", post(handlers::create_category))
        .route("/categories/:id", put(handlers::update_category))
        .route("/categories/:id", delete(handlers::delete_category))
        // Email routes
        .route("/emails", get(handlers::list_emails))
        .route("/emails/stats", get(handlers::get_email_stats))
        .route("/emails/:id", get(handlers::get_email))
        // Agent decision routes
        .route("/decisions", get(handlers::list_decisions))
        .route("/decisions", post(handlers::create_decision))
        .route("/decisions/pending", get(handlers::list_pending_decisions))
        .route("/decisions/stats", get(handlers::get_decision_stats))
        .route("/decisions/:id", get(handlers::get_decision))
        .route("/decisions/:id/approve", post(handlers::approve_decision))
        .route("/decisions/:id/reject", post(handlers::reject_decision))
        .route(
            "/decisions/batch/approve",
            post(handlers::batch_approve_decisions),
        )
        .route(
            "/decisions/batch/reject",
            post(handlers::batch_reject_decisions),
        )
        // Agent rules routes
        .route("/rules", get(handlers::list_agent_rules))
        .route("/rules", post(handlers::create_agent_rule))
        .route("/rules/:id", get(handlers::get_agent_rule))
        .route("/rules/:id", put(handlers::update_agent_rule))
        .route("/rules/:id", delete(handlers::delete_agent_rule))
        .route(
            "/rules/:id/toggle",
            post(handlers::toggle_agent_rule_active),
        )
        // Chat routes
        .route("/chat", post(handlers::send_chat_message))
        .route("/chat/history", get(handlers::get_chat_history))
        .route("/chat/history", delete(handlers::clear_chat_history))
        // Calendar event routes
        .route("/calendar-events", get(handlers::list_calendar_events))
        .route("/calendar-events/today", get(handlers::get_todays_events))
        .route(
            "/calendar-events/week",
            get(handlers::get_this_weeks_events),
        )
        .route("/calendar-events/:id", get(handlers::get_calendar_event))
        // Calendar account routes
        .route("/calendar-accounts", get(handlers::list_calendar_accounts))
        .route(
            "/calendar-accounts",
            post(handlers::create_calendar_account),
        )
        .route(
            "/calendar-accounts/:id",
            delete(handlers::delete_calendar_account),
        )
        .route(
            "/calendar-accounts/:id/toggle",
            post(handlers::toggle_calendar_account),
        )
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            auth::require_auth,
        ));

    let app = Router::new()
        .route("/health", get(health_check))
        // Public auth routes
        .route("/api/auth/login", get(auth::auth_login))
        .route("/api/auth/callback", get(auth::auth_callback))
        .route("/api/auth/logout", post(auth::auth_logout))
        .route("/api/auth/me", get(auth::auth_me))
        // Mount protected routes under /api
        .nest("/api", protected_routes)
        .layer(build_cors_layer())
        .with_state(app_state);

    // Serve static frontend files if the directory exists
    let frontend_dir =
        std::env::var("FRONTEND_DIR").unwrap_or_else(|_| "frontend/dist".to_string());
    let app = if std::path::Path::new(&frontend_dir).exists() {
        tracing::info!("Serving frontend from {}", frontend_dir);
        let index_path = format!("{}/index.html", frontend_dir);
        let serve_dir = ServeDir::new(&frontend_dir).not_found_service(ServeFile::new(&index_path));
        app.fallback_service(serve_dir)
    } else {
        tracing::info!(
            "Frontend directory not found at {}, serving API only",
            frontend_dir
        );
        app
    };

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> StatusCode {
    StatusCode::OK
}

/// Build CORS layer based on environment configuration.
///
/// If CORS_ALLOWED_ORIGINS is set, only those origins are allowed.
/// If not set, defaults to permissive CORS (for development only).
fn build_cors_layer() -> CorsLayer {
    let allowed_origins = std::env::var("CORS_ALLOWED_ORIGINS").ok();

    match allowed_origins {
        Some(origins) => {
            let origins: Vec<_> = origins
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();

            if origins.is_empty() {
                tracing::warn!(
                    "CORS_ALLOWED_ORIGINS is set but empty, using permissive CORS (not recommended for production)"
                );
                CorsLayer::permissive()
            } else {
                tracing::info!("CORS configured for origins: {:?}", origins);
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
        None => {
            tracing::warn!(
                "CORS_ALLOWED_ORIGINS not set, using permissive CORS (not recommended for production)"
            );
            CorsLayer::permissive()
        }
    }
}
