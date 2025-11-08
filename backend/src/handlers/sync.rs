use axum::{extract::State, Json};

use crate::db::DbPool;
use crate::error::ApiResult;
use shared::api::{TriggerSyncRequest, TriggerSyncResponse, SyncStatusResponse};

pub async fn trigger_sync(
    State(_pool): State<DbPool>,
    Json(_payload): Json<TriggerSyncRequest>,
) -> ApiResult<Json<TriggerSyncResponse>> {
    // TODO: Implement sync triggering
    Ok(Json(TriggerSyncResponse {
        triggered: false,
        message: "Sync not yet implemented".to_string(),
        source_ids: vec![],
    }))
}

pub async fn get_sync_status(
    State(_pool): State<DbPool>,
) -> ApiResult<Json<SyncStatusResponse>> {
    // TODO: Implement sync status retrieval
    Ok(Json(SyncStatusResponse {
        sources: vec![],
        overall_status: "Not configured".to_string(),
    }))
}
