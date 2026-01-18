use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use serde::{Deserialize, Serialize};
use shared_types::{
    AgentDecisionResponse, AgentRuleResponse, ApproveDecisionRequest, BatchApproveDecisionsRequest,
    BatchOperationFailure, BatchOperationResponse, BatchRejectDecisionsRequest, CalendarEventQuery,
    CalendarEventResponse, Category, ChatHistoryQuery, ChatIntent, ChatMessageResponse,
    ChatResponse, ConnectEmailAccountRequest, CreateAgentDecisionRequest, CreateAgentRuleRequest,
    CreateCategoryRequest, CreateTodoRequest, DecisionStats, EmailAccountResponse, EmailListQuery,
    EmailResponse, RejectDecisionRequest, RuleListQuery, SendChatMessageRequest, SuggestedAction,
    Todo, UpdateAgentRuleRequest, UpdateCategoryRequest, UpdateTodoRequest,
};
use uuid::Uuid;

use crate::db::{
    agent_rules, calendar_accounts, calendar_events, categories, chat_messages, decisions,
    email_accounts, emails, get_conn, todos, DbPool,
};
use crate::error::{ApiError, ApiResult};
use crate::services::DecisionService;

// Todo handlers
pub async fn list_todos(State(pool): State<DbPool>) -> ApiResult<Json<Vec<Todo>>> {
    let mut conn = get_conn(&pool).await?;
    let items = todos::list_all(&mut conn).await?;
    Ok(Json(items))
}

pub async fn create_todo(
    State(pool): State<DbPool>,
    Json(payload): Json<CreateTodoRequest>,
) -> ApiResult<Json<Todo>> {
    let mut conn = get_conn(&pool).await?;
    let todo = todos::create(
        &mut conn,
        &payload.title,
        payload.description.as_deref(),
        payload.due_date,
        payload.link.as_deref(),
        payload.category_id,
    )
    .await?;
    Ok(Json(todo))
}

pub async fn update_todo(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTodoRequest>,
) -> ApiResult<Json<Todo>> {
    let mut conn = get_conn(&pool).await?;
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
    .await?;
    Ok(Json(todo))
}

