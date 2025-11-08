use yew::prelude::*;
use yew_router::prelude::*;

use crate::router::Route;

#[function_component(NotFound)]
pub fn not_found() -> Html {
    html! {
        <div class="container">
            <div class="empty-state">
                <h2>{ "404 - Page Not Found" }</h2>
                <p>{ "The page you're looking for doesn't exist." }</p>
                <Link<Route> to={Route::Home}>
                    <button class="btn btn-primary">{ "Go Home" }</button>
                </Link<Route>>
            </div>
        </div>
    }
}
