use gloo_net::http::Request;
use shared_types::{
    AgentDecisionResponse, ApproveDecisionRequest, AuthUserResponse, BatchApproveDecisionsRequest,
    BatchRejectDecisionsRequest, CalendarEventResponse, Category, ChatMessageResponse,
    ChatResponse, CreateTodoRequest, DecisionStats, EmailResponse, LoginInitResponse,
    ProposedTodoAction, RejectDecisionRequest, SendChatMessageRequest, SuggestedAction, Todo,
    UpdateTodoRequest,
};
use uuid::Uuid;
use web_sys::{Element, HtmlInputElement};
use yew::prelude::*;

// ============================================================================
// Authentication Types
// ============================================================================

/// Authentication state for the app
#[derive(Clone, PartialEq)]
pub enum AuthState {
    /// Initial state - checking auth status
    Unknown,
    /// User is authenticated
    Authenticated { email: String, name: Option<String> },
    /// User is not authenticated
    Unauthenticated,
}

// ============================================================================
// App State Context
// ============================================================================

/// View/tab options for the application
#[derive(Clone, PartialEq, Copy)]
pub enum View {
    Inbox,
    Todos,
    Calendar,
    DecisionLog,
    Categories,
}

/// Shared application state accessible via context
#[derive(Clone, PartialEq)]
pub struct AppState {
    pub current_view: View,
    pub pending_decisions_count: i64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_view: View::Inbox,
            pending_decisions_count: 0,
        }
    }
}

/// Context value providing both state and setter
#[derive(Clone, PartialEq)]
pub struct AppContext {
    pub state: AppState,
    pub set_view: Callback<View>,
    pub set_pending_count: Callback<i64>,
    pub refresh_pending_count: Callback<()>,
}

/// Hook to access app context
#[hook]
pub fn use_app_context() -> AppContext {
    use_context::<AppContext>()
        .expect("AppContext not found - wrap component in AppContextProvider")
}

/// Provider component that wraps children with app state context
#[derive(Properties, PartialEq)]
pub struct AppContextProviderProps {
    #[prop_or_default]
    pub children: Html,
}

#[function_component(AppContextProvider)]
pub fn app_context_provider(props: &AppContextProviderProps) -> Html {
    let current_view = use_state(|| View::Inbox);
    let pending_count = use_state(|| 0i64);

    // Fetch initial pending count
    {
        let pending_count = pending_count.clone();
        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(response) = Request::get("/api/decisions/stats").send().await {
                    if response.ok() {
                        if let Ok(stats) = response.json::<DecisionStats>().await {
                            pending_count.set(stats.pending);
                        }
                    }
                }
            });
            || ()
        });
    }

    let set_view = {
        let current_view = current_view.clone();
        Callback::from(move |view: View| current_view.set(view))
    };

    let set_pending_count = {
        let pending_count = pending_count.clone();
        Callback::from(move |count: i64| pending_count.set(count))
    };

    let refresh_pending_count = {
        let pending_count = pending_count.clone();
        Callback::from(move |_: ()| {
            let pending_count = pending_count.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(response) = Request::get("/api/decisions/stats").send().await {
                    if response.ok() {
                        if let Ok(stats) = response.json::<DecisionStats>().await {
                            pending_count.set(stats.pending);
                        }
                    }
                }
            });
        })
    };

    let context = AppContext {
        state: AppState {
            current_view: *current_view,
            pending_decisions_count: *pending_count,
        },
        set_view,
        set_pending_count,
        refresh_pending_count,
    };

    html! {
        <ContextProvider<AppContext> context={context}>
            { props.children.clone() }
        </ContextProvider<AppContext>>
    }
}

#[function_component(App)]
fn app() -> Html {
    let auth_state = use_state(|| AuthState::Unknown);

    // Check authentication status on mount
    {
        let auth_state = auth_state.clone();
        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                match Request::get("/api/auth/me").send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<AuthUserResponse>().await {
                                Ok(user) => {
                                    auth_state.set(AuthState::Authenticated {
                                        email: user.email,
                                        name: user.name,
                                    });
                                }
                                Err(_) => {
                                    auth_state.set(AuthState::Unauthenticated);
                                }
                            }
                        } else {
                            auth_state.set(AuthState::Unauthenticated);
                        }
                    }
                    Err(_) => {
                        auth_state.set(AuthState::Unauthenticated);
                    }
                }
            });
            || ()
        });
    }

    let on_logout = {
        let auth_state = auth_state.clone();
        Callback::from(move |_| {
            let auth_state = auth_state.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(response) = Request::post("/api/auth/logout").send().await {
                    if response.ok() || response.status() == 303 {
                        auth_state.set(AuthState::Unauthenticated);
                    }
                }
            });
        })
    };

    match &*auth_state {
        AuthState::Unknown => {
            html! {
                <div class="auth-loading">
                    <div class="auth-loading-spinner"></div>
                    <p>{"Loading..."}</p>
                </div>
            }
        }
        AuthState::Unauthenticated => {
            html! { <LoginPage /> }
        }
        AuthState::Authenticated { email, name } => {
            html! {
                <AppContextProvider>
                    <AuthenticatedApp
                        email={email.clone()}
                        name={name.clone()}
                        on_logout={on_logout}
                    />
                </AppContextProvider>
            }
        }
    }
}

// ============================================================================
// Login Page Component
// ============================================================================

#[function_component(LoginPage)]
fn login_page() -> Html {
    let loading = use_state(|| false);
    let error = use_state(|| None::<String>);

    let on_login = {
        let loading = loading.clone();
        let error = error.clone();
        Callback::from(move |_| {
            let loading = loading.clone();
            let error = error.clone();
            loading.set(true);
            error.set(None);
            wasm_bindgen_futures::spawn_local(async move {
                match Request::get("/api/auth/login").send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<LoginInitResponse>().await {
                                Ok(login_resp) => {
                                    // Redirect to Google OAuth
                                    if let Some(window) = web_sys::window() {
                                        let _ = window.location().set_href(&login_resp.auth_url);
                                    }
                                }
                                Err(e) => {
                                    error.set(Some(format!("Failed to start login: {}", e)));
                                    loading.set(false);
                                }
                            }
                        } else {
                            error.set(Some("Failed to start login process".to_string()));
                            loading.set(false);
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                        loading.set(false);
                    }
                }
            });
        })
    };

    // Check for error in URL params (from OAuth callback)
    let url_error = use_state(|| None::<String>);
    {
        let url_error = url_error.clone();
        use_effect_with((), move |_| {
            if let Some(window) = web_sys::window() {
                if let Ok(search) = window.location().search() {
                    if search.contains("auth_error=") {
                        let error_msg = if search.contains("unauthorized_email") {
                            "Your email is not authorized to access this application."
                        } else if search.contains("token_exchange_failed") {
                            "Failed to complete authentication with Google."
                        } else {
                            "Authentication failed. Please try again."
                        };
                        url_error.set(Some(error_msg.to_string()));
                        // Clear the URL params
                        let _ = window.history().and_then(|h| {
                            h.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some("/"))
                        });
                    }
                }
            }
            || ()
        });
    }

    html! {
        <div class="login-page">
            <div class="login-card">
                <div class="login-header">
                    <h1>{"Agentive Inversion"}</h1>
                    <p>{"Your AI-powered task and email assistant"}</p>
                </div>

                {if let Some(err) = (*url_error).as_ref() {
                    html! { <div class="login-error">{err}</div> }
                } else if let Some(err) = (*error).as_ref() {
                    html! { <div class="login-error">{err}</div> }
                } else {
                    html! {}
                }}

                <button
                    class="login-button"
                    onclick={on_login}
                    disabled={*loading}
                >
                    {if *loading {
                        html! { <span class="login-spinner"></span> }
                    } else {
                        html! {
                            <>
                                <svg class="google-icon" viewBox="0 0 24 24" width="20" height="20">
                                    <path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"/>
                                    <path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"/>
                                    <path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"/>
                                    <path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"/>
                                </svg>
                                {"Sign in with Google"}
                            </>
                        }
                    }}
                </button>

                <p class="login-info">
                    {"Only authorized email addresses can access this application."}
                </p>
            </div>
        </div>
    }
}

