use gloo_net::http::Request;
use shared_types::ClaudeAuthStatusResponse;
use web_sys::HtmlInputElement;
use yew::prelude::*;

/// Claude Code login card: start flow, show URL, paste code back
#[function_component(ClaudeAuthCard)]
pub(crate) fn claude_auth_card() -> Html {
    let status = use_state(|| None::<ClaudeAuthStatusResponse>);
    let auth_url = use_state(|| None::<String>);
    let code = use_state(String::new);
    let busy = use_state(|| false);
    let error = use_state(|| None::<String>);

    let refresh_status = {
        let status = status.clone();
        Callback::from(move |_: ()| {
            let status = status.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(resp) = Request::get("/api/claude-auth/status").send().await {
                    if resp.ok() {
                        if let Ok(data) = resp.json::<ClaudeAuthStatusResponse>().await {
                            status.set(Some(data));
                        }
                    }
                }
            });
        })
    };

    {
        let refresh = refresh_status.clone();
        use_effect_with((), move |_| {
            refresh.emit(());
            || ()
        });
    }

    let on_start = {
        let auth_url = auth_url.clone();
        let busy = busy.clone();
        let error = error.clone();
        Callback::from(move |_| {
            let auth_url = auth_url.clone();
            let busy = busy.clone();
            let error = error.clone();
            busy.set(true);
            error.set(None);
            wasm_bindgen_futures::spawn_local(async move {
                match Request::post("/api/claude-auth/start").send().await {
                    Ok(resp) if resp.ok() => {
                        if let Ok(data) = resp.json::<serde_json::Value>().await {
                            if let Some(url) = data.get("auth_url").and_then(|u| u.as_str()) {
                                auth_url.set(Some(url.to_string()));
                            }
                        }
                    }
                    Ok(resp) => error.set(Some(format!("Start failed: {}", resp.status()))),
                    Err(e) => error.set(Some(format!("Start failed: {e}"))),
                }
                busy.set(false);
            });
        })
    };

    let on_code_input = {
        let code = code.clone();
        Callback::from(move |e: InputEvent| {
            let target: HtmlInputElement = e.target_unchecked_into();
            code.set(target.value());
        })
    };

    let on_complete = {
        let code = code.clone();
        let auth_url = auth_url.clone();
        let busy = busy.clone();
        let error = error.clone();
        let refresh = refresh_status.clone();
        Callback::from(move |_| {
            // Catch the classic mispaste (the authorize URL, or URL + code)
            // before it costs a round trip and kills the CLI session. Real
            // codes are ~92 chars and never contain a scheme.
            let pasted = code.trim().to_string();
            if pasted.starts_with("http://") || pasted.starts_with("https://") {
                error.set(Some(
                    "That looks like the URL - paste the code shown on that page instead"
                        .to_string(),
                ));
                return;
            }
            if pasted.len() > 150 {
                error.set(Some(format!(
                    "That's {} characters - the code is ~92. Paste just the code, not the whole page",
                    pasted.len()
                )));
                return;
            }
            let body = serde_json::json!({ "code": pasted });
            let code = code.clone();
            let auth_url = auth_url.clone();
            let busy = busy.clone();
            let error = error.clone();
            let refresh = refresh.clone();
            busy.set(true);
            error.set(None);
            wasm_bindgen_futures::spawn_local(async move {
                let result = Request::post("/api/claude-auth/complete")
                    .header("Content-Type", "application/json")
                    .body(body.to_string())
                    .expect("body")
                    .send()
                    .await;
                match result {
                    Ok(resp) if resp.ok() => {
                        auth_url.set(None);
                        code.set(String::new());
                        refresh.emit(());
                    }
                    Ok(resp) => {
                        // Any failure resets to step one. A rejected code is
                        // terminal for the CLI session (its error screen has
                        // no input field), so the old URL + input would offer
                        // a retry that cannot work.
                        let text = resp.text().await.unwrap_or_default();
                        auth_url.set(None);
                        code.set(String::new());
                        error.set(Some(format!(
                            "Login failed ({}): {} - click Login to start over",
                            resp.status(),
                            text
                        )));
                    }
                    Err(e) => {
                        auth_url.set(None);
                        error.set(Some(format!("Login failed: {e}")));
                    }
                }
                busy.set(false);
            });
        })
    };

    html! {
        <div class="claude-auth-card">
            <h3>{"Claude Code"}</h3>
            {if let Some(err) = (*error).clone() {
                html! { <div class="error-banner">{err}</div> }
            } else {
                html! {}
            }}
            {match ((*status).clone(), (*auth_url).clone()) {
                (_, Some(url)) => html! {
                    <div class="claude-auth-flow">
                        <p>{"1. Open this link and sign in to Claude:"}</p>
                        <p><a href={url.clone()} target="_blank" rel="noopener">{url}</a></p>
                        <p>{"2. Paste the code shown on that page (just the code, not the URL):"}</p>
                        <div class="claude-auth-code-row">
                            <input
                                type="text"
                                placeholder="Paste the ~92-character code"
                                value={(*code).clone()}
                                oninput={on_code_input}
                            />
                            <button class="btn-primary" disabled={*busy || code.is_empty()} onclick={on_complete}>
                                {if *busy { "Completing..." } else { "Complete login" }}
                            </button>
                        </div>
                    </div>
                },
                (Some(st), None) if st.connected => html! {
                    <div class="claude-auth-connected">
                        <span class="status-dot status-ok"></span>
                        <span>
                            {"Connected"}
                            {st.updated_at.map(|t| format!(" (since {})", t.format("%Y-%m-%d %H:%M UTC"))).unwrap_or_default()}
                        </span>
                        <button class="btn-secondary" disabled={*busy} onclick={on_start.clone()}>
                            {"Re-login"}
                        </button>
                    </div>
                },
                _ => html! {
                    <div class="claude-auth-disconnected">
                        <p>{"Not connected. The triage pipeline needs a Claude credential."}</p>
                        <button class="btn-primary" disabled={*busy} onclick={on_start}>
                            {if *busy { "Starting..." } else { "Login to Claude Code" }}
                        </button>
                    </div>
                },
            }}
        </div>
    }
}
