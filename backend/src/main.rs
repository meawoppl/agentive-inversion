mod db;
mod handlers;
mod middleware;
mod routes;
mod error;
mod config;

use anyhow::Result;
use axum::Router;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::AppConfig;
use crate::db::DbPool;
use crate::routes::api_routes;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend=debug,tower_http=debug,axum=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    dotenv::dotenv().ok();
    let config = AppConfig::from_env()?;

    tracing::info!("Starting Agentive Inversion backend server");

    // Initialize database pool
    let pool = DbPool::new(&config.database_url).await?;
    tracing::info!("Database connection pool initialized");

    // Build application
    let app = create_app(pool).await?;

    // Run server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn create_app(pool: DbPool) -> Result<Router> {
    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build router
    let app = Router::new()
        .nest("/api", api_routes())
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(pool);

    Ok(app)
}