// ============================================================================
// Authenticated App Wrapper
// ============================================================================

#[derive(Properties, PartialEq)]
pub struct AuthenticatedAppProps {
    pub email: String,
    pub name: Option<String>,
    pub on_logout: Callback<()>,
}

#[function_component(AuthenticatedApp)]
fn authenticated_app(props: &AuthenticatedAppProps) -> Html {
    let ctx = use_app_context();
    let current_view = ctx.state.current_view;
    let pending_count = ctx.state.pending_decisions_count;

    let set_view_inbox = {
        let set_view = ctx.set_view.clone();
        Callback::from(move |_| set_view.emit(View::Inbox))
    };

    let set_view_todos = {
        let set_view = ctx.set_view.clone();
        Callback::from(move |_| set_view.emit(View::Todos))
    };

    let set_view_log = {
        let set_view = ctx.set_view.clone();
        Callback::from(move |_| set_view.emit(View::DecisionLog))
    };

    let set_view_categories = {
        let set_view = ctx.set_view.clone();
        Callback::from(move |_| set_view.emit(View::Categories))
    };

    let set_view_calendar = {
        let set_view = ctx.set_view.clone();
        Callback::from(move |_| set_view.emit(View::Calendar))
    };

    let on_logout = props.on_logout.clone();

    html! {
        <div class="app">
            <header>
                <div class="header-top">
                    <h1>{"Agentive Inversion"}</h1>
                    <div class="user-menu">
                        <span class="user-email">
                            {props.name.as_ref().unwrap_or(&props.email)}
                        </span>
                        <button class="logout-btn" onclick={Callback::from(move |_| on_logout.emit(()))}>
                            {"Logout"}
                        </button>
                    </div>
                </div>
                <nav class="main-nav">
                    <button
                        class={if current_view == View::Inbox { "nav-btn active" } else { "nav-btn" }}
                        onclick={set_view_inbox}
                    >
                        {"Inbox"}
                        {if pending_count > 0 {
                            html! { <span class="nav-badge">{pending_count}</span> }
                        } else {
                            html! {}
                        }}
                    </button>
                    <button
                        class={if current_view == View::Todos { "nav-btn active" } else { "nav-btn" }}
                        onclick={set_view_todos}
                    >
                        {"Todos"}
                    </button>
                    <button
                        class={if current_view == View::Calendar { "nav-btn active" } else { "nav-btn" }}
                        onclick={set_view_calendar}
                    >
                        {"Calendar"}
                    </button>
                    <button
                        class={if current_view == View::DecisionLog { "nav-btn active" } else { "nav-btn" }}
                        onclick={set_view_log}
                    >
                        {"Decision Log"}
                    </button>
                    <button
                        class={if current_view == View::Categories { "nav-btn active" } else { "nav-btn" }}
                        onclick={set_view_categories}
                    >
                        {"Categories"}
                    </button>
                </nav>
            </header>
            <main>
                {match current_view {
                    View::Inbox => html! { <DecisionInbox /> },
                    View::Todos => html! { <TodoList /> },
                    View::Calendar => html! { <CalendarView /> },
                    View::DecisionLog => html! { <DecisionLog /> },
                    View::Categories => html! { <CategoryManager /> },
                }}
            </main>
            <ChatWidget />
        </div>
    }
}

// ============================================================================
// Decision Inbox Component - Shows pending decisions for review
// ============================================================================

