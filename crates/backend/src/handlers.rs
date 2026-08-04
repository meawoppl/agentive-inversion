use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use shared_types::{
    AboutMeResponse, AgentDecisionResponse, ApproveDecisionRequest, ArchiveReviewItem,
    ArchiveReviewResponse, BatchApproveDecisionsRequest, BatchOperationFailure,
    BatchOperationResponse, BatchRejectDecisionsRequest, CalendarEventQuery, CalendarEventResponse,
    Category, ChatHistoryQuery, ChatIntent, ChatMessageResponse, ChatResponse,
    ClaudeAuthCompleteRequest, ClaudeAuthStartResponse, ClaudeAuthStatusResponse,
    CreateAgentDecisionRequest, CreateCalendarEventRequest, CreateCalendarEventResponse,
    CreateCategoryRequest, CreateTodoRequest, DecisionStats, EmailListQuery, EmailResponse,
    GoogleAccountResponse, PipelineStatsResponse, RejectDecisionRequest, SendChatMessageRequest,
    SuggestedAction, Todo, TriageDecideRequest, TriageDecideResponse, TriageStageCount,
    UpdateAboutMeRequest, UpdateCategoryRequest, UpdateTodoRequest,
};
use uuid::Uuid;

// Authentication is handled by middleware layer in main.rs
use crate::db::{
    about_me, calendar_events, categories, chat_messages, decisions, emails, get_conn,
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
            triage_status: e.triage_status,
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
        triage_status: email.triage_status,
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

// About-me document handlers
pub async fn get_about_me(State(state): State<AppState>) -> ApiResult<Json<AboutMeResponse>> {
    let mut conn = get_conn(&state.pool).await?;
    let doc = about_me::get(&mut conn).await?;
    Ok(Json(AboutMeResponse {
        content: doc.content,
        updated_at: doc.updated_at,
    }))
}

pub async fn update_about_me(
    State(state): State<AppState>,
    Json(req): Json<UpdateAboutMeRequest>,
) -> ApiResult<Json<AboutMeResponse>> {
    let mut conn = get_conn(&state.pool).await?;
    let doc = about_me::update(&mut conn, &req.content).await?;
    Ok(Json(AboutMeResponse {
        content: doc.content,
        updated_at: doc.updated_at,
    }))
}

// Calendar event creation (executes a write to the agent calendar)
pub async fn create_calendar_event(
    State(state): State<AppState>,
    Json(req): Json<CreateCalendarEventRequest>,
) -> ApiResult<Json<CreateCalendarEventResponse>> {
    use crate::calendar_writer::{CalendarWriter, NewCalendarEvent};

    let mut conn = get_conn(&state.pool).await?;
    let account = google_accounts::get_by_email(&mut conn, &req.account_email)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!("No connected account: {}", req.account_email))
        })?;
    drop(conn);

    let writer = CalendarWriter::from_account(&account)
        .await
        .map_err(ApiError::Internal)?;

    let calendar_name = req.calendar_name.as_deref().unwrap_or("Agent");
    let calendar_id = writer
        .ensure_calendar(calendar_name)
        .await
        .map_err(ApiError::Internal)?;

    let created = writer
        .create_event(
            &calendar_id,
            NewCalendarEvent {
                summary: req.summary,
                description: req.description,
                location: req.location,
                start: req.start,
                end: req.end,
                email_link: req.email_link,
            },
        )
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(CreateCalendarEventResponse {
        google_event_id: created.google_event_id,
        html_link: created.html_link,
        calendar_id: created.calendar_id,
    }))
}

// Triage decision endpoint (called by agent-cli from agent sessions)
pub async fn post_triage_decision(
    State(state): State<AppState>,
    Json(req): Json<TriageDecideRequest>,
) -> ApiResult<Json<TriageDecideResponse>> {
    let resp = crate::services::TriageService::apply(&state.pool, req)
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(resp))
}

// Pipeline stats for the pipeline screen and monitoring (stable typed shape)
pub async fn get_pipeline_stats(
    State(state): State<AppState>,
) -> ApiResult<Json<PipelineStatsResponse>> {
    let mut conn = get_conn(&state.pool).await?;
    let counts = emails::triage_status_counts(&mut conn).await?;

    let health = state.triage_health.read().await.clone();

    Ok(Json(PipelineStatsResponse {
        mode: health.mode,
        last_cycle_at: health.last_cycle_at,
        last_cycle_error: health.last_cycle_error,
        consecutive_failures: health.consecutive_failures,
        stage_counts: counts
            .into_iter()
            .map(|(status, count)| TriageStageCount { status, count })
            .collect(),
    }))
}

// ============================================================================
// Claude Code login flow (mints the pipeline's subscription credential)
// ============================================================================

/// Abandoned login flows are reaped after this long (see the reaper task in
/// main.rs); complete also refuses flows older than this
pub const LOGIN_FLOW_TTL: std::time::Duration = std::time::Duration::from_secs(600);

