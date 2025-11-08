use axum::{
    routing::{get, post, put, delete},
    Router,
};

use crate::db::DbPool;
use crate::handlers::{todos, sources, sync, health};

pub fn api_routes() -> Router<DbPool> {
    Router::new()
        // Health check
        .route("/health", get(health::health_check))

        // Todo routes
        .route("/todos", get(todos::list_todos))
        .route("/todos", post(todos::create_todo))
        .route("/todos/:id", get(todos::get_todo))
        .route("/todos/:id", put(todos::update_todo))
        .route("/todos/:id", delete(todos::delete_todo))

        // Source routes
        .route("/sources", get(sources::list_sources))
        .route("/sources/gmail", post(sources::create_gmail_source))
        .route("/sources/calendar", post(sources::create_calendar_source))
        .route("/sources/:id", get(sources::get_source))
        .route("/sources/:id", put(sources::update_source))
        .route("/sources/:id", delete(sources::delete_source))

        // Sync routes
        .route("/sync/trigger", post(sync::trigger_sync))
        .route("/sync/status", get(sync::get_sync_status))
}
