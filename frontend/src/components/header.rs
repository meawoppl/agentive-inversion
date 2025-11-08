use yew::prelude::*;
use yew_router::prelude::*;

use crate::router::Route;

#[function_component(Header)]
pub fn header() -> Html {
    html! {
        <header class="header">
            <div class="container">
                <h1>{ "Agentive Inversion" }</h1>
                <nav>
                    <Link<Route> to={Route::Home}>{ "Todos" }</Link<Route>>
                    { " | " }
                    <Link<Route> to={Route::Sources}>{ "Sources" }</Link<Route>>
                    { " | " }
                    <Link<Route> to={Route::Settings}>{ "Settings" }</Link<Route>>
                </nav>
            </div>
        </header>
    }
}
