use gloo_net::http::Request;
use shared::api::{
    ListTodosResponse, ListTodosQuery, CreateTodoRequest, TodoResponse,
    ListSourcesResponse, SyncStatusResponse,
};

const API_BASE_URL: &str = "http://localhost:8080/api";

pub struct ApiService;

impl ApiService {
    pub async fn list_todos(query: Option<ListTodosQuery>) -> Result<ListTodosResponse, String> {
        let mut url = format!("{}/todos", API_BASE_URL);

        if let Some(q) = query {
            let mut params = Vec::new();
            if let Some(page) = q.page {
                params.push(format!("page={}", page));
            }
            if let Some(per_page) = q.per_page {
                params.push(format!("per_page={}", per_page));
            }
            if !params.is_empty() {
                url.push('?');
                url.push_str(&params.join("&"));
            }
        }

        let response = Request::get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {:?}", e))?;

        if !response.ok() {
            return Err(format!("HTTP error: {}", response.status()));
        }

        response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {:?}", e))
    }

    pub async fn create_todo(request: CreateTodoRequest) -> Result<TodoResponse, String> {
        let url = format!("{}/todos", API_BASE_URL);

        let response = Request::post(&url)
            .json(&request)
            .map_err(|e| format!("Failed to serialize request: {:?}", e))?
            .send()
            .await
            .map_err(|e| format!("Request failed: {:?}", e))?;

        if !response.ok() {
            return Err(format!("HTTP error: {}", response.status()));
        }

        response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {:?}", e))
    }

    pub async fn list_sources() -> Result<ListSourcesResponse, String> {
        let url = format!("{}/sources", API_BASE_URL);

        let response = Request::get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {:?}", e))?;

        if !response.ok() {
            return Err(format!("HTTP error: {}", response.status()));
        }

        response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {:?}", e))
    }

    pub async fn get_sync_status() -> Result<SyncStatusResponse, String> {
        let url = format!("{}/sync/status", API_BASE_URL);

        let response = Request::get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {:?}", e))?;

        if !response.ok() {
            return Err(format!("HTTP error: {}", response.status()));
        }

        response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {:?}", e))
    }
}