pub async fn claude_auth_start(
    State(state): State<AppState>,
) -> ApiResult<Json<ClaudeAuthStartResponse>> {
    use claude_codes::auth::{LoginFlow, LoginMode};

    tracing::info!("Claude login: starting setup-token flow");

    // PTY interaction is blocking; drive it off the async runtime
    let (flow, auth_url) = tokio::task::spawn_blocking(|| {
        let mut flow = LoginFlow::start(LoginMode::SetupToken)
            .map_err(|e| anyhow::anyhow!("Failed to start claude login: {e}"))?;
        let url = flow
            .auth_url(std::time::Duration::from_secs(30))
            .map_err(|e| anyhow::anyhow!("Login flow produced no auth URL: {e}"))?;
        Ok::<_, anyhow::Error>((flow, url))
    })
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Login task panicked: {e}")))?
    .map_err(|e| {
        tracing::error!("Claude login: start failed: {e:#}");
        ApiError::Internal(e)
    })?;

    // Replacing any previous in-flight flow cancels it (Drop kills the child)
    *state
        .claude_login
        .lock()
        .expect("claude_login mutex poisoned") = Some((flow, std::time::Instant::now()));

    tracing::info!("Claude login: auth URL extracted, awaiting code paste");
    Ok(Json(ClaudeAuthStartResponse { auth_url }))
}

pub async fn claude_auth_complete(
    State(state): State<AppState>,
    Json(req): Json<ClaudeAuthCompleteRequest>,
) -> ApiResult<Json<ClaudeAuthStatusResponse>> {
    let (flow, started) = state
        .claude_login
        .lock()
        .expect("claude_login mutex poisoned")
        .take()
        .ok_or_else(|| {
            ApiError::Gone("No Claude login in progress - start one first".to_string())
        })?;

    if started.elapsed() > LOGIN_FLOW_TTL {
        // Dropping the flow kills the PTY child
        tracing::info!("Claude login: flow expired before code arrived");
        return Err(ApiError::Gone(
            "Login expired - start a new one".to_string(),
        ));
    }

    // Browser copies inject invisible unicode that trim() does not touch;
    // an empty or poisoned code presses Enter on a bad field and presents as
    // a silent timeout. Sanitize before spending a flow attempt.
    let code: String = req
        .code
        .chars()
        .filter(|c| {
            !c.is_whitespace()
                && !matches!(
                    c,
                    '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' | '\u{00A0}'
                )
        })
        .collect();
    if code.is_empty() {
        // Flow stays stored: nothing was submitted, so the session is intact
        *state
            .claude_login
            .lock()
            .expect("claude_login mutex poisoned") = Some((flow, started));
        return Err(ApiError::BadRequest(
            "Pasted code is empty after removing whitespace - copy it again".to_string(),
        ));
    }
    tracing::info!(
        "Claude login: code received (len={}), submitting to CLI",
        code.len()
    );

    // Outcome-driven wait: returns on minted token or CLI OAuth error, not on
    // process exit (the CLI never exits on a rejected code)
    let result = tokio::task::spawn_blocking(move || {
        let mut flow = flow;
        let outcome = flow.submit_code_and_wait(&code, std::time::Duration::from_secs(90));
        (flow, outcome)
    })
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Login task panicked: {e}")))?;

    let (flow, outcome) = result;
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(claude_codes::Error::CodeRejected { message }) => {
            // A rejected code is terminal for this CLI session: the error
            // screen renders no input component, and Enter restarts the OAuth
            // flow with a fresh PKCE verifier — a corrected paste has nowhere
            // to land (read out of the Ink state machine and verified against
            // the live CLI). Kill the child; the UI starts a new login.
            tracing::warn!("Claude login: code rejected by CLI: {message}");
            drop(flow);
            return Err(ApiError::BadRequest(format!(
                "Code rejected: {message} - start a new login and paste the fresh code"
            )));
        }
        Err(claude_codes::Error::LoginTimeout { transcript }) => {
            // The transcript leads with the channel diagnostics line
            // ([channels: screen=... osc52=... credentials=...]), which names
            // the stalled leg instead of leaving a silent mystery
            tracing::error!("Claude login: timed out; transcript: {transcript}");
            drop(flow);
            let tail: String = transcript.chars().take(400).collect();
            return Err(ApiError::Internal(anyhow::anyhow!(
                "Login timed out. CLI state: {tail}"
            )));
        }
        Err(e) => {
            // Fatal: dropping the flow kills the child; UI restarts the flow
            tracing::error!("Claude login: failed: {e}");
            drop(flow);
            return Err(ApiError::Internal(anyhow::anyhow!(
                "Login did not complete: {e}"
            )));
        }
    };
    drop(flow); // success: reap the CLI child

    tracing::info!(
        "Claude login: outcome channels: token_source={:?} osc52={:?} copy_nudge_sent={}",
        outcome.token_source,
        outcome.osc52,
        outcome.copy_nudge_sent
    );

    let token = match (outcome.token, outcome.credentials_updated) {
        (Some(token), _) => token,
        (None, true) => {
            // Legitimate success: the CLI accepted the code and persisted its
            // own credentials, but never exposed the token over the PTY.
            // Store the cli-persisted sentinel (empty token): the pipeline
            // then relies on the CLI's disk credentials instead of an env
            // token. Those live only inside the container, so a redeploy
            // requires re-login.
            tracing::warn!(
                "Claude login: succeeded via CLI-persisted credentials (token not \
                 capturable; osc52={:?}); credentials persist only as durably as \
                 ~/.claude does (persisted via volume on this deployment)",
                outcome.osc52
            );
            String::new()
        }
        (None, false) => {
            let tail: String = outcome
                .transcript
                .chars()
                .rev()
                .take(300)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            return Err(ApiError::Internal(anyhow::anyhow!(
                "Login finished without minting a token. CLI output ended with: {tail}"
            )));
        }
    };

    let mut conn = get_conn(&state.pool).await?;
    crate::db::claude_credentials::upsert(&mut conn, &token).await?;
    let cred = crate::db::claude_credentials::get(&mut conn).await?;

    tracing::info!("Claude login: credential minted and stored");

    Ok(Json(ClaudeAuthStatusResponse {
        connected: true,
        updated_at: cred.map(|c| c.updated_at),
    }))
}