#[function_component(DecisionInbox)]
fn decision_inbox() -> Html {
    let ctx = use_app_context();
    let decisions = use_state(Vec::<AgentDecisionResponse>::new);
    let emails = use_state(std::collections::HashMap::<Uuid, EmailResponse>::new);
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    let refresh_trigger = use_state(|| 0u32);
    let selected_decisions = use_state(std::collections::HashSet::<Uuid>::new);
    let selected_decision = use_state(|| None::<AgentDecisionResponse>);

    // Fetch pending decisions
    {
        let decisions = decisions.clone();
        let emails = emails.clone();
        let loading = loading.clone();
        let error = error.clone();
        let refresh_trigger = *refresh_trigger;

        use_effect_with(refresh_trigger, move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                // Fetch pending decisions
                match Request::get("/api/decisions/pending").send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<Vec<AgentDecisionResponse>>().await {
                                Ok(data) => {
                                    // Fetch email details for each decision
                                    let mut email_map = std::collections::HashMap::new();
                                    for decision in &data {
                                        if decision.source_type == "email" {
                                            if let Some(source_id) = decision.source_id {
                                                if let Ok(email_resp) = Request::get(&format!(
                                                    "/api/emails/{}",
                                                    source_id
                                                ))
                                                .send()
                                                .await
                                                {
                                                    if email_resp.ok() {
                                                        if let Ok(email) =
                                                            email_resp.json::<EmailResponse>().await
                                                        {
                                                            email_map.insert(source_id, email);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    emails.set(email_map);
                                    decisions.set(data);
                                    loading.set(false);
                                }
                                Err(e) => {
                                    error.set(Some(format!("Failed to parse decisions: {}", e)));
                                    loading.set(false);
                                }
                            }
                        } else {
                            error.set(Some(format!("API error: {}", response.status())));
                            loading.set(false);
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                        loading.set(false);
                    }
                }
            });
            || ()
        });
    }

    let on_approve = {
        let refresh_trigger = refresh_trigger.clone();
        let selected_decision = selected_decision.clone();
        let refresh_pending = ctx.refresh_pending_count.clone();
        Callback::from(move |id: Uuid| {
            let refresh_trigger = refresh_trigger.clone();
            let selected_decision = selected_decision.clone();
            let refresh_pending = refresh_pending.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let request = ApproveDecisionRequest {
                    modifications: None,
                    create_rule: None,
                    rule_name: None,
                };
                if let Ok(response) = Request::post(&format!("/api/decisions/{}/approve", id))
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&request).unwrap())
                    .unwrap()
                    .send()
                    .await
                {
                    if response.ok() {
                        selected_decision.set(None);
                        refresh_trigger.set(*refresh_trigger + 1);
                        refresh_pending.emit(());
                    }
                }
            });
        })
    };

    let on_reject = {
        let refresh_trigger = refresh_trigger.clone();
        let selected_decision = selected_decision.clone();
        let refresh_pending = ctx.refresh_pending_count.clone();
        Callback::from(move |(id, feedback): (Uuid, Option<String>)| {
            let refresh_trigger = refresh_trigger.clone();
            let selected_decision = selected_decision.clone();
            let refresh_pending = refresh_pending.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let request = RejectDecisionRequest {
                    feedback,
                    create_rule: None,
                    rule_action: None,
                };
                if let Ok(response) = Request::post(&format!("/api/decisions/{}/reject", id))
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&request).unwrap())
                    .unwrap()
                    .send()
                    .await
                {
                    if response.ok() {
                        selected_decision.set(None);
                        refresh_trigger.set(*refresh_trigger + 1);
                        refresh_pending.emit(());
                    }
                }
            });
        })
    };

    let on_batch_approve = {
        let selected_decisions = selected_decisions.clone();
        let refresh_trigger = refresh_trigger.clone();
        let refresh_pending = ctx.refresh_pending_count.clone();
        Callback::from(move |_| {
            let ids: Vec<Uuid> = selected_decisions.iter().copied().collect();
            if ids.is_empty() {
                return;
            }
            let selected_decisions = selected_decisions.clone();
            let refresh_trigger = refresh_trigger.clone();
            let refresh_pending = refresh_pending.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let request = BatchApproveDecisionsRequest { decision_ids: ids };
                if let Ok(response) = Request::post("/api/decisions/batch/approve")
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&request).unwrap())
                    .unwrap()
                    .send()
                    .await
                {
                    if response.ok() {
                        selected_decisions.set(std::collections::HashSet::new());
                        refresh_trigger.set(*refresh_trigger + 1);
                        refresh_pending.emit(());
                    }
                }
            });
        })
    };

    let on_batch_reject = {
        let selected_decisions = selected_decisions.clone();
        let refresh_trigger = refresh_trigger.clone();
        let refresh_pending = ctx.refresh_pending_count.clone();
        Callback::from(move |_| {
            let ids: Vec<Uuid> = selected_decisions.iter().copied().collect();
            if ids.is_empty() {
                return;
            }
            let selected_decisions = selected_decisions.clone();
            let refresh_trigger = refresh_trigger.clone();
            let refresh_pending = refresh_pending.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let request = BatchRejectDecisionsRequest {
                    decision_ids: ids,
                    feedback: None,
                };
                if let Ok(response) = Request::post("/api/decisions/batch/reject")
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&request).unwrap())
                    .unwrap()
                    .send()
                    .await
                {
                    if response.ok() {
                        selected_decisions.set(std::collections::HashSet::new());
                        refresh_trigger.set(*refresh_trigger + 1);
                        refresh_pending.emit(());
                    }
                }
            });
        })
    };

    let toggle_selection = {
        let selected_decisions = selected_decisions.clone();
        Callback::from(move |id: Uuid| {
            let mut new_set = (*selected_decisions).clone();
            if new_set.contains(&id) {
                new_set.remove(&id);
            } else {
                new_set.insert(id);
            }
            selected_decisions.set(new_set);
        })
    };

    let select_all = {
        let selected_decisions = selected_decisions.clone();
        let decisions = decisions.clone();
        Callback::from(move |_| {
            let all_ids: std::collections::HashSet<Uuid> = decisions.iter().map(|d| d.id).collect();
            selected_decisions.set(all_ids);
        })
    };

    let clear_selection = {
        let selected_decisions = selected_decisions.clone();
        Callback::from(move |_| {
            selected_decisions.set(std::collections::HashSet::new());
        })
    };

    if *loading {
        return html! {
            <div class="decision-inbox">
                <h2>{"Inbox - Pending Decisions"}</h2>
                <p class="loading">{"Loading decisions..."}</p>
            </div>
        };
    }

    if let Some(err) = &*error {
        return html! {
            <div class="decision-inbox">
                <h2>{"Inbox - Pending Decisions"}</h2>
                <p class="error">{err}</p>
            </div>
        };
    }

    let decisions_list = (*decisions).clone();
    let emails_map = (*emails).clone();
    let has_selected = !selected_decisions.is_empty();

    html! {
        <div class="decision-inbox">
            <div class="inbox-header">
                <h2>{"Inbox - Pending Decisions"}</h2>
                <p class="decision-count">{format!("{} decisions awaiting review", decisions_list.len())}</p>
            </div>

            {if !decisions_list.is_empty() {
                html! {
                    <div class="batch-actions">
                        <button class="btn-secondary" onclick={select_all}>{"Select All"}</button>
                        <button class="btn-secondary" onclick={clear_selection}>{"Clear Selection"}</button>
                        {if has_selected {
                            html! {
                                <>
                                    <button class="btn-approve" onclick={on_batch_approve.clone()}>
                                        {format!("Approve Selected ({})", selected_decisions.len())}
                                    </button>
                                    <button class="btn-reject" onclick={on_batch_reject.clone()}>
                                        {format!("Reject Selected ({})", selected_decisions.len())}
                                    </button>
                                </>
                            }
                        } else {
                            html! {}
                        }}
                    </div>
                }
            } else {
                html! {}
            }}

            <div class="decision-list">
                {if decisions_list.is_empty() {
                    html! { <p class="empty-state">{"No pending decisions. All caught up!"}</p> }
                } else {
                    decisions_list.iter().map(|decision| {
                        let decision_id = decision.id;
                        let is_selected = selected_decisions.contains(&decision_id);
                        let toggle = toggle_selection.clone();
                        let approve = on_approve.clone();
                        let reject = on_reject.clone();
                        let select_detail = selected_decision.clone();
                        let decision_clone = decision.clone();

                        // Get email info if available
                        let email = decision.source_id.and_then(|sid| emails_map.get(&sid));

                        // Parse proposed action
                        let proposed: Option<ProposedTodoAction> = serde_json::from_value(decision.proposed_action.clone()).ok();

                        html! {
                            <div key={decision.id.to_string()}
                                class={format!("decision-item {} {}",
                                    if is_selected { "selected" } else { "" },
                                    decision.confidence_level.as_str())}>
                                <div class="decision-select">
                                    <input
                                        type="checkbox"
                                        checked={is_selected}
                                        onchange={Callback::from(move |_| toggle.emit(decision_id))}
                                    />
                                </div>
                                <div class="decision-content" onclick={Callback::from(move |_| select_detail.set(Some(decision_clone.clone())))}>
                                    <div class="decision-header">
                                        <span class="decision-source">
                                            {if decision.source_type == "email" { "Email" } else { &decision.source_type }}
                                        </span>
                                        <span class={format!("confidence-badge {}", decision.confidence_level)}>
                                            {format!("{}% confident", (decision.confidence * 100.0) as i32)}
                                        </span>
                                        <span class="decision-time">
                                            {decision.created_at.format("%b %d, %H:%M").to_string()}
                                        </span>
                                    </div>

                                    {if let Some(email) = email {
                                        html! {
                                            <div class="decision-email-info">
                                                <span class="email-from">{email.from_name.clone().unwrap_or_else(|| email.from_address.clone())}</span>
                                                <span class="email-subject">{&email.subject}</span>
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }}

                                    <div class="decision-proposed">
                                        <span class="proposed-label">{"Proposed: "}</span>
                                        <span class="proposed-action">{&decision.decision_type}</span>
                                        {if let Some(action) = &proposed {
                                            html! {
                                                <span class="proposed-title">{format!(" - \"{}\"", action.todo_title)}</span>
                                            }
                                        } else {
                                            html! {}
                                        }}
                                    </div>

                                    <div class="decision-reasoning">
                                        {&decision.reasoning}
                                    </div>
                                </div>
                                <div class="decision-actions">
                                    <button class="btn-approve" onclick={Callback::from(move |e: MouseEvent| {
                                        e.stop_propagation();
                                        approve.emit(decision_id);
                                    })}>{"Approve"}</button>
                                    <button class="btn-reject" onclick={Callback::from(move |e: MouseEvent| {
                                        e.stop_propagation();
                                        reject.emit((decision_id, None));
                                    })}>{"Reject"}</button>
                                </div>
                            </div>
                        }
                    }).collect::<Html>()
                }}
            </div>

            // Decision detail modal
            {if let Some(decision) = &*selected_decision {
                let close_modal = {
                    let selected_decision = selected_decision.clone();
                    Callback::from(move |_| selected_decision.set(None))
                };
                let decision_for_approve = decision.clone();
                let decision_for_reject = decision.clone();
                let approve = on_approve.clone();
                let reject = on_reject.clone();

                html! {
                    <div class="modal-overlay" onclick={close_modal.clone()}>
                        <div class="modal-content" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                            <div class="modal-header">
                                <h3>{"Decision Details"}</h3>
                                <button class="modal-close" onclick={close_modal}>{"x"}</button>
                            </div>
                            <div class="modal-body">
                                <DecisionDetailView decision={decision.clone()} />
                            </div>
                            <div class="modal-footer">
                                <button class="btn-approve" onclick={Callback::from(move |_| approve.emit(decision_for_approve.id))}>
                                    {"Approve Decision"}
                                </button>
                                <button class="btn-reject" onclick={Callback::from(move |_| reject.emit((decision_for_reject.id, None)))}>
                                    {"Reject Decision"}
                                </button>
                            </div>
                        </div>
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}

// ============================================================================
// Decision Detail View Component
// ============================================================================

#[derive(Properties, PartialEq, Clone)]
struct DecisionDetailProps {
    decision: AgentDecisionResponse,
}

#[function_component(DecisionDetailView)]
fn decision_detail_view(props: &DecisionDetailProps) -> Html {
    let decision = &props.decision;
    let proposed: Option<ProposedTodoAction> =
        serde_json::from_value(decision.proposed_action.clone()).ok();

    html! {
        <div class="decision-detail">
            <div class="detail-section">
                <h4>{"Source"}</h4>
                <p>{format!("Type: {}", decision.source_type)}</p>
                {if let Some(ext_id) = &decision.source_external_id {
                    html! { <p>{format!("External ID: {}", ext_id)}</p> }
                } else {
                    html! {}
                }}
            </div>

            <div class="detail-section">
                <h4>{"Decision"}</h4>
                <p>{format!("Type: {}", decision.decision_type)}</p>
                <p>{format!("Status: {}", decision.status)}</p>
                <p class={format!("confidence {}", decision.confidence_level)}>
                    {format!("Confidence: {}% ({})", (decision.confidence * 100.0) as i32, decision.confidence_level)}
                </p>
            </div>

            {if let Some(action) = proposed {
                html! {
                    <div class="detail-section">
                        <h4>{"Proposed Todo"}</h4>
                        <p><strong>{"Title: "}</strong>{&action.todo_title}</p>
                        {if let Some(desc) = &action.todo_description {
                            html! { <p><strong>{"Description: "}</strong>{desc}</p> }
                        } else {
                            html! {}
                        }}
                        {if let Some(due) = &action.due_date {
                            html! { <p><strong>{"Due Date: "}</strong>{due.format("%Y-%m-%d %H:%M").to_string()}</p> }
                        } else {
                            html! {}
                        }}
                        {if let Some(priority) = &action.priority {
                            html! { <p><strong>{"Priority: "}</strong>{priority}</p> }
                        } else {
                            html! {}
                        }}
                    </div>
                }
            } else {
                html! {}
            }}

            <div class="detail-section">
                <h4>{"Reasoning"}</h4>
                <p class="reasoning-text">{&decision.reasoning}</p>
            </div>

            {if let Some(details) = &decision.reasoning_details {
                html! {
                    <div class="detail-section">
                        <h4>{"Analysis Details"}</h4>
                        <pre class="reasoning-details">{serde_json::to_string_pretty(details).unwrap_or_default()}</pre>
                    </div>
                }
            } else {
                html! {}
            }}

            <div class="detail-section">
                <h4>{"Timeline"}</h4>
                <p>{format!("Created: {}", decision.created_at.format("%Y-%m-%d %H:%M:%S"))}</p>
                {if let Some(reviewed) = decision.reviewed_at {
                    html! { <p>{format!("Reviewed: {}", reviewed.format("%Y-%m-%d %H:%M:%S"))}</p> }
                } else {
                    html! {}
                }}
                {if let Some(executed) = decision.executed_at {
                    html! { <p>{format!("Executed: {}", executed.format("%Y-%m-%d %H:%M:%S"))}</p> }
                } else {
                    html! {}
                }}
            </div>
        </div>
    }
}

// ============================================================================
// Decision Log Component - Shows all historical decisions
// ============================================================================

#[function_component(DecisionLog)]
fn decision_log() -> Html {
    let decisions = use_state(Vec::<AgentDecisionResponse>::new);
    let stats = use_state(|| None::<DecisionStats>);
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    let filter = use_state(|| "all".to_string());

    {
        let decisions = decisions.clone();
        let stats = stats.clone();
        let loading = loading.clone();
        let error = error.clone();
        let filter = (*filter).clone();

        use_effect_with(filter.clone(), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                // Fetch stats
                if let Ok(response) = Request::get("/api/decisions/stats").send().await {
                    if response.ok() {
                        if let Ok(s) = response.json::<DecisionStats>().await {
                            stats.set(Some(s));
                        }
                    }
                }

                // Fetch decisions
                let url = if filter == "all" {
                    "/api/decisions".to_string()
                } else {
                    format!("/api/decisions?status={}", filter)
                };

                match Request::get(&url).send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<Vec<AgentDecisionResponse>>().await {
                                Ok(data) => {
                                    decisions.set(data);
                                    loading.set(false);
                                }
                                Err(e) => {
                                    error.set(Some(format!("Failed to parse decisions: {}", e)));
                                    loading.set(false);
                                }
                            }
                        } else {
                            error.set(Some(format!("API error: {}", response.status())));
                            loading.set(false);
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                        loading.set(false);
                    }
                }
            });
            || ()
        });
    }

    let on_filter_change = {
        let filter = filter.clone();
        Callback::from(move |e: Event| {
            let target: HtmlInputElement = e.target_unchecked_into();
            filter.set(target.value());
        })
    };

    if *loading {
        return html! {
            <div class="decision-log">
                <h2>{"Decision Log"}</h2>
                <p class="loading">{"Loading decisions..."}</p>
            </div>
        };
    }

    if let Some(err) = &*error {
        return html! {
            <div class="decision-log">
                <h2>{"Decision Log"}</h2>
                <p class="error">{err}</p>
            </div>
        };
    }

    let decisions_list = (*decisions).clone();

    html! {
        <div class="decision-log">
            <h2>{"Decision Log"}</h2>

            // Stats overview
            {if let Some(s) = &*stats {
                html! {
                    <div class="stats-overview">
                        <div class="stat-card">
                            <span class="stat-value">{s.total}</span>
                            <span class="stat-label">{"Total"}</span>
                        </div>
                        <div class="stat-card pending">
                            <span class="stat-value">{s.pending}</span>
                            <span class="stat-label">{"Pending"}</span>
                        </div>
                        <div class="stat-card approved">
                            <span class="stat-value">{s.approved}</span>
                            <span class="stat-label">{"Approved"}</span>
                        </div>
                        <div class="stat-card rejected">
                            <span class="stat-value">{s.rejected}</span>
                            <span class="stat-label">{"Rejected"}</span>
                        </div>
                        <div class="stat-card auto">
                            <span class="stat-value">{s.auto_approved}</span>
                            <span class="stat-label">{"Auto-approved"}</span>
                        </div>
                        <div class="stat-card confidence">
                            <span class="stat-value">{format!("{:.0}%", s.average_confidence * 100.0)}</span>
                            <span class="stat-label">{"Avg Confidence"}</span>
                        </div>
                    </div>
                }
            } else {
                html! {}
            }}

            // Filter
            <div class="log-filter">
                <label>{"Filter by status: "}</label>
                <select onchange={on_filter_change} value={(*filter).clone()}>
                    <option value="all">{"All"}</option>
                    <option value="proposed">{"Pending"}</option>
                    <option value="approved">{"Approved"}</option>
                    <option value="rejected">{"Rejected"}</option>
                    <option value="auto_approved">{"Auto-approved"}</option>
                    <option value="executed">{"Executed"}</option>
                </select>
            </div>

            <p class="decision-count">{format!("{} decisions", decisions_list.len())}</p>

            <div class="log-list">
                {if decisions_list.is_empty() {
                    html! { <p class="empty-state">{"No decisions found."}</p> }
                } else {
                    decisions_list.iter().map(|decision| {
                        let status_class = match decision.status.as_str() {
                            "proposed" => "status-pending",
                            "approved" | "executed" => "status-approved",
                            "rejected" => "status-rejected",
                            "auto_approved" => "status-auto",
                            _ => "",
                        };

                        html! {
                            <div key={decision.id.to_string()} class={format!("log-item {}", status_class)}>
                                <div class="log-header">
                                    <span class={format!("status-badge {}", status_class)}>{&decision.status}</span>
                                    <span class="log-type">{&decision.decision_type}</span>
                                    <span class="log-source">{format!("via {}", decision.source_type)}</span>
                                    <span class="log-time">{decision.created_at.format("%b %d, %H:%M").to_string()}</span>
                                </div>
                                <div class="log-reasoning">{&decision.reasoning}</div>
                                {if let Some(feedback) = &decision.user_feedback {
                                    html! { <div class="log-feedback">{format!("Feedback: {}", feedback)}</div> }
                                } else {
                                    html! {}
                                }}
                            </div>
                        }
                    }).collect::<Html>()
                }}
            </div>
        </div>
    }
}

