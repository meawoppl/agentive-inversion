use yew::prelude::*;
use yew_router::prelude::*;

use crate::pages::{home::Home, sources::Sources, settings::Settings, not_found::NotFound};

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/sources")]
    Sources,
    #[at("/settings")]
    Settings,
    #[not_found]
    #[at("/404")]
    NotFound,
}

pub fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! { <Home /> },
        Route::Sources => html! { <Sources /> },
        Route::Settings => html! { <Settings /> },
        Route::NotFound => html! { <NotFound /> },
    }
}
