use yew::prelude::*;

#[function_component(Sources)]
pub fn sources() -> Html {
    html! {
        <div class="container">
            <h2>{ "Connected Sources" }</h2>
            <p>{ "Manage your Gmail and Calendar integrations here." }</p>
            <div class="empty-state">
                <p>{ "No sources configured yet. Add a Gmail account or Calendar to get started." }</p>
            </div>
        </div>
    }
}
