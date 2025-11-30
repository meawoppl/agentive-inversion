use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use serde::{Deserialize, Serialize};
use shared_types::{
    Category, ConnectEmailAccountRequest, CreateCategoryRequest, CreateTodoRequest,
    EmailAccountResponse, Todo, UpdateCategoryRequest, UpdateTodoRequest,
};
use uuid::Uuid;

use crate::db::{email_accounts, DbPool};

// Todo handlers
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
         scope=https://www.googleapis.com/auth/gmail.readonly&\
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
pub async fn list_categories() -> Result<Json<Vec<Category>>, StatusCode> {
    Ok(Json(vec![]))
}

pub async fn create_category(
    Json(_payload): Json<CreateCategoryRequest>,
) -> Result<Json<Category>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}

pub async fn update_category(
    Path(_id): Path<Uuid>,
    Json(_payload): Json<UpdateCategoryRequest>,
) -> Result<Json<Category>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}

pub async fn delete_category(Path(_id): Path<Uuid>) -> Result<StatusCode, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}
