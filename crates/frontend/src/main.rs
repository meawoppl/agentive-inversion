use gloo_net::http::Request;
use shared_types::{Category, EmailResponse, Todo};
use yew::prelude::*;

#[derive(Clone, PartialEq)]
enum View {
    Inbox,
    Todos,
    Categories,
}

#[function_component(App)]
fn app() -> Html {
    let todos = use_state(Vec::<Todo>::new);
    let categories = use_state(Vec::<Category>::new);
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
                    View::Todos => html! { <TodoList todos={(*todos).clone()} categories={(*categories).clone()} /> },
                    View::Categories => html! { <CategoryManager categories={(*categories).clone()} /> },
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

#[derive(Properties, PartialEq)]
struct TodoListProps {
    todos: Vec<Todo>,
    categories: Vec<Category>,
}

#[function_component(TodoList)]
fn todo_list(props: &TodoListProps) -> Html {
    // Sort todos by due date (nulls last)
    let mut sorted_todos = props.todos.clone();
    sorted_todos.sort_by(|a, b| match (&a.due_date, &b.due_date) {
        (Some(date_a), Some(date_b)) => date_a.cmp(date_b),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    html! {
        <div class="todo-list">
            {
                if sorted_todos.is_empty() {
                    html! { <p>{"No todos yet!"}</p> }
                } else {
                    sorted_todos.iter().map(|todo| {
                        let category = todo.category_id
                            .and_then(|cat_id| props.categories.iter().find(|c| c.id == cat_id));

                        html! {
                            <div key={todo.id.to_string()} class="todo-item">
                                <h3>{&todo.title}</h3>
                                {
                                    if let Some(desc) = &todo.description {
                                        html! { <p class="description">{desc}</p> }
                                    } else {
                                        html! {}
                                    }
                                }
                                {
                                    if let Some(due) = &todo.due_date {
                                        html! {
                                            <p class="due-date">
                                                {"Due: "}
                                                {due.format("%Y-%m-%d %H:%M").to_string()}
                                            </p>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                                {
                                    if let Some(link) = &todo.link {
                                        html! {
                                            <p class="link">
                                                <a href={link.clone()} target="_blank">{"View Link"}</a>
                                            </p>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                                {
                                    if let Some(cat) = category {
                                        html! {
                                            <span class="category" style={format!("background-color: {}", cat.color.as_deref().unwrap_or("#cccccc"))}>
                                                {&cat.name}
                                            </span>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                            </div>
                        }
                    }).collect::<Html>()
                }
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct CategoryManagerProps {
    categories: Vec<Category>,
}

#[function_component(CategoryManager)]
fn category_manager(props: &CategoryManagerProps) -> Html {
    html! {
        <div class="category-manager">
            <h2>{"Manage Categories"}</h2>
            <div class="category-list">
                {
                    if props.categories.is_empty() {
                        html! { <p>{"No categories yet!"}</p> }
                    } else {
                        props.categories.iter().map(|category| {
                            html! {
                                <div key={category.id.to_string()} class="category-item">
                                    <div
                                        class="category-color"
                                        style={format!("background-color: {}", category.color.as_deref().unwrap_or("#cccccc"))}
                                    />
                                    <span class="category-name">{&category.name}</span>
                                    <button class="delete-btn">{"Delete"}</button>
                                </div>
                            }
                        }).collect::<Html>()
                    }
                }
            </div>
            <div class="add-category">
                <h3>{"Add New Category"}</h3>
                <form>
                    <input type="text" placeholder="Category name" />
                    <input type="color" />
                    <button type="submit">{"Add Category"}</button>
                </form>
            </div>
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
