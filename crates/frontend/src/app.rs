use crate::connector::Console;
use crate::login::LoginPage;
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

    // Preserve pairing across the top-level Google OAuth round trip.
    {
        use_effect_with((), |_| {
            use gloo::storage::{SessionStorage, Storage};
            if let Some(w) = web_sys::window() {
                if let Ok(search) = w.location().search() {
                    if let Some(id) = search
                        .strip_prefix("?connect=")
                        .and_then(|s| uuid::Uuid::parse_str(s).ok())
                    {
                        let _ = SessionStorage::set("agentive-pairing", id.to_string());
                    }
                }
            }
            || ()
        });
    }

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
        AuthState::Authenticated { email, .. } => {
            html! { <Console email={email.clone()} on_logout={on_logout} /> }
        }
    }
}