// ============================================================================
// Todo List Component (existing, kept with minimal changes)
// ============================================================================

#[function_component(TodoList)]
fn todo_list() -> Html {
    let todos = use_state(Vec::<Todo>::new);
    let categories = use_state(Vec::<Category>::new);
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    let new_title = use_state(String::new);
    let refresh_trigger = use_state(|| 0u32);

    // Fetch todos and categories
    {
        let todos = todos.clone();
        let categories = categories.clone();
        let loading = loading.clone();
        let error = error.clone();
        let refresh_trigger = *refresh_trigger;

        use_effect_with(refresh_trigger, move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                // Fetch todos
                match Request::get("/api/todos").send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<Vec<Todo>>().await {
                                Ok(data) => todos.set(data),
                                Err(e) => {
                                    error.set(Some(format!("Failed to parse todos: {}", e)));
                                }
                            }
                        } else {
                            error.set(Some(format!("API error: {}", response.status())));
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                    }
                }

                // Fetch categories
                if let Ok(response) = Request::get("/api/categories").send().await {
                    if response.ok() {
                        if let Ok(data) = response.json::<Vec<Category>>().await {
                            categories.set(data);
                        }
                    }
                }

                loading.set(false);
            });
            || ()
        });
    }

    let on_title_input = {
        let new_title = new_title.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            new_title.set(input.value());
        })
    };

    let on_create = {
        let new_title = new_title.clone();
        let refresh_trigger = refresh_trigger.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let title = (*new_title).clone();
            if title.is_empty() {
                return;
            }
            let new_title = new_title.clone();
            let refresh_trigger = refresh_trigger.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let request = CreateTodoRequest {
                    title,
                    description: None,
                    due_date: None,
                    link: None,
                    category_id: None,
                };
                if let Ok(response) = Request::post("/api/todos")
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&request).unwrap())
                    .unwrap()
                    .send()
                    .await
                {
                    if response.ok() {
                        new_title.set(String::new());
                        refresh_trigger.set(*refresh_trigger + 1);
                    }
                }
            });
        })
    };

    let toggle_complete = {
        let refresh_trigger = refresh_trigger.clone();
        Callback::from(move |(id, completed): (Uuid, bool)| {
            let refresh_trigger = refresh_trigger.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let request = UpdateTodoRequest {
                    title: None,
                    description: None,
                    completed: Some(!completed),
                    due_date: None,
                    link: None,
                    category_id: None,
                };
                if let Ok(response) = Request::put(&format!("/api/todos/{}", id))
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&request).unwrap())
                    .unwrap()
                    .send()
                    .await
                {
                    if response.ok() {
                        refresh_trigger.set(*refresh_trigger + 1);
                    }
                }
            });
        })
    };

    let delete_todo = {
        let refresh_trigger = refresh_trigger.clone();
        Callback::from(move |id: Uuid| {
            let refresh_trigger = refresh_trigger.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(response) = Request::delete(&format!("/api/todos/{}", id)).send().await {
                    if response.ok() {
                        refresh_trigger.set(*refresh_trigger + 1);
                    }
                }
            });
        })
    };

    if *loading {
        return html! {
            <div class="todo-container">
                <h2>{"Todos"}</h2>
                <p class="loading">{"Loading todos..."}</p>
            </div>
        };
    }

    if let Some(err) = &*error {
        return html! {
            <div class="todo-container">
                <h2>{"Todos"}</h2>
                <p class="error">{err}</p>
            </div>
        };
    }

    // Sort todos by due date (nulls last), then by completed status
    let mut sorted_todos = (*todos).clone();
    sorted_todos.sort_by(|a, b| match (a.completed, b.completed) {
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        _ => match (&a.due_date, &b.due_date) {
            (Some(date_a), Some(date_b)) => date_a.cmp(date_b),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        },
    });

    let categories_list = (*categories).clone();

    html! {
        <div class="todo-container">
            <h2>{"Todos"}</h2>
            <form class="todo-form" onsubmit={on_create}>
                <input
                    type="text"
                    placeholder="Add a new todo..."
                    value={(*new_title).clone()}
                    oninput={on_title_input}
                />
                <button type="submit">{"Add"}</button>
            </form>
            <p class="todo-count">{format!("{} todos", sorted_todos.len())}</p>
            {if sorted_todos.is_empty() {
                html! { <p class="empty-state">{"No todos yet! Add one above or approve some decisions."}</p> }
            } else {
                html! {
                    <table class="todo-table">
                        <thead>
                            <tr>
                                <th class="col-done">{"Done"}</th>
                                <th class="col-title">{"Title"}</th>
                                <th class="col-due">{"Due Date"}</th>
                                <th class="col-link">{"Link"}</th>
                                <th class="col-actions">{"Actions"}</th>
                            </tr>
                        </thead>
                        <tbody>
                            {sorted_todos.iter().map(|todo| {
                                let category = todo.category_id
                                    .and_then(|cat_id| categories_list.iter().find(|c| c.id == cat_id));
                                let todo_id = todo.id;
                                let todo_completed = todo.completed;
                                let toggle = toggle_complete.clone();
                                let delete = delete_todo.clone();

                                html! {
                                    <tr key={todo.id.to_string()} class={if todo.completed { "completed" } else { "" }}>
                                        <td class="col-done">
                                            <input
                                                type="checkbox"
                                                checked={todo.completed}
                                                onchange={Callback::from(move |_| toggle.emit((todo_id, todo_completed)))}
                                            />
                                        </td>
                                        <td class="col-title">
                                            <span class={if todo.completed { "strikethrough" } else { "" }}>{&todo.title}</span>
                                            {if let Some(desc) = &todo.description {
                                                html! { <span class="todo-description">{" - "}{desc}</span> }
                                            } else {
                                                html! {}
                                            }}
                                            {if let Some(cat) = category {
                                                html! {
                                                    <span class="category-badge" style={format!("background-color: {}", cat.color.as_deref().unwrap_or("#cccccc"))}>
                                                        {&cat.name}
                                                    </span>
                                                }
                                            } else {
                                                html! {}
                                            }}
                                            {if todo.decision_id.is_some() {
                                                html! { <span class="source-badge">{"From Agent"}</span> }
                                            } else {
                                                html! {}
                                            }}
                                        </td>
                                        <td class="col-due">
                                            {if let Some(due) = &todo.due_date {
                                                html! { {due.format("%Y-%m-%d %H:%M").to_string()} }
                                            } else {
                                                html! { <span class="empty-cell">{"-"}</span> }
                                            }}
                                        </td>
                                        <td class="col-link">
                                            {if let Some(link) = &todo.link {
                                                html! { <a href={link.clone()} target="_blank">{"View"}</a> }
                                            } else {
                                                html! { <span class="empty-cell">{"-"}</span> }
                                            }}
                                        </td>
                                        <td class="col-actions">
                                            <button class="delete-btn" onclick={Callback::from(move |_| delete.emit(todo_id))}>{"Delete"}</button>
                                        </td>
                                    </tr>
                                }
                            }).collect::<Html>()}
                        </tbody>
                    </table>
                }
            }}
        </div>
    }
}

