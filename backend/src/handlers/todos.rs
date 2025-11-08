use axum::{
    extract::{Path, Query, State},
    Json,
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::ApiResult;
use shared::api::{
    CreateTodoRequest, UpdateTodoRequest, TodoResponse, ListTodosResponse, ListTodosQuery,
};

pub async fn list_todos(
    State(_pool): State<DbPool>,
    Query(_query): Query<ListTodosQuery>,
) -> ApiResult<Json<ListTodosResponse>> {
    // TODO: Implement actual database query
    Ok(Json(ListTodosResponse {
        todos: vec![],
        total: 0,
        page: 1,
        per_page: 20,
    }))
}

pub async fn create_todo(
    State(_pool): State<DbPool>,
    Json(_payload): Json<CreateTodoRequest>,
) -> ApiResult<Json<TodoResponse>> {
    // TODO: Implement todo creation
    unimplemented!("Todo creation not yet implemented")
}

pub async fn get_todo(
    State(_pool): State<DbPool>,
    Path(_id): Path<Uuid>,
) -> ApiResult<Json<TodoResponse>> {
    // TODO: Implement todo retrieval
    unimplemented!("Todo retrieval not yet implemented")
}

pub async fn update_todo(
    State(_pool): State<DbPool>,
    Path(_id): Path<Uuid>,
    Json(_payload): Json<UpdateTodoRequest>,
) -> ApiResult<Json<TodoResponse>> {
    // TODO: Implement todo update
    unimplemented!("Todo update not yet implemented")
}

pub async fn delete_todo(
    State(_pool): State<DbPool>,
    Path(_id): Path<Uuid>,
) -> ApiResult<()> {
    // TODO: Implement todo deletion
    unimplemented!("Todo deletion not yet implemented")
}
