use axum::{
    extract::State,
    http::StatusCode,
    middleware,
    routing::{delete, get, post, put},
    Json, Router,
};
use clap::Parser;
use memory_serve::{load_assets, CacheControl, MemoryServe};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod auth;
mod calendar_writer;
mod config;
mod db;
pub mod error;
mod handlers;
mod models;
mod pollers;
pub mod repository;
mod schema;
mod services;

use auth::types::AuthConfig;
use config::Config;
use shared_types::HealthResponse;

#[derive(Parser, Debug, Clone)]
#[command(name = "backend")]
#[command(about = "Agentive Inversion backend server")]
struct Args {
    /// Skip starting the Gmail and Calendar background pollers.
    ///
    /// Local development normally wants the API and UI without polling live
    /// mailboxes every five minutes.
    #[arg(long)]
    dev_mode: bool,
}

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    pub pool: db::DbPool,
    pub auth_config: Arc<AuthConfig>,
    pub triage_health: pollers::TriageHealth,
    /// In-flight interactive Claude Code login with its start time, if any
    /// (single-user app: one flow at a time; replacing a flow cancels the
    /// previous one, and a reaper expires abandoned flows after a TTL so a
    /// forgotten login can't leave a PTY child alive indefinitely)
    pub claude_login:
        Arc<std::sync::Mutex<Option<(claude_codes::auth::LoginFlow, std::time::Instant)>>>,
}

/// Build the full application router from shared state.
///
/// Kept as a pure function of its inputs, separate from `main()`, so tests can
/// drive the entire app in-process via `tower::ServiceExt::oneshot` — no bound
/// port, no network.
pub fn build_app(state: AppState, config: &Config) -> Router {
    // Protected API routes (require authentication)
    let protected_routes = Router::new()
        // Todo routes
        .route("/todos", get(handlers::list_todos))
        .route("/todos", post(handlers::create_todo))
        .route("/todos/:id", put(handlers::update_todo))
        .route("/todos/:id", delete(handlers::delete_todo))
        // Google account routes (for viewing connected accounts)
        .route("/google-accounts", get(handlers::list_google_accounts))
        // About-me document (personal context for the triage agent)
        .route("/about-me", get(handlers::get_about_me))
        .route("/about-me", put(handlers::update_about_me))
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
        // Triage pipeline routes (agent-cli + pipeline screen)
        .route("/triage/decisions", post(handlers::post_triage_decision))
        .route("/pipeline/stats", get(handlers::get_pipeline_stats))
        // Claude Code login flow (subscription auth for the triage pipeline)
        .route("/claude-auth/start", post(handlers::claude_auth_start))
        .route(
            "/claude-auth/complete",
            post(handlers::claude_auth_complete),
        )
        .route("/claude-auth/status", get(handlers::claude_auth_status))
        .route(
            "/pipeline/archive-review",
            get(handlers::get_archive_review),
        )
        // Chat routes
        .route("/chat", post(handlers::send_chat_message))
        .route("/chat/history", get(handlers::get_chat_history))
        .route("/chat/history", delete(handlers::clear_chat_history))
        // Calendar event routes
        .route(
            "/calendar-events/create",
            post(handlers::create_calendar_event),
        )
        .route("/calendar-events", get(handlers::list_calendar_events))
        .route("/calendar-events/today", get(handlers::get_todays_events))
        .route(
            "/calendar-events/week",
            get(handlers::get_this_weeks_events),
        )
        .route("/calendar-events/:id", get(handlers::get_calendar_event))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth,
        ));

    // Pre-compressed (brotli/gzip), content-negotiated frontend assets with
    // ETag/304 and an SPA fallback to index.html. Assets are embedded in the
    // binary in release builds and read from disk in debug builds.
    let frontend = MemoryServe::new(load_assets!("../frontend/dist"))
        .index_file(Some("/index.html"))
        .fallback(Some("/index.html"))
        .fallback_status(StatusCode::OK)
        .html_cache_control(CacheControl::NoCache)
        .cache_control(CacheControl::Long)
        .into_router();

    Router::new()
        // `/health` is what the production Traefik and Docker probes hit;
        // `/api/health` is the skeleton convention.
        .route("/health", get(health_check))
        .route("/api/health", get(health_check))
        // Public auth routes
        .route("/api/auth/login", get(auth::auth_login))
        .route("/api/auth/callback", get(auth::auth_callback))
        .route("/api/auth/logout", post(auth::auth_logout))
        .route("/api/auth/me", get(auth::auth_me))
        // Mount protected routes under /api
        .nest("/api", protected_routes)
        .with_state(state)
        .merge(frontend)
        .layer(config.cors_layer())
}

