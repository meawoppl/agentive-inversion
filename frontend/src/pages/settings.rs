use yew::prelude::*;

#[function_component(Settings)]
pub fn settings() -> Html {
    html! {
        <div class="container">
            <h2>{ "Settings" }</h2>
            <p>{ "Configure your preferences and application settings." }</p>
        </div>
    }
}
