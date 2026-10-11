use gloo_net::http::Request;
use shared_types::{Category, CreateTodoRequest, Todo, UpdateTodoRequest};
use uuid::Uuid;
use web_sys::HtmlInputElement;
use yew::prelude::*;

/// Sort options for the todo list
#[derive(Clone, PartialEq, Copy)]
enum TodoSortOption {
    DueDate,
    Category,
    Title,
}

#[function_component(TodoList)]
pub(crate) fn todo_list() -> Html {
    let todos = use_state(Vec::<Todo>::new);
    let categories = use_state(Vec::<Category>::new);
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    let new_title = use_state(String::new);
    let refresh_trigger = use_state(|| 0u32);
    let sort_option = use_state(|| TodoSortOption::DueDate);
    let show_completed = use_state(|| false);

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

    let categories_list = (*categories).clone();
    let current_sort = *sort_option;
    let showing_completed = *show_completed;

    // Filter and sort todos
    let mut sorted_todos: Vec<Todo> = (*todos)
        .iter()
        .filter(|t| showing_completed || !t.completed)
        .cloned()
        .collect();
    sorted_todos.sort_by(|a, b| {
        // Always put completed items at the bottom
        match (a.completed, b.completed) {
            (true, false) => return std::cmp::Ordering::Greater,
            (false, true) => return std::cmp::Ordering::Less,
            _ => {}
        }
        // Then sort by selected option
        match current_sort {
            TodoSortOption::DueDate => match (&a.due_date, &b.due_date) {
                (Some(date_a), Some(date_b)) => date_a.cmp(date_b),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            },
            TodoSortOption::Category => {
                let cat_a = a
                    .category_id
                    .and_then(|id| categories_list.iter().find(|c| c.id == id))
                    .map(|c| c.name.as_str());
                let cat_b = b
                    .category_id
                    .and_then(|id| categories_list.iter().find(|c| c.id == id))
                    .map(|c| c.name.as_str());
                match (cat_a, cat_b) {
                    (Some(name_a), Some(name_b)) => name_a.cmp(name_b),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            }
            TodoSortOption::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
        }
    });

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
            <div class="todo-toolbar">
                <p class="todo-count">{format!("{} todos", sorted_todos.len())}</p>
                <label class="show-completed-toggle">
                    <input
                        type="checkbox"
                        checked={showing_completed}
                        onchange={{
                            let show_completed = show_completed.clone();
                            Callback::from(move |_| show_completed.set(!*show_completed))
                        }}
                    />
                    {"Show completed"}
                </label>
                <div class="sort-controls">
                    <span class="sort-label">{"Sort by:"}</span>
                    <button
                        class={if current_sort == TodoSortOption::DueDate { "sort-btn active" } else { "sort-btn" }}
                        onclick={{
                            let sort_option = sort_option.clone();
                            Callback::from(move |_| sort_option.set(TodoSortOption::DueDate))
                        }}
                    >
                        {"Date"}
                    </button>
                    <button
                        class={if current_sort == TodoSortOption::Category { "sort-btn active" } else { "sort-btn" }}
                        onclick={{
                            let sort_option = sort_option.clone();
                            Callback::from(move |_| sort_option.set(TodoSortOption::Category))
                        }}
                    >
                        {"Category"}
                    </button>
                    <button
                        class={if current_sort == TodoSortOption::Title { "sort-btn active" } else { "sort-btn" }}
                        onclick={{
                            let sort_option = sort_option.clone();
                            Callback::from(move |_| sort_option.set(TodoSortOption::Title))
                        }}
                    >
                        {"Title"}
                    </button>
                </div>
            </div>
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