/// Install the global tracing subscriber, exactly once.
///
/// Do NOT add a separate `tracing_log::LogTracer::init()` here: `.init()`
/// already installs the log-facade bridge (tracing-subscriber's default
/// `tracing-log` feature), so log-crate records from dependencies such as
/// claude-codes reach this subscriber. A second install returns
/// `SetLoggerError`, which `.init()` unwraps into a panic before the server
/// binds — that crash-looped production on 2026-08-03 (#106).
fn init_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Install rustls crypto provider before any TLS operations
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    init_tracing();

    // Log panics with their source location and a backtrace via tracing, so
    // crashes are captured in structured logs rather than only on stderr.
    // Set RUST_BACKTRACE=1 to populate the backtrace.
    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = std::backtrace::Backtrace::capture();
        match panic_info.location() {
            Some(loc) => tracing::error!(
                "PANIC at {}:{}:{}: {}",
                loc.file(),
                loc.line(),
                loc.column(),
                panic_info
            ),
            None => tracing::error!("PANIC: {}", panic_info),
        }
        tracing::error!("Backtrace:\n{backtrace}");
    }));

    if args.dev_mode {
        tracing::warn!("DEV MODE ENABLED — background pollers will not start");
    }

    dotenvy::dotenv().ok();

    // Build provenance: which claude-codes rev this binary actually contains.
    // Deploy gates read this line instead of grepping the binary, which can
    // false-negative on optimized-out literals.
    tracing::info!(
        "Build provenance: claude-codes rev {}",
        env!("CLAUDE_CODES_REV")
    );

    let config = Config::from_env();

    // Migrations are embedded in the binary and applied before anything else
    // touches the schema.
    tracing::info!("Running database migrations...");
    match db::run_migrations() {
        Ok(applied) => {
            if applied.is_empty() {
                tracing::info!("Database is up to date");
            } else {
                for m in &applied {
                    tracing::info!("Applied migration: {}", m);
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to run migrations: {:#}", e);
            return Err(e);
        }
    }

    // Establish database connection pool
    let pool = db::establish_connection_pool()?;

    // Load auth configuration once at startup
    let auth_config =
        Arc::new(AuthConfig::from_env().map_err(|e| anyhow::anyhow!("Auth config error: {}", e))?);
    tracing::info!(
        "Auth configured for {} allowed email(s)",
        auth_config.allowed_emails.len()
    );

    let triage_health: pollers::TriageHealth = Arc::new(tokio::sync::RwLock::new(
        pollers::TriageHealthState::default(),
    ));

    let app_state = AppState {
        pool: pool.clone(),
        auth_config: auth_config.clone(),
        triage_health: triage_health.clone(),
        claude_login: Arc::new(std::sync::Mutex::new(None)),
    };

    // Reap abandoned Claude login flows so a forgotten PTY child can't
    // linger inside the container's memory budget (runs in dev mode too:
    // the login flow exists regardless of the pollers)
    let login_reaper_state = app_state.claude_login.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            tick.tick().await;
            let expired = {
                let mut slot = login_reaper_state
                    .lock()
                    .expect("claude_login mutex poisoned");
                match slot.as_ref() {
                    Some((_, started)) if started.elapsed() > handlers::LOGIN_FLOW_TTL => {
                        slot.take()
                    }
                    _ => None,
                }
            };
            if expired.is_some() {
                tracing::info!("Reaped abandoned Claude login flow (TTL exceeded)");
            }
        }
    });

    if !args.dev_mode {
        // Start email polling background task (ingestion is never gated on triage)
        let email_poll_pool = pool.clone();
        tokio::spawn(async move {
            pollers::start_email_polling_task(email_poll_pool).await;
        });

        // Start the agentic triage pipeline task
        let triage_pool = pool.clone();
        let triage_auth = auth_config.clone();
        tokio::spawn(async move {
            pollers::start_triage_task(triage_pool, triage_auth, triage_health).await;
        });

        // Start calendar polling background task (stub - not yet implemented)
        let calendar_poll_pool = pool.clone();
        tokio::spawn(async move {
            pollers::start_calendar_polling_task(calendar_poll_pool).await;
        });
    }

    let app = build_app(app_state, &config);

    let listener = tokio::net::TcpListener::bind(config.bind_addr()).await?;
    tracing::info!("Listening on {}", listener.local_addr()?);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Health check that verifies database connectivity, so an "up" container
