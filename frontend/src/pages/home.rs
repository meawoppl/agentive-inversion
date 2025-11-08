use yew::prelude::*;
use shared::api::TodoResponse;

use crate::components::todo_list::TodoList;
use crate::services::api::ApiService;

#[function_component(Home)]
pub fn home() -> Html {
    let todos = use_state(|| Vec::<TodoResponse>::new());
    let loading = use_state(|| true);

    {
        let todos = todos.clone();
        let loading = loading.clone();

        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                match ApiService::list_todos(None).await {
                    Ok(response) => {
                        todos.set(response.todos);
                        loading.set(false);
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch todos: {:?}", e);
                        loading.set(false);
                    }
                }
            });
            || ()
        });
    }

    let on_toggle = Callback::from(move |_idx: usize| {
        // TODO: Implement todo toggle
        tracing::info!("Todo toggled");
    });

    html! {
        <div class="container">
            <h2>{ "My Todos" }</h2>
            if *loading {
                <div class="loading">
                    <div class="spinner"></div>
                </div>
            } else {
                <TodoList todos={(*todos).clone()} on_toggle={on_toggle} />
            }
        </div>
    }
}
