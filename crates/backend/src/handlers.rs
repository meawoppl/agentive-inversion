use axum::{
    extract::{Json, Path},
    http::StatusCode,
};
use shared_types::{CreateTodoRequest, Todo, UpdateTodoRequest};
use uuid::Uuid;

pub async fn list_todos() -> Result<Json<Vec<Todo>>, StatusCode> {
    Ok(Json(vec![]))
}

pub async fn create_todo(
    Json(_payload): Json<CreateTodoRequest>,
) -> Result<Json<Todo>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}

pub async fn update_todo(
    Path(_id): Path<Uuid>,
    Json(_payload): Json<UpdateTodoRequest>,
) -> Result<Json<Todo>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}

pub async fn delete_todo(Path(_id): Path<Uuid>) -> Result<StatusCode, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}