/// that can't reach Postgres still reads as unhealthy
async fn health_check(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    match db::ping(&state.pool).await {
        Ok(()) => (
            StatusCode::OK,
            Json(HealthResponse {
                status: "ok".to_string(),
            }),
        ),
        Err(e) => {
            tracing::error!("Health check failed: {:#}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthResponse {
                    status: "database unavailable".to_string(),
                }),
            )
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("Received Ctrl+C, shutting down..."),
        _ = terminate => tracing::info!("Received SIGTERM, shutting down..."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{header, Request};
    use diesel_async::pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager};
    use diesel_async::AsyncPgConnection;
    use tower::ServiceExt;

    /// Boot smoke test: init_tracing() performs two global installs (the
    /// tracing dispatcher and, via tracing-subscriber's `tracing-log`
    /// feature, the log-facade logger). Both are set-once process-wide, so a
    /// duplicate install anywhere in the init path panics here the same way
    /// it would in main() — which took production down on 2026-08-03 (#106)
    /// with every CI check green. Keep this the ONLY test that installs a
    /// subscriber.
    #[test]
    fn init_tracing_boots_without_panicking() {
        init_tracing();
    }

    /// State with a pool that is never actually connected. deadpool builds
    /// connections lazily, so routes that don't touch the database — asset
    /// serving and the SPA fallback — can be exercised without a Postgres.
    fn test_state() -> AppState {
        let manager = AsyncDieselConnectionManager::<AsyncPgConnection>::new(
            "postgres://127.0.0.1:1/nonexistent",
        );
        let pool = Pool::builder(manager).build().unwrap();
        AppState {
            pool,
            triage_health: Arc::new(tokio::sync::RwLock::new(
                pollers::TriageHealthState::default(),
            )),
            auth_config: Arc::new(AuthConfig {
                jwt_secret: "test-secret".to_string(),
                allowed_emails: vec!["test@example.com".to_string()],
                token_duration_days: 7,
                cookie_name: "auth_token".to_string(),
                google_client_id: "test-client-id".to_string(),
                google_client_secret: "test-client-secret".to_string(),
                auth_redirect_uri: "http://localhost:3000/api/auth/callback".to_string(),
            }),
            claude_login: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    fn test_config() -> Config {
        Config {
            host: "127.0.0.1".to_string(),
            port: 3000,
            cors_allowed_origins: vec![],
        }
    }

    fn app() -> Router {
        build_app(test_state(), &test_config())
    }

    #[tokio::test]
    async fn health_reports_unavailable_without_a_database() {
        let resp = app()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let parsed: HealthResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed.status, "database unavailable");
    }

    #[tokio::test]
    async fn api_health_is_routed_alongside_the_nested_api_routes() {
        let resp = app()
            .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        // Reaches the handler (rather than the auth middleware or a 404).
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn index_served_as_html() {
        let resp = app()
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(ct.contains("html"), "expected HTML, got {ct}");
    }

    #[tokio::test]
    async fn unknown_path_falls_back_to_index() {
        // SPA fallback: any unmatched path serves index.html with 200.
        let resp = app()
            .oneshot(
                Request::get("/some/client/route")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn protected_route_rejects_unauthenticated_requests() {
        let resp = app()
            .oneshot(Request::get("/api/todos").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
