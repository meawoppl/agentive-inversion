use yew::prelude::*;
use shared::api::TodoResponse;

#[derive(Properties, PartialEq)]
pub struct TodoItemProps {
    pub todo: TodoResponse,
    pub on_toggle: Callback<()>,
}

#[function_component(TodoItem)]
pub fn todo_item(props: &TodoItemProps) -> Html {
    let todo = &props.todo;
    let on_toggle = props.on_toggle.clone();

    let toggle = Callback::from(move |_| {
        on_toggle.emit(());
    });

    html! {
        <div class="todo-item">
            <input
                type="checkbox"
                class="todo-checkbox"
                checked={todo.completed}
                onclick={toggle}
            />
            <div class="todo-content">
                <div class="todo-title">{ &todo.title }</div>
                if let Some(desc) = &todo.description {
                    <div class="todo-description">{ desc }</div>
                }
                <div class="todo-meta">
                    <span class={format!("todo-badge badge-{:?}", todo.source_type).to_lowercase()}>
                        { format!("{:?}", todo.source_type) }
                    </span>
                    <span class={format!("todo-badge badge-priority-{:?}", todo.priority).to_lowercase()}>
                        { format!("{:?}", todo.priority) }
                    </span>
                </div>
            </div>
        </div>
    }
}
