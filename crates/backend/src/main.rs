use axum::{
    http::{HeaderValue, Method, StatusCode},
    routing::{delete, get, post, put},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};

mod db;
mod handlers;
mod models;
mod schema;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    dotenvy::dotenv().ok();

    // Establish database connection pool
    let pool = db::establish_connection_pool()?;

    let app = Router::new()
        .route("/health", get(health_check))
        // Todo routes
        .route("/api/todos", get(handlers::list_todos))
        .route("/api/todos", post(handlers::create_todo))
        .route("/api/todos/:id", put(handlers::update_todo))
        .route("/api/todos/:id", delete(handlers::delete_todo))
        // Email account routes
        .route("/api/email-accounts", get(handlers::list_email_accounts))
        .route("/api/email-accounts", post(handlers::start_gmail_oauth))
        .route(
            "/api/email-accounts/:id",
            delete(handlers::delete_email_account),
        )
        // OAuth routes
        .route(
            "/api/email-accounts/oauth/callback",
            get(handlers::gmail_oauth_callback),
        )
        // Category routes
        .route("/api/categories", get(handlers::list_categories))
        .route("/api/categories", post(handlers::create_category))
        .route("/api/categories/:id", put(handlers::update_category))
        .route("/api/categories/:id", delete(handlers::delete_category))
        // Email routes
        .route("/api/emails", get(handlers::list_emails))
        .route("/api/emails/stats", get(handlers::get_email_stats))
        .route("/api/emails/:id", get(handlers::get_email))
        // Agent decision routes
        .route("/api/decisions", get(handlers::list_decisions))
        .route("/api/decisions", post(handlers::create_decision))
        .route(
            "/api/decisions/pending",
            get(handlers::list_pending_decisions),
        )
        .route("/api/decisions/stats", get(handlers::get_decision_stats))
        .route("/api/decisions/:id", get(handlers::get_decision))
        .route(
            "/api/decisions/:id/approve",
            post(handlers::approve_decision),
        )
        .route("/api/decisions/:id/reject", post(handlers::reject_decision))
        // Agent rules routes
        .route("/api/rules", get(handlers::list_agent_rules))
        .route("/api/rules", post(handlers::create_agent_rule))
        .route("/api/rules/:id", get(handlers::get_agent_rule))
        .route("/api/rules/:id", put(handlers::update_agent_rule))
        .route("/api/rules/:id", delete(handlers::delete_agent_rule))
        .route(
            "/api/rules/:id/toggle",
            post(handlers::toggle_agent_rule_active),
        )
        .layer(build_cors_layer())
        .with_state(pool);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Backend server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> StatusCode {
    StatusCode::OK
}

fn build_cors_layer() -> CorsLayer {
    // Check for CORS_ALLOWED_ORIGINS env var
    // Format: comma-separated list of origins, e.g., "https://example.com,https://app.example.com"
    // If not set, allow all origins (development mode)
    match std::env::var("CORS_ALLOWED_ORIGINS") {
        Ok(origins) if !origins.is_empty() => {
            let allowed_origins: Vec<HeaderValue> = origins
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();

            if allowed_origins.is_empty() {
                tracing::warn!(
                    "CORS_ALLOWED_ORIGINS set but no valid origins found, using permissive CORS"
                );
                CorsLayer::permissive()
            } else {
                tracing::info!("CORS configured for origins: {}", origins);
                CorsLayer::new()
                    .allow_origin(allowed_origins)
                    .allow_methods([
                        Method::GET,
                        Method::POST,
                        Method::PUT,
                        Method::DELETE,
                        Method::OPTIONS,
                    ])
                    .allow_headers(Any)
                    .allow_credentials(true)
            }
        }
        _ => {
            tracing::warn!(
                "CORS_ALLOWED_ORIGINS not set, using permissive CORS (not recommended for production)"
            );
            CorsLayer::permissive()
        }
    }
}
