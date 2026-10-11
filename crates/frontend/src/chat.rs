use gloo_net::http::Request;
use shared_types::{
    ChatMessageResponse, ChatResponse, CreateTodoRequest, SendChatMessageRequest, SuggestedAction,
};
use uuid::Uuid;
use web_sys::{Element, HtmlInputElement};
use yew::prelude::*;

#[function_component(ChatWidget)]
pub(crate) fn chat_widget() -> Html {
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
