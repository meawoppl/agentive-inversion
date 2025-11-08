use yew::prelude::*;
use shared::api::TodoResponse;

use crate::components::todo_item::TodoItem;

#[derive(Properties, PartialEq)]
pub struct TodoListProps {
    pub todos: Vec<TodoResponse>,
    pub on_toggle: Callback<usize>,
}

#[function_component(TodoList)]
pub fn todo_list(props: &TodoListProps) -> Html {
    if props.todos.is_empty() {
        return html! {
            <div class="empty-state">
                <h2>{ "No todos yet!" }</h2>
                <p>{ "Your todos from Gmail and Calendar will appear here." }</p>
            </div>
        };
    }

    html! {
        <div class="todo-list">
            { for props.todos.iter().enumerate().map(|(idx, todo)| {
                let on_toggle = props.on_toggle.clone();
                let callback = Callback::from(move |_| {
                    on_toggle.emit(idx);
                });

                html! {
                    <TodoItem todo={todo.clone()} on_toggle={callback} />
                }
            })}
        </div>
    }
}
