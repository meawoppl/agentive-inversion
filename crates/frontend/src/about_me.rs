use gloo_net::http::Request;
use shared_types::{AboutMeResponse, UpdateAboutMeRequest};
use yew::prelude::*;

/// Editor for the about-me document the action-stage agent reads
#[function_component(AboutMeEditor)]
pub(crate) fn about_me_editor() -> Html {
    let content = use_state(String::new);
    let status = use_state(|| None::<String>);
    let loaded = use_state(|| false);

    {
        let content = content.clone();
        let loaded = loaded.clone();
        use_effect_with((), move |_| {
            let content = content.clone();
            let loaded = loaded.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(resp) = Request::get("/api/about-me").send().await {
                    if resp.ok() {
                        if let Ok(doc) = resp.json::<AboutMeResponse>().await {
                            content.set(doc.content);
                        }
                    }
                }
                loaded.set(true);
            });
            || ()
        });
    }

    let on_input = {
        let content = content.clone();
        Callback::from(move |e: InputEvent| {
            let target: web_sys::HtmlTextAreaElement = e.target_unchecked_into();
            content.set(target.value());
        })
    };

    let on_save = {
        let content = content.clone();
        let status = status.clone();
        Callback::from(move |_| {
            let body = UpdateAboutMeRequest {
                content: (*content).clone(),
            };
            let status = status.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result = Request::put("/api/about-me")
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .expect("serializable body")
                    .send()
                    .await;
                match result {
                    Ok(resp) if resp.ok() => status.set(Some("Saved".to_string())),
                    Ok(resp) => status.set(Some(format!("Save failed: {}", resp.status()))),
                    Err(e) => status.set(Some(format!("Save failed: {e}"))),
                }
            });
        })
    };

    html! {
        <div class="about-me-editor">
            <h2>{"About Me"}</h2>
            <p class="about-me-hint">
                {"The triage agent reads this before deciding what belongs on your todo \
                  list. Describe who you are, active projects, what deserves a todo, \
                  senders who always matter, and what is noise."}
            </p>
            {if *loaded {
                html! {
                    <>
                        <textarea
                            class="about-me-textarea"
                            rows="24"
                            value={(*content).clone()}
                            oninput={on_input}
                        />
                        <div class="about-me-actions">
                            <button class="btn-primary" onclick={on_save}>{"Save"}</button>
                            {if let Some(msg) = (*status).clone() {
                                html! { <span class="about-me-status">{msg}</span> }
                            } else {
                                html! {}
                            }}
                        </div>
                    </>
                }
            } else {
                html! { <p>{"Loading..."}</p> }
            }}
        </div>
    }
}
