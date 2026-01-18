use gloo_net::http::Request;
use shared_types::{Category, CreateTodoRequest, EmailResponse, Todo, UpdateTodoRequest};
use uuid::Uuid;
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[derive(Clone, PartialEq)]
enum View {
    Inbox,
    Todos,
    Categories,
}

#[function_component(App)]
fn app() -> Html {
    let current_view = use_state(|| View::Inbox);

    let set_view_inbox = {
        let current_view = current_view.clone();
        Callback::from(move |_| current_view.set(View::Inbox))
    };

    let set_view_todos = {
        let current_view = current_view.clone();
        Callback::from(move |_| current_view.set(View::Todos))
    };

    let set_view_categories = {
        let current_view = current_view.clone();
        Callback::from(move |_| current_view.set(View::Categories))
    };

    html! {
        <div class="app">
            <header>
                <h1>{"Agentive Inversion"}</h1>
                <nav class="main-nav">
                    <button
                        class={if *current_view == View::Inbox { "nav-btn active" } else { "nav-btn" }}
                        onclick={set_view_inbox}
                    >
                        {"Inbox"}
                    </button>
                    <button
                        class={if *current_view == View::Todos { "nav-btn active" } else { "nav-btn" }}
                        onclick={set_view_todos}
                    >
                        {"Todos"}
                    </button>
                    <button
                        class={if *current_view == View::Categories { "nav-btn active" } else { "nav-btn" }}
                        onclick={set_view_categories}
                    >
                        {"Categories"}
                    </button>
                </nav>
            </header>
            <main>
                {match &*current_view {
                    View::Inbox => html! { <EmailInbox /> },
                    View::Todos => html! { <TodoList /> },
                    View::Categories => html! { <CategoryManager /> },
                }}
            </main>
        </div>
    }
}

#[function_component(EmailInbox)]
fn email_inbox() -> Html {
    let emails = use_state(Vec::<EmailResponse>::new);
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);

    {
        let emails = emails.clone();
        let loading = loading.clone();
        let error = error.clone();

        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                match Request::get("/api/emails?limit=50").send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<Vec<EmailResponse>>().await {
                                Ok(data) => {
                                    emails.set(data);
                                    loading.set(false);
                                }
                                Err(e) => {
                                    error.set(Some(format!("Failed to parse emails: {}", e)));
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

    if *loading {
        return html! {
            <div class="email-inbox">
                <h2>{"Inbox"}</h2>
                <p class="loading">{"Loading emails..."}</p>
            </div>
        };
    }

    if let Some(err) = &*error {
        return html! {
            <div class="email-inbox">
                <h2>{"Inbox"}</h2>
                <p class="error">{err}</p>
            </div>
        };
    }

    let emails_list = (*emails).clone();

    html! {
        <div class="email-inbox">
            <h2>{"Inbox"}</h2>
            <p class="email-count">{format!("{} emails", emails_list.len())}</p>
            <div class="email-list">
                {if emails_list.is_empty() {
                    html! { <p class="empty-state">{"No emails found. Connect an email account to get started."}</p> }
                } else {
                    emails_list.iter().map(|email| {
                        let from_display = email.from_name.clone()
                            .unwrap_or_else(|| email.from_address.clone());
                        let received = email.received_at.format("%b %d, %H:%M").to_string();

                        html! {
                            <div key={email.id.to_string()} class={format!("email-item {}", if email.processed { "processed" } else { "" })}>
                                <div class="email-header">
                                    <span class="email-from">{from_display}</span>
                                    <span class="email-date">{received}</span>
                                </div>
                                <div class="email-subject">{&email.subject}</div>
                                {if let Some(snippet) = &email.snippet {
                                    html! { <div class="email-snippet">{snippet}</div> }
                                } else {
                                    html! {}
                                }}
                                <div class="email-badges">
                                    {if email.has_attachments {
                                        html! { <span class="badge attachment">{"Attachment"}</span> }
                                    } else {
                                        html! {}
                                    }}
                                    {if email.processed {
                                        html! { <span class="badge processed">{"Processed"}</span> }
                                    } else {
                                        html! { <span class="badge pending">{"Pending"}</span> }
                                    }}
                                </div>
                            </div>
                        }
                    }).collect::<Html>()
                }}
            </div>
        </div>
    }
}

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
            <div class="todo-list">
                {if sorted_todos.is_empty() {
                    html! { <p class="empty-state">{"No todos yet! Add one above."}</p> }
                } else {
                    sorted_todos.iter().map(|todo| {
                        let category = todo.category_id
                            .and_then(|cat_id| categories_list.iter().find(|c| c.id == cat_id));
                        let todo_id = todo.id;
                        let todo_completed = todo.completed;
                        let toggle = toggle_complete.clone();
                        let delete = delete_todo.clone();

                        html! {
                            <div key={todo.id.to_string()} class={format!("todo-item {}", if todo.completed { "completed" } else { "" })}>
                                <div class="todo-header">
                                    <input
                                        type="checkbox"
                                        checked={todo.completed}
                                        onchange={Callback::from(move |_| toggle.emit((todo_id, todo_completed)))}
                                    />
                                    <h3 class={if todo.completed { "strikethrough" } else { "" }}>{&todo.title}</h3>
                                </div>
                                {if let Some(desc) = &todo.description {
                                    html! { <p class="description">{desc}</p> }
                                } else {
                                    html! {}
                                }}
                                {if let Some(due) = &todo.due_date {
                                    html! {
                                        <p class="due-date">
                                            {"Due: "}
                                            {due.format("%Y-%m-%d %H:%M").to_string()}
                                        </p>
                                    }
                                } else {
                                    html! {}
                                }}
                                {if let Some(link) = &todo.link {
                                    html! {
                                        <p class="link">
                                            <a href={link.clone()} target="_blank">{"View Link"}</a>
                                        </p>
                                    }
                                } else {
                                    html! {}
                                }}
                                <div class="todo-footer">
                                    {if let Some(cat) = category {
                                        html! {
                                            <span class="category-badge" style={format!("background-color: {}", cat.color.as_deref().unwrap_or("#cccccc"))}>
                                                {&cat.name}
                                            </span>
                                        }
                                    } else {
                                        html! {}
                                    }}
                                    <button class="delete-btn" onclick={Callback::from(move |_| delete.emit(todo_id))}>{"Delete"}</button>
                                </div>
                            </div>
                        }
                    }).collect::<Html>()
                }}
            </div>
        </div>
    }
}

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

fn main() {
    yew::Renderer::<App>::new().render();
}
