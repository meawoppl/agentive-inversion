use shared_types::{Category, Todo};
use yew::prelude::*;

#[function_component(App)]
fn app() -> Html {
    let todos = use_state(Vec::<Todo>::new);
    let categories = use_state(Vec::<Category>::new);
    let show_categories = use_state(|| false);

    let toggle_categories = {
        let show_categories = show_categories.clone();
        Callback::from(move |_| {
            show_categories.set(!*show_categories);
        })
    };

    html! {
        <div class="app">
            <header>
                <h1>{"Agentive Inversion - Self-Updating Todo List"}</h1>
                <nav>
                    <button onclick={toggle_categories}>
                        {if *show_categories { "Hide Categories" } else { "Manage Categories" }}
                    </button>
                </nav>
            </header>
            <main>
                {if *show_categories {
                    html! { <CategoryManager categories={(*categories).clone()} /> }
                } else {
                    html! { <TodoList todos={(*todos).clone()} categories={(*categories).clone()} /> }
                }}
            </main>
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
