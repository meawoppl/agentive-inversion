use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use serde::{Deserialize, Serialize};
use shared_types::{
    AgentDecisionResponse, ApproveDecisionRequest, Category, ConnectEmailAccountRequest,
    CreateAgentDecisionRequest, CreateCategoryRequest, CreateTodoRequest, DecisionStats,
    EmailAccountResponse, ProposedTodoAction, RejectDecisionRequest, Todo, UpdateCategoryRequest,
    UpdateTodoRequest,
};
use uuid::Uuid;

use crate::db::{categories, decisions, email_accounts, todos, DbPool};

// Todo handlers
pub async fn list_todos(State(pool): State<DbPool>) -> Result<Json<Vec<Todo>>, StatusCode> {
    let mut conn = pool.get().await.map_err(|e| {
        tracing::error!("Failed to get db connection: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let items = todos::list_all(&mut conn).await.map_err(|e| {
        tracing::error!("Failed to list todos: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(items))
}

pub async fn create_todo(
    State(pool): State<DbPool>,
    Json(payload): Json<CreateTodoRequest>,
) -> Result<Json<Todo>, StatusCode> {
    let mut conn = pool
        .get()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let todo = todos::create(
        &mut conn,
        &payload.title,
        payload.description.as_deref(),
        payload.due_date,
        payload.link.as_deref(),
        payload.category_id,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(todo))
}

pub async fn update_todo(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTodoRequest>,
) -> Result<Json<Todo>, StatusCode> {
    let mut conn = pool
        .get()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let todo = todos::update(
        &mut conn,
        id,
        payload.title.as_deref(),
        payload.description.as_deref(),
        payload.completed,
        payload.due_date,
        payload.link.as_deref(),
        payload.category_id,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(todo))
}

pub async fn delete_todo(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let mut conn = pool
        .get()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    todos::delete(&mut conn, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

// Email account handlers
pub async fn list_email_accounts(
    State(pool): State<DbPool>,
) -> Result<Json<Vec<EmailAccountResponse>>, StatusCode> {
    let mut conn = pool
        .get()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let accounts = email_accounts::list_all(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let responses: Vec<EmailAccountResponse> = accounts.into_iter().map(Into::into).collect();

    Ok(Json(responses))
}

pub async fn delete_email_account(
    State(pool): State<DbPool>,
    Path(account_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let mut conn = pool
        .get()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    email_accounts::delete(&mut conn, account_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize)]
pub struct OAuthStartResponse {
    pub auth_url: String,
    pub account_id: Uuid,
}

// OAuth flow - Step 1: Start OAuth flow
pub async fn start_gmail_oauth(
    State(pool): State<DbPool>,
    Json(payload): Json<ConnectEmailAccountRequest>,
) -> Result<Json<OAuthStartResponse>, StatusCode> {
    let client_id =
        std::env::var("GMAIL_CLIENT_ID").map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create a placeholder email account to track this connection
    let mut conn = pool
        .get()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let account = email_accounts::create(
        &mut conn,
        &payload.account_name,
        "pending@oauth.flow", // Temporary email until OAuth completes
        "gmail",
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Build OAuth URL
    let redirect_uri = std::env::var("OAUTH_REDIRECT_URI")
        .unwrap_or_else(|_| "http://localhost:3000/api/email-accounts/oauth/callback".to_string());

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?\
         client_id={}&\
         redirect_uri={}&\
         response_type=code&\
         scope=https://www.googleapis.com/auth/gmail.modify&\
         access_type=offline&\
         state={}",
        urlencoding::encode(&client_id),
        urlencoding::encode(&redirect_uri),
        account.id
    );

    Ok(Json(OAuthStartResponse {
        auth_url,
        account_id: account.id,
    }))
}

#[derive(Debug, Deserialize)]
pub struct OAuthCallbackParams {
    pub code: String,
    pub state: String,
}

// OAuth flow - Step 2: Handle OAuth callback
pub async fn gmail_oauth_callback(
    State(pool): State<DbPool>,
    Query(params): Query<OAuthCallbackParams>,
) -> impl IntoResponse {
    let account_id = match Uuid::parse_str(&params.state) {
        Ok(account_uuid) => account_uuid,
        Err(_) => return Redirect::to("/oauth/error?msg=invalid_state").into_response(),
    };

    let client_id = match std::env::var("GMAIL_CLIENT_ID") {
        Ok(client_id_str) => client_id_str,
        Err(_) => return Redirect::to("/oauth/error?msg=missing_config").into_response(),
    };

    let client_secret = match std::env::var("GMAIL_CLIENT_SECRET") {
        Ok(secret) => secret,
        Err(_) => return Redirect::to("/oauth/error?msg=missing_config").into_response(),
    };

    let redirect_uri = std::env::var("OAUTH_REDIRECT_URI")
        .unwrap_or_else(|_| "http://localhost:3000/api/email-accounts/oauth/callback".to_string());

    // Exchange code for tokens using reqwest
    #[derive(Serialize)]
    struct TokenRequest {
        code: String,
        client_id: String,
        client_secret: String,
        redirect_uri: String,
        grant_type: String,
    }

    #[derive(Deserialize, Debug)]
    struct TokenResponse {
        access_token: String,
        refresh_token: Option<String>,
        expires_in: i64,
    }

    let client = reqwest::Client::new();
    let token_response = match client
        .post("https://oauth2.googleapis.com/token")
        .form(&TokenRequest {
            code: params.code.clone(),
            client_id: client_id.clone(),
            client_secret: client_secret.clone(),
            redirect_uri: redirect_uri.clone(),
            grant_type: "authorization_code".to_string(),
        })
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(_) => return Redirect::to("/oauth/error?msg=token_exchange_failed").into_response(),
    };

    let tokens: TokenResponse = match token_response.json().await {
        Ok(t) => t,
        Err(_) => return Redirect::to("/oauth/error?msg=invalid_token_response").into_response(),
    };

    let refresh_token = match tokens.refresh_token {
        Some(rt) => rt,
        None => return Redirect::to("/oauth/error?msg=no_refresh_token").into_response(),
    };

    // Get user's email address using the access token
    #[derive(Deserialize)]
    struct UserInfo {
        email: String,
    }

    let user_info: UserInfo = match client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(&tokens.access_token)
        .send()
        .await
    {
        Ok(resp) => match resp.json().await {
            Ok(info) => info,
            Err(_) => return Redirect::to("/oauth/error?msg=failed_to_get_email").into_response(),
        },
        Err(_) => return Redirect::to("/oauth/error?msg=failed_to_get_email").into_response(),
    };

    // Update account with OAuth tokens and actual email
    let mut conn = match pool.get().await {
        Ok(c) => c,
        Err(_) => return Redirect::to("/oauth/error?msg=db_error").into_response(),
    };

    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(tokens.expires_in);

    // Update OAuth tokens using the db module function
    match crate::db::email_accounts::update_oauth_tokens(
        &mut conn,
        account_id,
        &refresh_token,
        &tokens.access_token,
        expires_at,
    )
    .await
    {
        Ok(_) => {}
        Err(_) => return Redirect::to("/oauth/error?msg=db_update_failed").into_response(),
    };

    // Also update the email address
    use crate::schema::email_accounts::dsl;
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;

    match diesel::update(dsl::email_accounts.filter(dsl::id.eq(account_id)))
        .set(dsl::email_address.eq(&user_info.email))
        .execute(&mut conn)
        .await
    {
        Ok(_) => {}
        Err(_) => return Redirect::to("/oauth/error?msg=email_update_failed").into_response(),
    };

    Redirect::to("/oauth/success").into_response()
}

// Category handlers
pub async fn list_categories(
    State(pool): State<DbPool>,
) -> Result<Json<Vec<Category>>, StatusCode> {
    let mut conn = pool.get().await.map_err(|e| {
        tracing::error!("Failed to get db connection: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let items = categories::list_all(&mut conn).await.map_err(|e| {
        tracing::error!("Failed to list categories: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(items))
}

pub async fn create_category(
    State(pool): State<DbPool>,
    Json(payload): Json<CreateCategoryRequest>,
) -> Result<Json<Category>, StatusCode> {
    let mut conn = pool
        .get()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let category = categories::create(&mut conn, &payload.name, payload.color.as_deref())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(category))
}

pub async fn update_category(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateCategoryRequest>,
) -> Result<Json<Category>, StatusCode> {
    let mut conn = pool
        .get()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let category = categories::update(
        &mut conn,
        id,
        payload.name.as_deref(),
        payload.color.as_deref(),
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(category))
}

pub async fn delete_category(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let mut conn = pool
        .get()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    categories::delete(&mut conn, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Agent Decision handlers
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct DecisionListParams {
    pub status: Option<String>,
    pub source_type: Option<String>,
}

pub async fn list_decisions(
    State(pool): State<DbPool>,
    Query(params): Query<DecisionListParams>,
) -> Result<Json<Vec<AgentDecisionResponse>>, StatusCode> {
    let mut conn = pool.get().await.map_err(|e| {
        tracing::error!("Failed to get db connection: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let items = if let Some(status) = params.status {
        decisions::list_by_status(&mut conn, &status).await
    } else if let Some(source_type) = params.source_type {
        decisions::list_by_source(&mut conn, &source_type).await
    } else {
        decisions::list_all(&mut conn).await
    }
    .map_err(|e| {
        tracing::error!("Failed to list decisions: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let responses: Vec<AgentDecisionResponse> = items.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}

pub async fn list_pending_decisions(
    State(pool): State<DbPool>,
) -> Result<Json<Vec<AgentDecisionResponse>>, StatusCode> {
    let mut conn = pool.get().await.map_err(|e| {
        tracing::error!("Failed to get db connection: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let items = decisions::list_pending(&mut conn).await.map_err(|e| {
        tracing::error!("Failed to list pending decisions: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let responses: Vec<AgentDecisionResponse> = items.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}

pub async fn get_decision(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<AgentDecisionResponse>, StatusCode> {
    let mut conn = pool
        .get()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let decision = decisions::get_by_id(&mut conn, id).await.map_err(|e| {
        tracing::error!("Failed to get decision: {:?}", e);
        StatusCode::NOT_FOUND
    })?;

    Ok(Json(decision.into()))
}

pub async fn create_decision(
    State(pool): State<DbPool>,
    Json(payload): Json<CreateAgentDecisionRequest>,
) -> Result<Json<AgentDecisionResponse>, StatusCode> {
    let mut conn = pool
        .get()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let decision = decisions::create(
        &mut conn,
        &payload.source_type,
        payload.source_id,
        payload.source_external_id.as_deref(),
        &payload.decision_type,
        payload.proposed_action,
        &payload.reasoning,
        payload.reasoning_details,
        payload.confidence,
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to create decision: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(decision.into()))
}

pub async fn approve_decision(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ApproveDecisionRequest>,
) -> Result<Json<AgentDecisionResponse>, StatusCode> {
    let mut conn = pool
        .get()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get the decision first
    let decision = decisions::get_by_id(&mut conn, id).await.map_err(|e| {
        tracing::error!("Failed to get decision: {:?}", e);
        StatusCode::NOT_FOUND
    })?;

    // If decision type is create_todo, create the todo
    let todo_id = if decision.decision_type == "create_todo" {
        // Use modifications if provided, otherwise use proposed_action
        let action: ProposedTodoAction = if let Some(mods) = payload.modifications {
            mods
        } else {
            serde_json::from_str(&decision.proposed_action).map_err(|e| {
                tracing::error!("Failed to parse proposed_action: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
        };

        let todo = todos::create(
            &mut conn,
            &action.todo_title,
            action.todo_description.as_deref(),
            action.due_date,
            None, // link
            action.category_id,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to create todo from decision: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        Some(todo.id)
    } else {
        None
    };

    // Update the decision status
    let updated = decisions::approve(&mut conn, id, todo_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to approve decision: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Mark as executed since we've already created the todo
    let final_decision = if todo_id.is_some() {
        decisions::mark_executed(&mut conn, id).await.map_err(|e| {
            tracing::error!("Failed to mark decision as executed: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    } else {
        updated
    };

    Ok(Json(final_decision.into()))
}

pub async fn reject_decision(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<RejectDecisionRequest>,
) -> Result<Json<AgentDecisionResponse>, StatusCode> {
    let mut conn = pool
        .get()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let decision = decisions::reject(&mut conn, id, payload.feedback.as_deref())
        .await
        .map_err(|e| {
            tracing::error!("Failed to reject decision: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(decision.into()))
}

pub async fn get_decision_stats(
    State(pool): State<DbPool>,
) -> Result<Json<DecisionStats>, StatusCode> {
    let mut conn = pool.get().await.map_err(|e| {
        tracing::error!("Failed to get db connection: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let stats = decisions::get_stats(&mut conn).await.map_err(|e| {
        tracing::error!("Failed to get decision stats: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(stats))
}
