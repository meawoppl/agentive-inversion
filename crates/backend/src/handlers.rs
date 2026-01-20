use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use shared_types::{
    AgentDecisionResponse, AgentRuleResponse, ApproveDecisionRequest, BatchApproveDecisionsRequest,
    BatchOperationFailure, BatchOperationResponse, BatchRejectDecisionsRequest, CalendarEventQuery,
    CalendarEventResponse, Category, ChatHistoryQuery, ChatIntent, ChatMessageResponse,
    ChatResponse, CreateAgentDecisionRequest, CreateAgentRuleRequest, CreateCategoryRequest,
    CreateTodoRequest, DecisionStats, EmailListQuery, EmailResponse, GoogleAccountResponse,
    RejectDecisionRequest, RuleListQuery, SendChatMessageRequest, SuggestedAction, Todo,
    UpdateAgentRuleRequest, UpdateCategoryRequest, UpdateTodoRequest,
};
use uuid::Uuid;

// Authentication is handled by middleware layer in main.rs
use crate::db::{
    agent_rules, calendar_events, categories, chat_messages, decisions, emails, get_conn,
    google_accounts, todos,
};
use crate::error::{ApiError, ApiResult};
use crate::services::DecisionService;
use crate::AppState;

// Todo handlers
pub async fn list_todos(State(state): State<AppState>) -> ApiResult<Json<Vec<Todo>>> {
    let mut conn = get_conn(&state.pool).await?;
    let items = todos::list_all(&mut conn).await?;
    Ok(Json(items))
}