pub async fn delete_todo(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let mut conn = get_conn(&pool).await?;
    todos::delete(&mut conn, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// Email account handlers
pub async fn list_email_accounts(
    State(pool): State<DbPool>,
) -> ApiResult<Json<Vec<EmailAccountResponse>>> {
    let mut conn = pool.get().await?;
    let accounts = email_accounts::list_all(&mut conn).await?;
    let responses: Vec<EmailAccountResponse> = accounts.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}

pub async fn delete_email_account(
    State(pool): State<DbPool>,
    Path(account_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let mut conn = pool.get().await?;
    email_accounts::delete(&mut conn, account_id).await?;
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
) -> ApiResult<Json<OAuthStartResponse>> {
    let client_id =
        std::env::var("GOOGLE_CLIENT_ID").map_err(|_| ApiError::missing_env("GOOGLE_CLIENT_ID"))?;

    let mut conn = pool.get().await?;

    let account = email_accounts::create(
        &mut conn,
        &payload.account_name,
        "pending@oauth.flow", // Temporary email until OAuth completes
        "gmail",
    )
    .await?;

    let redirect_uri = std::env::var("OAUTH_REDIRECT_URI")
        .map_err(|_| ApiError::missing_env("OAUTH_REDIRECT_URI"))?;

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

    let client_id = match std::env::var("GOOGLE_CLIENT_ID") {
        Ok(client_id_str) => client_id_str,
        Err(_) => return Redirect::to("/oauth/error?msg=missing_config").into_response(),
    };

    let client_secret = match std::env::var("GOOGLE_CLIENT_SECRET") {
        Ok(secret) => secret,
        Err(_) => return Redirect::to("/oauth/error?msg=missing_config").into_response(),
    };

    let redirect_uri = match std::env::var("OAUTH_REDIRECT_URI") {
        Ok(uri) => uri,
        Err(_) => return Redirect::to("/oauth/error?msg=missing_redirect_uri").into_response(),
    };

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
pub async fn list_categories(State(pool): State<DbPool>) -> ApiResult<Json<Vec<Category>>> {
    let mut conn = get_conn(&pool).await?;
    let items = categories::list_all(&mut conn).await?;
    Ok(Json(items))
}

pub async fn create_category(
    State(pool): State<DbPool>,
    Json(payload): Json<CreateCategoryRequest>,
) -> ApiResult<Json<Category>> {
    let mut conn = get_conn(&pool).await?;
    let category = categories::create(&mut conn, &payload.name, payload.color.as_deref()).await?;
    Ok(Json(category))
}

pub async fn update_category(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateCategoryRequest>,
) -> ApiResult<Json<Category>> {
    let mut conn = get_conn(&pool).await?;
    let category = categories::update(
        &mut conn,
        id,
        payload.name.as_deref(),
        payload.color.as_deref(),
    )
    .await?;
    Ok(Json(category))
}

pub async fn delete_category(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let mut conn = get_conn(&pool).await?;
    categories::delete(&mut conn, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// Email handlers
pub async fn list_emails(
    State(pool): State<DbPool>,
    Query(query): Query<EmailListQuery>,
) -> ApiResult<Json<Vec<EmailResponse>>> {
    let mut conn = pool.get().await?;
    let limit = query.limit.or(Some(50));
    let offset = query.offset;

    let items = if let Some(acc_id) = query.account_id {
        emails::list_by_account(&mut conn, acc_id, limit).await?
    } else {
        emails::list_all(&mut conn, limit, offset).await?
    };

    let responses: Vec<EmailResponse> = items
        .into_iter()
        .map(|e| EmailResponse {
            id: e.id,
            account_id: e.account_id,
            gmail_id: e.gmail_id,
            thread_id: e.thread_id,
            subject: e.subject,
            from_address: e.from_address,
            from_name: e.from_name,
            to_addresses: e.to_addresses.into_iter().flatten().collect(),
            snippet: e.snippet,
            has_attachments: e.has_attachments,
            received_at: e.received_at,
            processed: e.processed,
            archived_in_gmail: e.archived_in_gmail,
        })
        .collect();

    Ok(Json(responses))
}

pub async fn get_email(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<EmailResponse>> {
    let mut conn = pool.get().await?;
    let email = emails::get_by_id(&mut conn, id).await?;

    Ok(Json(EmailResponse {
        id: email.id,
        account_id: email.account_id,
        gmail_id: email.gmail_id,
        thread_id: email.thread_id,
        subject: email.subject,
        from_address: email.from_address,
        from_name: email.from_name,
        to_addresses: email.to_addresses.into_iter().flatten().collect(),
        snippet: email.snippet,
        has_attachments: email.has_attachments,
        received_at: email.received_at,
        processed: email.processed,
        archived_in_gmail: email.archived_in_gmail,
    }))
}

#[derive(Debug, Serialize)]
pub struct EmailStatsResponse {
    pub total: i64,
    pub unprocessed: i64,
}

pub async fn get_email_stats(State(pool): State<DbPool>) -> ApiResult<Json<EmailStatsResponse>> {
    let mut conn = pool.get().await?;
    let total = emails::count_all(&mut conn).await?;
    let unprocessed = emails::count_unprocessed(&mut conn).await?;
    Ok(Json(EmailStatsResponse { total, unprocessed }))
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
) -> ApiResult<Json<Vec<AgentDecisionResponse>>> {
    let mut conn = pool.get().await?;

    let items = if let Some(status) = params.status {
        decisions::list_by_status(&mut conn, &status).await?
    } else if let Some(source_type) = params.source_type {
        decisions::list_by_source(&mut conn, &source_type).await?
    } else {
        decisions::list_all(&mut conn).await?
    };

    let responses: Vec<AgentDecisionResponse> = items.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}

pub async fn list_pending_decisions(
    State(pool): State<DbPool>,
) -> ApiResult<Json<Vec<AgentDecisionResponse>>> {
    let mut conn = pool.get().await?;
    let items = decisions::list_pending(&mut conn).await?;
    let responses: Vec<AgentDecisionResponse> = items.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}

pub async fn get_decision(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<AgentDecisionResponse>> {
    let mut conn = pool.get().await?;
    let decision = decisions::get_by_id(&mut conn, id).await?;
    Ok(Json(decision.into()))
}

pub async fn create_decision(
    State(pool): State<DbPool>,
    Json(payload): Json<CreateAgentDecisionRequest>,
) -> ApiResult<Json<AgentDecisionResponse>> {
    let mut conn = pool.get().await?;
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
    .await?;
    Ok(Json(decision.into()))
}

pub async fn approve_decision(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ApproveDecisionRequest>,
) -> ApiResult<Json<AgentDecisionResponse>> {
    let mut conn = pool.get().await?;
    let result = DecisionService::approve(&mut conn, id, payload.modifications).await?;
    Ok(Json(result.decision.into()))
}

pub async fn reject_decision(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<RejectDecisionRequest>,
) -> ApiResult<Json<AgentDecisionResponse>> {
    let mut conn = pool.get().await?;
    let decision = DecisionService::reject(&mut conn, id, payload.feedback.as_deref()).await?;
    Ok(Json(decision.into()))
}

pub async fn get_decision_stats(State(pool): State<DbPool>) -> ApiResult<Json<DecisionStats>> {
    let mut conn = pool.get().await?;
    let stats = decisions::get_stats(&mut conn).await?;
    Ok(Json(stats))
}

pub async fn batch_approve_decisions(
    State(pool): State<DbPool>,
    Json(payload): Json<BatchApproveDecisionsRequest>,
) -> ApiResult<Json<BatchOperationResponse>> {
    let result = DecisionService::batch_approve(&pool, payload.decision_ids).await?;
    let failed = result
        .failed
        .into_iter()
        .map(|f| BatchOperationFailure {
            id: f.id,
            error: f.error,
        })
        .collect();
    Ok(Json(BatchOperationResponse {
        successful: result.successful,
        failed,
    }))
}

pub async fn batch_reject_decisions(
    State(pool): State<DbPool>,
    Json(payload): Json<BatchRejectDecisionsRequest>,
) -> ApiResult<Json<BatchOperationResponse>> {
    let result =
        DecisionService::batch_reject(&pool, payload.decision_ids, payload.feedback.as_deref())
            .await?;
    let failed = result
        .failed
        .into_iter()
        .map(|f| BatchOperationFailure {
            id: f.id,
            error: f.error,
        })
        .collect();
    Ok(Json(BatchOperationResponse {
        successful: result.successful,
        failed,
    }))
}

// Agent rules handlers
pub async fn list_agent_rules(
    State(pool): State<DbPool>,
    Query(query): Query<RuleListQuery>,
) -> ApiResult<Json<Vec<AgentRuleResponse>>> {
    let mut conn = pool.get().await?;

    let rules = if let Some(source) = &query.source_type {
        agent_rules::list_by_source_type(&mut conn, source).await?
    } else if query.is_active == Some(true) {
        agent_rules::list_active(&mut conn).await?
    } else {
        agent_rules::list_all(&mut conn).await?
    };

    let responses: Vec<AgentRuleResponse> = rules
        .into_iter()
        .filter_map(|r| r.try_into().ok())
        .collect();

    Ok(Json(responses))
}

pub async fn get_agent_rule(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<AgentRuleResponse>> {
    let mut conn = pool.get().await?;
    let rule = agent_rules::get_by_id(&mut conn, id).await?;
    let response: AgentRuleResponse = rule.try_into()?;
    Ok(Json(response))
}

pub async fn create_agent_rule(
    State(pool): State<DbPool>,
    Json(payload): Json<CreateAgentRuleRequest>,
) -> ApiResult<Json<AgentRuleResponse>> {
    let mut conn = pool.get().await?;

    let rule = agent_rules::create(&mut conn, &payload).await?;
    let response: AgentRuleResponse = rule.try_into()?;
    Ok(Json(response))
}

pub async fn update_agent_rule(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAgentRuleRequest>,
) -> ApiResult<Json<AgentRuleResponse>> {
    let mut conn = pool.get().await?;
    let rule = agent_rules::update(
        &mut conn,
        id,
        payload.name.as_deref(),
        payload.description.as_deref(),
        payload.source_type.as_deref(),
        payload.rule_type.as_deref(),
        payload.conditions.as_ref(),
        payload.action.as_deref(),
        payload.action_params.as_ref(),
        payload.priority,
        payload.is_active,
    )
    .await?;
    let response: AgentRuleResponse = rule.try_into()?;
    Ok(Json(response))
}

pub async fn delete_agent_rule(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let mut conn = pool.get().await?;
    agent_rules::delete(&mut conn, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize)]
pub struct ToggleActiveResponse {
    pub id: Uuid,
    pub is_active: bool,
}

pub async fn toggle_agent_rule_active(
    State(pool): State<DbPool>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ToggleActiveResponse>> {
    let mut conn = pool.get().await?;
    let current = agent_rules::get_by_id(&mut conn, id).await?;

    let updated = agent_rules::set_active(&mut conn, id, !current.is_active).await?;
    Ok(Json(ToggleActiveResponse {
        id: updated.id,
        is_active: updated.is_active,
    }))
}

// ============================================================================
// Chat handlers
// ============================================================================

pub async fn get_chat_history(
    State(pool): State<DbPool>,
    Query(query): Query<ChatHistoryQuery>,
) -> ApiResult<Json<Vec<ChatMessageResponse>>> {
    let mut conn = pool.get().await?;
    let messages = chat_messages::list_history(&mut conn, query.limit, query.before).await?;
    let responses: Vec<ChatMessageResponse> = messages.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}

pub async fn send_chat_message(
    State(pool): State<DbPool>,
    Json(payload): Json<SendChatMessageRequest>,
) -> ApiResult<Json<ChatResponse>> {
    let mut conn = pool.get().await?;

    // Detect intent from the message content
    let (detected_intent, suggested_actions) = classify_intent(&payload.content, &mut conn).await;

    // Save the user's message
    chat_messages::create(&mut conn, "user", &payload.content, None).await?;

    // Generate assistant response based on intent
    let assistant_response = generate_response(&detected_intent, &payload.content, &mut conn).await;

    // Save the assistant's response
    let assistant_message = chat_messages::create(
        &mut conn,
        "assistant",
        &assistant_response,
        Some(detected_intent.as_str()),
    )
    .await?;

    Ok(Json(ChatResponse {
        message: assistant_message.into(),
        detected_intent: Some(detected_intent.as_str().to_string()),
        suggested_actions,
    }))
}

pub async fn clear_chat_history(State(pool): State<DbPool>) -> ApiResult<StatusCode> {
    let mut conn = pool.get().await?;

    chat_messages::delete_all(&mut conn).await?;
    Ok(StatusCode::NO_CONTENT)
}

// Intent classification - simple keyword-based for now
async fn classify_intent(
    content: &str,
    conn: &mut diesel_async::AsyncPgConnection,
) -> (ChatIntent, Vec<SuggestedAction>) {
    let lower = content.to_lowercase();
    let mut actions = Vec::new();

    // Check for todo creation intent
    if lower.contains("add")
        || lower.contains("create")
        || lower.contains("new task")
        || lower.contains("remind me")
        || lower.contains("todo")
    {
        // Extract potential todo title from the message
        let title = extract_todo_title(content);
        if !title.is_empty() {
            actions.push(SuggestedAction {
                label: format!("Create: {}", truncate_str(&title, 30)),
                action_type: "create_todo".to_string(),
                payload: serde_json::json!({ "title": title }),
            });
        }
        return (ChatIntent::CreateTodo, actions);
    }

    // Check for todo query intent
    if lower.contains("show")
        || lower.contains("list")
        || lower.contains("what")
        || lower.contains("my tasks")
        || lower.contains("my todos")
    {
        actions.push(SuggestedAction {
            label: "View all todos".to_string(),
            action_type: "navigate".to_string(),
            payload: serde_json::json!({ "view": "todos" }),
        });
        return (ChatIntent::QueryTodos, actions);
    }

    // Check for completion intent
    if lower.contains("done") || lower.contains("complete") || lower.contains("finish") {
        return (ChatIntent::MarkComplete, actions);
    }

    // Check for decision-related intents
    if lower.contains("decision") || lower.contains("pending") || lower.contains("review") {
        // Check if there are pending decisions
        if let Ok(pending) = decisions::list_pending(conn).await {
            if !pending.is_empty() {
                actions.push(SuggestedAction {
                    label: format!("Review {} pending", pending.len()),
                    action_type: "navigate".to_string(),
                    payload: serde_json::json!({ "view": "decisions" }),
                });
            }
        }
        return (ChatIntent::QueryDecisions, actions);
    }

    // Check for approval intent
    if lower.contains("approve") || lower.contains("accept") {
        return (ChatIntent::ApproveDecision, actions);
    }

    // Check for help intent
    if lower.contains("help") || lower.contains("what can you") || lower.contains("how do") {
        return (ChatIntent::Help, actions);
    }

    (ChatIntent::General, actions)
}

async fn generate_response(
    intent: &ChatIntent,
    _content: &str,
    conn: &mut diesel_async::AsyncPgConnection,
) -> String {
    match intent {
        ChatIntent::CreateTodo => {
            "I can help you create a todo. Use the suggested action above, or tell me more details about the task you want to add.".to_string()
        }
        ChatIntent::QueryTodos => {
            // Get todo stats
            match todos::list_all(conn).await {
                Ok(all_todos) => {
                    let total = all_todos.len();
                    let completed = all_todos.iter().filter(|t| t.completed).count();
                    let pending = total - completed;
                    format!(
                        "You have {} todos: {} pending and {} completed. Click 'View all todos' to see them.",
                        total, pending, completed
                    )
                }
                Err(_) => "I couldn't retrieve your todos. Please try again.".to_string(),
            }
        }
        ChatIntent::MarkComplete => {
            "To mark a task as complete, navigate to your todos and click the checkbox next to the task.".to_string()
        }
        ChatIntent::QueryDecisions => {
            match decisions::list_pending(conn).await {
                Ok(pending) => {
                    if pending.is_empty() {
                        "You have no pending decisions to review. Great job staying on top of things!".to_string()
                    } else {
                        format!(
                            "You have {} pending decisions awaiting review. Click 'Review pending' to see them.",
                            pending.len()
                        )
                    }
                }
                Err(_) => "I couldn't retrieve your decisions. Please try again.".to_string(),
            }
        }
        ChatIntent::ApproveDecision => {
            "To approve decisions, go to the decision inbox and click 'Approve' on individual decisions, or use batch approve for multiple at once.".to_string()
        }
        ChatIntent::RejectDecision => {
            "To reject decisions, go to the decision inbox and click 'Reject' on individual decisions. You can optionally provide feedback.".to_string()
        }
        ChatIntent::ModifyTodo => {
            "To modify a todo, navigate to your todos list and click on the todo you want to edit.".to_string()
        }
        ChatIntent::QueryEmails => {
            "Email viewing is available through the decision inbox when emails trigger agent decisions.".to_string()
        }
        ChatIntent::Help => {
            "I can help you with:\n\
            - Creating todos: \"Add a task to call John\"\n\
            - Viewing todos: \"Show my tasks\"\n\
            - Reviewing decisions: \"Show pending decisions\"\n\
            - Quick questions about your task management\n\n\
            Just type naturally and I'll try to help!".to_string()
        }
        ChatIntent::General => {
            "I'm here to help you manage your tasks and review agent decisions. Try asking me to create a todo, show your tasks, or review pending decisions.".to_string()
        }
    }
}

fn extract_todo_title(content: &str) -> String {
    // Simple extraction: remove common prefixes
    let lower = content.to_lowercase();
    let prefixes = [
        "add ",
        "create ",
        "new task ",
        "remind me to ",
        "todo ",
        "add a ",
        "create a ",
        "i need to ",
        "don't forget to ",
    ];

    for prefix in prefixes {
        if lower.starts_with(prefix) {
            return content[prefix.len()..].trim().to_string();
        }
    }

    // If no prefix matched, return the original content trimmed
    content.trim().to_string()
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

// Calendar event handlers
pub async fn list_calendar_events(
    State(pool): State<DbPool>,
    Query(params): Query<CalendarEventQuery>,
) -> ApiResult<Json<Vec<CalendarEventResponse>>> {
    let mut conn = pool.get().await?;
    let events = calendar_events::list_events(
        &mut conn,
        params.account_id,
        params.since,
        params.until,
        params.processed,
        params.limit,
    )
    .await?;
    let responses: Vec<CalendarEventResponse> = events.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}

pub async fn get_calendar_event(
    State(pool): State<DbPool>,
    Path(event_id): Path<Uuid>,
) -> ApiResult<Json<CalendarEventResponse>> {
    let mut conn = pool.get().await?;
    let event = calendar_events::get_by_id(&mut conn, event_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Calendar event"))?;
    Ok(Json(event.into()))
}

pub async fn get_todays_events(
    State(pool): State<DbPool>,
) -> ApiResult<Json<Vec<CalendarEventResponse>>> {
    let mut conn = pool.get().await?;
    let events = calendar_events::get_today(&mut conn).await?;
    let responses: Vec<CalendarEventResponse> = events.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}

pub async fn get_this_weeks_events(
    State(pool): State<DbPool>,
) -> ApiResult<Json<Vec<CalendarEventResponse>>> {
    let mut conn = pool.get().await?;
    let events = calendar_events::get_this_week(&mut conn).await?;
    let responses: Vec<CalendarEventResponse> = events.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}

// Calendar account handlers
#[derive(Debug, Deserialize)]
pub struct CreateCalendarAccountRequest {
    pub account_name: String,
    pub calendar_id: String,
    pub email_address: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CalendarAccountResponse {
    pub id: Uuid,
    pub account_name: String,
    pub calendar_id: String,
    pub email_address: Option<String>,
    pub sync_status: String,
    pub last_synced: Option<chrono::DateTime<chrono::Utc>>,
    pub is_active: bool,
}

impl From<shared_types::CalendarAccount> for CalendarAccountResponse {
    fn from(account: shared_types::CalendarAccount) -> Self {
        CalendarAccountResponse {
            id: account.id,
            account_name: account.account_name,
            calendar_id: account.calendar_id,
            email_address: account.email_address,
            sync_status: account.sync_status,
            last_synced: account.last_synced,
            is_active: account.is_active,
        }
    }
}

pub async fn list_calendar_accounts(
    State(pool): State<DbPool>,
) -> ApiResult<Json<Vec<CalendarAccountResponse>>> {
    let mut conn = pool.get().await?;
    let accounts = calendar_accounts::list(&mut conn).await?;
    let responses: Vec<CalendarAccountResponse> = accounts.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}

pub async fn create_calendar_account(
    State(pool): State<DbPool>,
    Json(payload): Json<CreateCalendarAccountRequest>,
) -> ApiResult<Json<CalendarAccountResponse>> {
    let mut conn = pool.get().await?;
    let account = calendar_accounts::create(
        &mut conn,
        &payload.account_name,
        &payload.calendar_id,
        payload.email_address.as_deref(),
    )
    .await?;
    Ok(Json(account.into()))
}

pub async fn delete_calendar_account(
    State(pool): State<DbPool>,
    Path(account_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let mut conn = pool.get().await?;
    calendar_accounts::delete(&mut conn, account_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn toggle_calendar_account(
    State(pool): State<DbPool>,
    Path(account_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let mut conn = pool.get().await?;

    // Get current state
    let account = calendar_accounts::get_by_id(&mut conn, account_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Calendar account"))?;

    // Toggle active state
    calendar_accounts::set_active(&mut conn, account_id, !account.is_active).await?;

    Ok(StatusCode::OK)
}