// ============================================================================
// Category Manager Component (existing)
// ============================================================================

#[function_component(CategoryManager)]
fn category_manager() -> Html {
    let categories = use_state(Vec::<Category>::new);
    let loading = use_state(|| true);
    let new_name = use_state(String::new);
    let new_color = use_state(|| "#3498db".to_string());
    let refresh_trigger = use_state(|| 0u32);

    // Fetch categories
    {
        let categories = categories.clone();
        let loading = loading.clone();
        let refresh_trigger = *refresh_trigger;

        use_effect_with(refresh_trigger, move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(response) = Request::get("/api/categories").send().await {
                    if response.ok() {
                        if let Ok(data) = response.json::<Vec<Category>>().await {
                            categories.set(data);
                        }
                    }
                }
                loading.set(false);
            });
            || ()
        });
    }

    let on_name_input = {
        let new_name = new_name.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            new_name.set(input.value());
        })
    };

    let on_color_input = {
        let new_color = new_color.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            new_color.set(input.value());
        })
    };

    let on_create = {
        let new_name = new_name.clone();
        let new_color = new_color.clone();
        let refresh_trigger = refresh_trigger.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let name = (*new_name).clone();
            if name.is_empty() {
                return;
            }
            let color = (*new_color).clone();
            let new_name = new_name.clone();
            let refresh_trigger = refresh_trigger.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let body = serde_json::json!({
                    "name": name,
                    "color": color
                });
                if let Ok(response) = Request::post("/api/categories")
                    .header("Content-Type", "application/json")
                    .body(body.to_string())
                    .unwrap()
                    .send()
                    .await
                {
                    if response.ok() {
                        new_name.set(String::new());
                        refresh_trigger.set(*refresh_trigger + 1);
                    }
                }
            });
        })
    };

    let delete_category = {
        let refresh_trigger = refresh_trigger.clone();
        Callback::from(move |id: Uuid| {
            let refresh_trigger = refresh_trigger.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(response) = Request::delete(&format!("/api/categories/{}", id))
                    .send()
                    .await
                {
                    if response.ok() {
                        refresh_trigger.set(*refresh_trigger + 1);
                    }
                }
            });
        })
    };

    if *loading {
        return html! {
            <div class="category-manager">
                <h2>{"Manage Categories"}</h2>
                <p class="loading">{"Loading categories..."}</p>
            </div>
        };
    }

    let categories_list = (*categories).clone();

    html! {
        <div class="category-manager">
            <h2>{"Manage Categories"}</h2>
            <div class="add-category">
                <form onsubmit={on_create}>
                    <input
                        type="text"
                        placeholder="Category name"
                        value={(*new_name).clone()}
                        oninput={on_name_input}
                    />
                    <input
                        type="color"
                        value={(*new_color).clone()}
                        oninput={on_color_input}
                    />
                    <button type="submit">{"Add Category"}</button>
                </form>
            </div>
            <div class="category-list">
                {if categories_list.is_empty() {
                    html! { <p class="empty-state">{"No categories yet!"}</p> }
                } else {
                    categories_list.iter().map(|category| {
                        let cat_id = category.id;
                        let delete = delete_category.clone();
                        html! {
                            <div key={category.id.to_string()} class="category-item">
                                <div
                                    class="category-color"
                                    style={format!("background-color: {}", category.color.as_deref().unwrap_or("#cccccc"))}
                                />
                                <span class="category-name">{&category.name}</span>
                                <button class="delete-btn" onclick={Callback::from(move |_| delete.emit(cat_id))}>{"Delete"}</button>
                            </div>
                        }
                    }).collect::<Html>()
                }}
            </div>
        </div>
    }
}

