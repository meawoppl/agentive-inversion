use axum::{
    http::StatusCode,
    routing::{delete, get, post, put},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

mod db;
mod handlers;
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
        .layer(CorsLayer::permissive())
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