pub async fn create_todo(
    State(state): State<AppState>,
    Json(payload): Json<CreateTodoRequest>,
) -> ApiResult<Json<Todo>> {
    let mut conn = get_conn(&state.pool).await?;
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
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTodoRequest>,
) -> ApiResult<Json<Todo>> {
    let mut conn = get_conn(&state.pool).await?;
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
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let mut conn = get_conn(&state.pool).await?;
    todos::delete(&mut conn, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// Google account handlers
pub async fn list_google_accounts(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<GoogleAccountResponse>>> {
    let mut conn = state.pool.get().await?;
    let accounts = google_accounts::list_all(&mut conn).await?;
    let responses: Vec<GoogleAccountResponse> = accounts.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}

// Category handlers
pub async fn list_categories(State(state): State<AppState>) -> ApiResult<Json<Vec<Category>>> {
    let mut conn = get_conn(&state.pool).await?;
    let items = categories::list_all(&mut conn).await?;
    Ok(Json(items))
}

pub async fn create_category(
    State(state): State<AppState>,
    Json(payload): Json<CreateCategoryRequest>,
) -> ApiResult<Json<Category>> {
    let mut conn = get_conn(&state.pool).await?;
    let category = categories::create(&mut conn, &payload.name, payload.color.as_deref()).await?;
    Ok(Json(category))
}

pub async fn update_category(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateCategoryRequest>,
) -> ApiResult<Json<Category>> {
    let mut conn = get_conn(&state.pool).await?;
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
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let mut conn = get_conn(&state.pool).await?;
    categories::delete(&mut conn, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// Email handlers
pub async fn list_emails(
    State(state): State<AppState>,
    Query(query): Query<EmailListQuery>,
) -> ApiResult<Json<Vec<EmailResponse>>> {
    let mut conn = state.pool.get().await?;
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
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<EmailResponse>> {
    let mut conn = state.pool.get().await?;
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

pub async fn get_email_stats(State(state): State<AppState>) -> ApiResult<Json<EmailStatsResponse>> {
    let mut conn = state.pool.get().await?;
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
    State(state): State<AppState>,
    Query(params): Query<DecisionListParams>,
) -> ApiResult<Json<Vec<AgentDecisionResponse>>> {
    let mut conn = state.pool.get().await?;

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
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<AgentDecisionResponse>>> {
    let mut conn = state.pool.get().await?;
    let items = decisions::list_pending(&mut conn).await?;
    let responses: Vec<AgentDecisionResponse> = items.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}

pub async fn get_decision(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<AgentDecisionResponse>> {
    let mut conn = state.pool.get().await?;
    let decision = decisions::get_by_id(&mut conn, id).await?;
    Ok(Json(decision.into()))
}

pub async fn create_decision(
    State(state): State<AppState>,
    Json(payload): Json<CreateAgentDecisionRequest>,
) -> ApiResult<Json<AgentDecisionResponse>> {
    let mut conn = state.pool.get().await?;
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
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ApproveDecisionRequest>,
) -> ApiResult<Json<AgentDecisionResponse>> {
    let mut conn = state.pool.get().await?;
    let result = DecisionService::approve(&mut conn, id, payload.modifications).await?;
    Ok(Json(result.decision.into()))
}

pub async fn reject_decision(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<RejectDecisionRequest>,
) -> ApiResult<Json<AgentDecisionResponse>> {
    let mut conn = state.pool.get().await?;
    let decision = DecisionService::reject(&mut conn, id, payload.feedback.as_deref()).await?;
    Ok(Json(decision.into()))
}

pub async fn get_decision_stats(State(state): State<AppState>) -> ApiResult<Json<DecisionStats>> {
    let mut conn = state.pool.get().await?;
    let stats = decisions::get_stats(&mut conn).await?;
    Ok(Json(stats))
}

pub async fn batch_approve_decisions(
    State(state): State<AppState>,
    Json(payload): Json<BatchApproveDecisionsRequest>,
) -> ApiResult<Json<BatchOperationResponse>> {
    let result = DecisionService::batch_approve(&state.pool, payload.decision_ids).await?;
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
    State(state): State<AppState>,
    Json(payload): Json<BatchRejectDecisionsRequest>,
) -> ApiResult<Json<BatchOperationResponse>> {
    let result = DecisionService::batch_reject(
        &state.pool,
        payload.decision_ids,
        payload.feedback.as_deref(),
    )
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
    State(state): State<AppState>,
    Query(query): Query<RuleListQuery>,
) -> ApiResult<Json<Vec<AgentRuleResponse>>> {
    let mut conn = state.pool.get().await?;

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
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<AgentRuleResponse>> {
    let mut conn = state.pool.get().await?;
    let rule = agent_rules::get_by_id(&mut conn, id).await?;
    let response: AgentRuleResponse = rule.try_into()?;
    Ok(Json(response))
}

pub async fn create_agent_rule(
    State(state): State<AppState>,
    Json(payload): Json<CreateAgentRuleRequest>,
) -> ApiResult<Json<AgentRuleResponse>> {
    let mut conn = state.pool.get().await?;

    let rule = agent_rules::create(&mut conn, &payload).await?;
    let response: AgentRuleResponse = rule.try_into()?;
    Ok(Json(response))
}

pub async fn update_agent_rule(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAgentRuleRequest>,
) -> ApiResult<Json<AgentRuleResponse>> {
    let mut conn = state.pool.get().await?;
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
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let mut conn = state.pool.get().await?;
    agent_rules::delete(&mut conn, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize)]
pub struct ToggleActiveResponse {
    pub id: Uuid,
    pub is_active: bool,
}

pub async fn toggle_agent_rule_active(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ToggleActiveResponse>> {
    let mut conn = state.pool.get().await?;
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
    State(state): State<AppState>,
    Query(query): Query<ChatHistoryQuery>,
) -> ApiResult<Json<Vec<ChatMessageResponse>>> {
    let mut conn = state.pool.get().await?;
    let messages = chat_messages::list_history(&mut conn, query.limit, query.before).await?;
    let responses: Vec<ChatMessageResponse> = messages.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}

pub async fn send_chat_message(
    State(state): State<AppState>,
    Json(payload): Json<SendChatMessageRequest>,
) -> ApiResult<Json<ChatResponse>> {
    let mut conn = state.pool.get().await?;

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

pub async fn clear_chat_history(State(state): State<AppState>) -> ApiResult<StatusCode> {
    let mut conn = state.pool.get().await?;

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
    State(state): State<AppState>,
    Query(params): Query<CalendarEventQuery>,
) -> ApiResult<Json<Vec<CalendarEventResponse>>> {
    let mut conn = state.pool.get().await?;
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
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> ApiResult<Json<CalendarEventResponse>> {
    let mut conn = state.pool.get().await?;
    let event = calendar_events::get_by_id(&mut conn, event_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Calendar event"))?;
    Ok(Json(event.into()))
}

pub async fn get_todays_events(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<CalendarEventResponse>>> {
    let mut conn = state.pool.get().await?;
    let events = calendar_events::get_today(&mut conn).await?;
    let responses: Vec<CalendarEventResponse> = events.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}

pub async fn get_this_weeks_events(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<CalendarEventResponse>>> {
    let mut conn = state.pool.get().await?;
    let events = calendar_events::get_this_week(&mut conn).await?;
    let responses: Vec<CalendarEventResponse> = events.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}
