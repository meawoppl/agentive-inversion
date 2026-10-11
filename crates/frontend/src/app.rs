use crate::about_me::AboutMeEditor;
use crate::accounts::AccountStatus;
use crate::archive_review::ArchiveReviewPanel;
use crate::calendar::CalendarView;
use crate::categories::CategoryManager;
use crate::chat::ChatWidget;
use crate::context::{use_app_context, AppContextProvider, View};
use crate::decisions::{DecisionInbox, DecisionLog};
use crate::login::LoginPage;
use crate::pipeline::PipelineView;
use crate::todos::TodoList;
use gloo_net::http::Request;
use shared_types::AuthUserResponse;
use yew::prelude::*;

/// Authentication state for the app
#[derive(Clone, PartialEq)]
enum AuthState {
    /// Initial state - checking auth status
    Unknown,
    /// User is authenticated
    Authenticated { email: String, name: Option<String> },
    /// User is not authenticated
    Unauthenticated,
}

#[function_component(App)]
pub(crate) fn app() -> Html {
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

#[derive(Properties, PartialEq)]
struct AuthenticatedAppProps {
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

    let set_view_pipeline = {
        let set_view = ctx.set_view.clone();
        Callback::from(move |_| set_view.emit(View::Pipeline))
    };

    let set_view_review = {
        let set_view = ctx.set_view.clone();
        Callback::from(move |_| set_view.emit(View::Review))
    };

    let set_view_about_me = {
        let set_view = ctx.set_view.clone();
        Callback::from(move |_| set_view.emit(View::AboutMe))
    };

    let on_logout = props.on_logout.clone();

    html! {
        <div class="app">
            <header>
                <div class="header-top">
                    <h1>{"Agentive Inversion"}</h1>
                    <div class="header-right">
                        <AccountStatus />
                        <div class="user-menu">
                            <span class="user-email">
                                {props.name.as_ref().unwrap_or(&props.email)}
                            </span>
                            <button class="logout-btn" onclick={Callback::from(move |_| on_logout.emit(()))}>
                                {"Logout"}
                            </button>
                        </div>
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
                        class={if current_view == View::Review { "nav-btn active" } else { "nav-btn" }}
                        onclick={set_view_review}
                    >
                        {"Review"}
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
                    <button
                        class={if current_view == View::Pipeline { "nav-btn active" } else { "nav-btn" }}
                        onclick={set_view_pipeline}
                    >
                        {"Pipeline"}
                    </button>
                    <button
                        class={if current_view == View::AboutMe { "nav-btn active" } else { "nav-btn" }}
                        onclick={set_view_about_me}
                    >
                        {"About Me"}
                    </button>
                </nav>
            </header>
            <main>
                {match current_view {
                    View::Inbox => html! { <DecisionInbox /> },
                    View::Review => html! { <ArchiveReviewPanel /> },
                    View::Todos => html! { <TodoList /> },
                    View::Calendar => html! { <CalendarView /> },
                    View::DecisionLog => html! { <DecisionLog /> },
                    View::Categories => html! { <CategoryManager /> },
                    View::Pipeline => html! { <PipelineView /> },
                    View::AboutMe => html! { <AboutMeEditor /> },
                }}
            </main>
            <ChatWidget />
        </div>
    }
}
