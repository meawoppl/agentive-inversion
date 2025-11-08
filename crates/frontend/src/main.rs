use shared_types::Todo;
use yew::prelude::*;

#[function_component(App)]
fn app() -> Html {
    let todos = use_state(|| Vec::<Todo>::new());

    html! {
        <div class="app">
            <header>
                <h1>{"Agentive Inversion - Self-Updating Todo List"}</h1>
            </header>
            <main>
                <TodoList todos={(*todos).clone()} />
            </main>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct TodoListProps {
    todos: Vec<Todo>,
}

#[function_component(TodoList)]
fn todo_list(props: &TodoListProps) -> Html {
    html! {
        <div class="todo-list">
            {
                if props.todos.is_empty() {
                    html! { <p>{"No todos yet!"}</p> }
                } else {
                    props.todos.iter().map(|todo| {
                        html! {
                            <div key={todo.id.to_string()} class="todo-item">
                                <h3>{&todo.title}</h3>
                                {
                                    if let Some(desc) = &todo.description {
                                        html! { <p>{desc}</p> }
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

fn main() {
    yew::Renderer::<App>::new().render();
}