// ============================================================================
// Calendar View Component
// ============================================================================

#[derive(Clone, PartialEq)]
enum CalendarTab {
    Today,
    Week,
    All,
}

#[function_component(CalendarView)]
fn calendar_view() -> Html {
    let events = use_state(Vec::<CalendarEventResponse>::new);
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    let current_tab = use_state(|| CalendarTab::Week);

    // Fetch events based on current tab
    {
        let events = events.clone();
        let loading = loading.clone();
        let error = error.clone();
        let tab = (*current_tab).clone();
        use_effect_with(tab, move |tab| {
            let events = events.clone();
            let loading = loading.clone();
            let error = error.clone();
            let tab = tab.clone();
            wasm_bindgen_futures::spawn_local(async move {
                loading.set(true);
                error.set(None);

                let endpoint = match tab {
                    CalendarTab::Today => "/api/calendar-events/today",
                    CalendarTab::Week => "/api/calendar-events/week",
                    CalendarTab::All => "/api/calendar-events?limit=100",
                };

                match Request::get(endpoint).send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<Vec<CalendarEventResponse>>().await {
                                Ok(data) => {
                                    events.set(data);
                                }
                                Err(e) => {
                                    error.set(Some(format!("Failed to parse events: {}", e)));
                                }
                            }
                        } else {
                            error.set(Some(format!("Server error: {}", response.status())));
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Failed to fetch events: {}", e)));
                    }
                }
                loading.set(false);
            });
            || ()
        });
    }

    let set_tab_today = {
        let current_tab = current_tab.clone();
        Callback::from(move |_| current_tab.set(CalendarTab::Today))
    };

    let set_tab_week = {
        let current_tab = current_tab.clone();
        Callback::from(move |_| current_tab.set(CalendarTab::Week))
    };

    let set_tab_all = {
        let current_tab = current_tab.clone();
        Callback::from(move |_| current_tab.set(CalendarTab::All))
    };

    let events_list = (*events).clone();

    html! {
        <div class="calendar-view">
            <div class="calendar-header">
                <h2>{"Calendar Events"}</h2>
                <div class="calendar-tabs">
                    <button
                        class={if *current_tab == CalendarTab::Today { "tab-btn active" } else { "tab-btn" }}
                        onclick={set_tab_today}
                    >
                        {"Today"}
                    </button>
                    <button
                        class={if *current_tab == CalendarTab::Week { "tab-btn active" } else { "tab-btn" }}
                        onclick={set_tab_week}
                    >
                        {"This Week"}
                    </button>
                    <button
                        class={if *current_tab == CalendarTab::All { "tab-btn active" } else { "tab-btn" }}
                        onclick={set_tab_all}
                    >
                        {"All Events"}
                    </button>
                </div>
            </div>

            {if *loading {
                html! { <div class="loading">{"Loading events..."}</div> }
            } else if let Some(err) = (*error).as_ref() {
                html! { <div class="error">{err}</div> }
            } else if events_list.is_empty() {
                html! {
                    <div class="empty-state">
                        <p>{"No calendar events found."}</p>
                        <p class="hint">{"Connect a calendar account to see events here."}</p>
                    </div>
                }
            } else {
                html! {
                    <div class="calendar-events-list">
                        {events_list.iter().map(|event| {
                            html! {
                                <CalendarEventCard event={event.clone()} />
                            }
                        }).collect::<Html>()}
                    </div>
                }
            }}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct CalendarEventCardProps {
    event: CalendarEventResponse,
}

#[function_component(CalendarEventCard)]
fn calendar_event_card(props: &CalendarEventCardProps) -> Html {
    let event = &props.event;

    // Format date and time
    let start_date = event.start_time.format("%a, %b %d").to_string();
    let start_time = event.start_time.format("%H:%M").to_string();
    let end_time = event.end_time.format("%H:%M").to_string();

    let time_display = if event.all_day {
        "All Day".to_string()
    } else {
        format!("{} - {}", start_time, end_time)
    };

    let status_class = match event.status.as_str() {
        "confirmed" => "status-confirmed",
        "tentative" => "status-tentative",
        "cancelled" => "status-cancelled",
        _ => "status-default",
    };

    html! {
        <div class={format!("calendar-event-card {}", status_class)}>
            <div class="event-datetime">
                <div class="event-date">{start_date}</div>
                <div class="event-time">{time_display}</div>
            </div>
            <div class="event-details">
                <h3 class="event-title">{event.summary.as_deref().unwrap_or("(No title)")}</h3>
                {if let Some(location) = &event.location {
                    html! { <div class="event-location">{"📍 "}{location}</div> }
                } else {
                    html! {}
                }}
                {if let Some(link) = &event.conference_link {
                    html! {
                        <div class="event-conference">
                            <a href={link.clone()} target="_blank" rel="noopener">{"🔗 Join video call"}</a>
                        </div>
                    }
                } else {
                    html! {}
                }}
                {if !event.attendees.is_empty() {
                    html! {
                        <div class="event-attendees">
                            <span class="attendees-label">{"Attendees: "}</span>
                            {event.attendees.iter().take(3).map(|a| {
                                let status_icon = match a.response_status.as_deref() {
                                    Some("accepted") => "✓",
                                    Some("declined") => "✗",
                                    Some("tentative") => "?",
                                    _ => "•",
                                };
                                html! {
                                    <span class="attendee" title={a.email.clone()}>
                                        {status_icon}{" "}{a.display_name.as_deref().unwrap_or(&a.email)}
                                    </span>
                                }
                            }).collect::<Html>()}
                            {if event.attendees.len() > 3 {
                                html! { <span class="more-attendees">{format!(" +{} more", event.attendees.len() - 3)}</span> }
                            } else {
                                html! {}
                            }}
                        </div>
                    }
                } else {
                    html! {}
                }}
                {if event.recurring {
                    html! { <span class="event-recurring">{"🔄 Recurring"}</span> }
                } else {
                    html! {}
                }}
            </div>
            <div class="event-status">
                <span class={format!("status-badge {}", status_class)}>{&event.status}</span>
            </div>
        </div>
    }
}

// ============================================================================
// Chat Widget Component
// ============================================================================

#[function_component(ChatWidget)]
fn chat_widget() -> Html {
    let is_open = use_state(|| false);
    let messages = use_state(Vec::<ChatMessageResponse>::new);
    let input_value = use_state(String::new);
    let loading = use_state(|| false);
    let suggested_actions = use_state(Vec::<SuggestedAction>::new);
    let messages_container_ref = use_node_ref();

    // Fetch chat history on mount
    {
        let messages = messages.clone();
        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(response) = Request::get("/api/chat/history?limit=50").send().await {
                    if response.ok() {
                        if let Ok(history) = response.json::<Vec<ChatMessageResponse>>().await {
                            messages.set(history);
                        }
                    }
                }
            });
            || ()
        });
    }

    // Auto-scroll to bottom when messages change
    {
        let messages_container_ref = messages_container_ref.clone();
        let messages_len = messages.len();
        use_effect_with(messages_len, move |_| {
            if let Some(container) = messages_container_ref.cast::<Element>() {
                container.set_scroll_top(container.scroll_height());
            }
            || ()
        });
    }

    let toggle_open = {
        let is_open = is_open.clone();
        Callback::from(move |_| {
            is_open.set(!*is_open);
        })
    };

    let on_input = {
        let input_value = input_value.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            input_value.set(input.value());
        })
    };

    let on_submit = {
        let input_value = input_value.clone();
        let messages = messages.clone();
        let loading = loading.clone();
        let suggested_actions = suggested_actions.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let content = (*input_value).clone();
            if content.trim().is_empty() {
                return;
            }

            let input_value = input_value.clone();
            let messages = messages.clone();
            let loading = loading.clone();
            let suggested_actions = suggested_actions.clone();

            // Add user message to UI immediately
            let user_msg = ChatMessageResponse {
                id: Uuid::new_v4(),
                role: "user".to_string(),
                content: content.clone(),
                intent: None,
                created_at: chrono::Utc::now(),
            };
            let mut current_msgs = (*messages).clone();
            current_msgs.push(user_msg);
            messages.set(current_msgs);
            input_value.set(String::new());
            loading.set(true);

            wasm_bindgen_futures::spawn_local(async move {
                let request = SendChatMessageRequest {
                    content: content.clone(),
                };

                match Request::post("/api/chat")
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&request).unwrap())
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(chat_response) = response.json::<ChatResponse>().await {
                                // Update messages with the actual server response
                                let mut current_msgs = (*messages).clone();
                                // Remove the temporary user message (last one) and add the real one
                                current_msgs.pop();
                                // Re-fetch to get properly persisted messages
                                if let Ok(history_resp) =
                                    Request::get("/api/chat/history?limit=50").send().await
                                {
                                    if history_resp.ok() {
                                        if let Ok(history) =
                                            history_resp.json::<Vec<ChatMessageResponse>>().await
                                        {
                                            messages.set(history);
                                        }
                                    }
                                }
                                // Update suggested actions
                                suggested_actions.set(chat_response.suggested_actions);
                            }
                        }
                    }
                    Err(e) => {
                        web_sys::console::error_1(
                            &format!("Failed to send chat message: {}", e).into(),
                        );
                    }
                }
                loading.set(false);
            });
        })
    };

    let on_action_click = {
        let suggested_actions = suggested_actions.clone();
        Callback::from(move |action: SuggestedAction| {
            let suggested_actions = suggested_actions.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match action.action_type.as_str() {
                    "create_todo" => {
                        // Extract title from payload
                        if let Some(title) = action.payload.get("title").and_then(|v| v.as_str()) {
                            let request = CreateTodoRequest {
                                title: title.to_string(),
                                description: None,
                                due_date: None,
                                link: None,
                                category_id: None,
                            };
                            if let Ok(response) = Request::post("/api/todos")
                                .header("Content-Type", "application/json")
                                .body(serde_json::to_string(&request).unwrap())
                                .unwrap()
                                .send()
                                .await
                            {
                                if response.ok() {
                                    // Clear suggested actions after successful creation
                                    suggested_actions.set(Vec::new());
                                }
                            }
                        }
                    }
                    "navigate" => {
                        // For navigation, we'd need to lift state up or use context
                        // For now, just clear actions
                        suggested_actions.set(Vec::new());
                    }
                    _ => {}
                }
            });
        })
    };

    let clear_history = {
        let messages = messages.clone();
        let suggested_actions = suggested_actions.clone();
        Callback::from(move |_| {
            let messages = messages.clone();
            let suggested_actions = suggested_actions.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(response) = Request::delete("/api/chat/history").send().await {
                    if response.ok() {
                        messages.set(Vec::new());
                        suggested_actions.set(Vec::new());
                    }
                }
            });
        })
    };

    html! {
        <div class={format!("chat-widget {}", if *is_open { "open" } else { "" })}>
            // Chat toggle button
            <button class="chat-toggle" onclick={toggle_open.clone()}>
                {if *is_open {
                    html! { <span class="chat-icon">{"x"}</span> }
                } else {
                    html! { <span class="chat-icon">{"?"}</span> }
                }}
            </button>

            // Chat panel
            {if *is_open {
                let messages_list = (*messages).clone();
                let actions_list = (*suggested_actions).clone();

                html! {
                    <div class="chat-panel">
                        <div class="chat-header">
                            <h3>{"Chat Assistant"}</h3>
                            <button class="chat-clear" onclick={clear_history} title="Clear chat history">
                                {"Clear"}
                            </button>
                        </div>

                        <div class="chat-messages" ref={messages_container_ref.clone()}>
                            {if messages_list.is_empty() {
                                html! {
                                    <div class="chat-welcome">
                                        <p>{"Hello! I can help you:"}</p>
                                        <ul>
                                            <li>{"Create todos: \"Add task to call John\""}</li>
                                            <li>{"View your tasks: \"Show my todos\""}</li>
                                            <li>{"Review decisions: \"Show pending decisions\""}</li>
                                        </ul>
                                        <p>{"Type a message to get started!"}</p>
                                    </div>
                                }
                            } else {
                                messages_list.iter().map(|msg| {
                                    let role_class = if msg.role == "user" { "user" } else { "assistant" };
                                    html! {
                                        <div key={msg.id.to_string()} class={format!("chat-message {}", role_class)}>
                                            <div class="message-content">{&msg.content}</div>
                                            {if let Some(intent) = &msg.intent {
                                                html! {
                                                    <div class="message-intent">
                                                        <span class="intent-badge">{intent}</span>
                                                    </div>
                                                }
                                            } else {
                                                html! {}
                                            }}
                                        </div>
                                    }
                                }).collect::<Html>()
                            }}

                            {if *loading {
                                html! {
                                    <div class="chat-message assistant typing">
                                        <div class="typing-indicator">
                                            <span></span><span></span><span></span>
                                        </div>
                                    </div>
                                }
                            } else {
                                html! {}
                            }}
                        </div>

                        // Suggested actions
                        {if !actions_list.is_empty() {
                            let on_action = on_action_click.clone();
                            html! {
                                <div class="chat-actions">
                                    {actions_list.iter().map(|action| {
                                        let action_clone = action.clone();
                                        let on_click = on_action.clone();
                                        html! {
                                            <button
                                                key={action.label.clone()}
                                                class="chat-action-btn"
                                                onclick={Callback::from(move |_| on_click.emit(action_clone.clone()))}
                                            >
                                                {&action.label}
                                            </button>
                                        }
                                    }).collect::<Html>()}
                                </div>
                            }
                        } else {
                            html! {}
                        }}

                        <form class="chat-input" onsubmit={on_submit}>
                            <input
                                type="text"
                                placeholder="Type a message..."
                                value={(*input_value).clone()}
                                oninput={on_input}
                                disabled={*loading}
                            />
                            <button type="submit" disabled={*loading || input_value.is_empty()}>
                                {"Send"}
                            </button>
                        </form>
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