pub async fn claude_auth_status(
    State(state): State<AppState>,
) -> ApiResult<Json<ClaudeAuthStatusResponse>> {
    let mut conn = get_conn(&state.pool).await?;
    let cred = crate::db::claude_credentials::get(&mut conn).await?;
    Ok(Json(ClaudeAuthStatusResponse {
        connected: cred.is_some(),
        updated_at: cred.map(|c| c.updated_at),
    }))
}

// Bulk audit surface for archive determinations (the dry-run review)
pub async fn get_archive_review(
    State(state): State<AppState>,
) -> ApiResult<Json<ArchiveReviewResponse>> {
    let mut conn = get_conn(&state.pool).await?;

    let decisions_list = decisions::list_by_type(&mut conn, "archive", 500).await?;
    let email_ids: Vec<Uuid> = decisions_list.iter().filter_map(|d| d.source_id).collect();
    let emails_list = emails::list_by_ids(&mut conn, &email_ids).await?;
    let accounts = google_accounts::list_all(&mut conn).await?;

    let items = decisions_list
        .into_iter()
        .filter_map(|d| {
            let email = d
                .source_id
                .and_then(|sid| emails_list.iter().find(|e| e.id == sid))?;
            let account_email = accounts
                .iter()
                .find(|a| a.id == email.account_id)
                .map(|a| a.email.clone())
                .unwrap_or_default();
            Some(ArchiveReviewItem {
                decision_id: d.id,
                email_id: email.id,
                status: d.status.clone(),
                confidence: d.confidence,
                reasoning: d.reasoning.clone(),
                decided_at: d.created_at,
                account_email,
                subject: email.subject.clone(),
                from_address: email.from_address.clone(),
                from_name: email.from_name.clone(),
                received_at: email.received_at,
                snippet: email.snippet.clone(),
            })
        })
        .collect();

    Ok(Json(ArchiveReviewResponse {
        archive_mode: crate::services::triage::archive_mode(),
        items,
    }))
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
    let decision_type = result.decision.decision_type.clone();
    drop(conn);

    // Approving a gated proposal executes its side effect
    if execute_approved_side_effect(&state, id, &decision_type).await? {
        let mut conn = state.pool.get().await?;
        let refreshed = decisions::get_by_id(&mut conn, id).await?;
        return Ok(Json(refreshed.into()));
    }

    Ok(Json(result.decision.into()))
}

/// Run the external side effect for an approved decision, if it has one.
/// Returns true when something executed (caller should re-read the decision).
async fn execute_approved_side_effect(
    state: &AppState,
    decision_id: Uuid,
    decision_type: &str,
) -> Result<bool, ApiError> {
    match decision_type {
        "create_calendar_event" => {
            crate::services::TriageService::execute_calendar_decision(&state.pool, decision_id)
                .await
                .map_err(ApiError::Internal)?;
            Ok(true)
        }
        "archive" => {
            crate::services::TriageService::execute_archive_decision(&state.pool, decision_id)
                .await
                .map_err(ApiError::Internal)?;
            Ok(true)
        }
        _ => Ok(false),
    }
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
    let mut failed: Vec<BatchOperationFailure> = result
        .failed
        .into_iter()
        .map(|f| BatchOperationFailure {
            id: f.id,
            error: f.error,
        })
        .collect();

    // Execute side effects (archives, calendar writes) per approved decision;
    // an execution failure downgrades that id to failed rather than aborting
    let mut successful = Vec::with_capacity(result.successful.len());
    for id in result.successful {
        let decision_type = {
            let mut conn = state.pool.get().await?;
            decisions::get_by_id(&mut conn, id).await?.decision_type
        };
        match execute_approved_side_effect(&state, id, &decision_type).await {
            Ok(_) => successful.push(id),
            Err(e) => failed.push(BatchOperationFailure {
                id,
                error: format!("approved but execution failed: {e:?}"),
            }),
        }
    }

    Ok(Json(BatchOperationResponse { successful, failed }))
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
