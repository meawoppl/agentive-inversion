mod components;
mod pages;
mod services;
mod router;

use yew::prelude::*;
use yew_router::BrowserRouter;

use crate::router::{switch, Route};

#[function_component(App)]
fn app() -> Html {
    html! {
        <BrowserRouter>
            <div id="app">
                <components::header::Header />
                <yew_router::Switch<Route> render={switch} />
            </div>
        </BrowserRouter>
    }
}

fn main() {
    // Initialize tracing
    tracing_wasm::set_as_global_default();

    yew::Renderer::<App>::new().render();
}
