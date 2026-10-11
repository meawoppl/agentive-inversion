use gloo_net::http::Request;
use shared_types::LoginInitResponse;
use yew::prelude::*;

#[function_component(LoginPage)]
pub(crate) fn login_page() -> Html {
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
