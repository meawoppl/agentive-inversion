use axum::{
    extract::{Path, State},
    Json,
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::ApiResult;
use shared::api::{
    CreateGmailSourceRequest, CreateCalendarSourceRequest, UpdateSourceRequest,
    SourceResponse, ListSourcesResponse,
};

pub async fn list_sources(
    State(_pool): State<DbPool>,
) -> ApiResult<Json<ListSourcesResponse>> {
    // TODO: Implement source listing
    Ok(Json(ListSourcesResponse {
        sources: vec![],
        total: 0,
    }))
}

pub async fn create_gmail_source(
    State(_pool): State<DbPool>,
    Json(_payload): Json<CreateGmailSourceRequest>,
) -> ApiResult<Json<SourceResponse>> {
    // TODO: Implement Gmail source creation
    unimplemented!("Gmail source creation not yet implemented")
}

pub async fn create_calendar_source(
    State(_pool): State<DbPool>,
    Json(_payload): Json<CreateCalendarSourceRequest>,
) -> ApiResult<Json<SourceResponse>> {
    // TODO: Implement Calendar source creation
    unimplemented!("Calendar source creation not yet implemented")
}

pub async fn get_source(
    State(_pool): State<DbPool>,
    Path(_id): Path<Uuid>,
) -> ApiResult<Json<SourceResponse>> {
    // TODO: Implement source retrieval
    unimplemented!("Source retrieval not yet implemented")
}

pub async fn update_source(
    State(_pool): State<DbPool>,
    Path(_id): Path<Uuid>,
    Json(_payload): Json<UpdateSourceRequest>,
) -> ApiResult<Json<SourceResponse>> {
    // TODO: Implement source update
    unimplemented!("Source update not yet implemented")
}

pub async fn delete_source(
    State(_pool): State<DbPool>,
    Path(_id): Path<Uuid>,
) -> ApiResult<()> {
    // TODO: Implement source deletion
    unimplemented!("Source deletion not yet implemented")
}
