use gloo_net::http::Request;
use shared_types::DecisionStats;
use yew::prelude::*;

/// View/tab options for the application
#[derive(Clone, PartialEq, Copy)]
pub(crate) enum View {
    Inbox,
    Review,
    Todos,
    Calendar,
    DecisionLog,
    Categories,
    Pipeline,
    AboutMe,
}

/// Shared application state accessible via context
#[derive(Clone, PartialEq)]
pub(crate) struct AppState {
    pub(crate) current_view: View,
    pub(crate) pending_decisions_count: i64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_view: View::Inbox,
            pending_decisions_count: 0,
        }
    }
}

/// Context value providing both state and setter
#[derive(Clone, PartialEq)]
pub(crate) struct AppContext {
    pub(crate) state: AppState,
    pub(crate) set_view: Callback<View>,
    pub(crate) set_pending_count: Callback<i64>,
    pub(crate) refresh_pending_count: Callback<()>,
}

/// Hook to access app context
#[hook]
pub(crate) fn use_app_context() -> AppContext {
    use_context::<AppContext>()
        .expect("AppContext not found - wrap component in AppContextProvider")
}

/// Provider component that wraps children with app state context
#[derive(Properties, PartialEq)]
pub(crate) struct AppContextProviderProps {
    #[prop_or_default]
    pub(crate) children: Html,
}

#[function_component(AppContextProvider)]
pub(crate) fn app_context_provider(props: &AppContextProviderProps) -> Html {
    let current_view = use_state(|| View::Inbox);
    let pending_count = use_state(|| 0i64);

    // Fetch initial pending count
    {
        let pending_count = pending_count.clone();
        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(response) = Request::get("/api/decisions/stats").send().await {
                    if response.ok() {
                        if let Ok(stats) = response.json::<DecisionStats>().await {
                            pending_count.set(stats.pending);
                        }
                    }
                }
            });
            || ()
        });
    }

    let set_view = {
        let current_view = current_view.clone();
        Callback::from(move |view: View| current_view.set(view))
    };

    let set_pending_count = {
        let pending_count = pending_count.clone();
        Callback::from(move |count: i64| pending_count.set(count))
    };

    let refresh_pending_count = {
        let pending_count = pending_count.clone();
        Callback::from(move |_: ()| {
            let pending_count = pending_count.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(response) = Request::get("/api/decisions/stats").send().await {
                    if response.ok() {
                        if let Ok(stats) = response.json::<DecisionStats>().await {
                            pending_count.set(stats.pending);
                        }
                    }
                }
            });
        })
    };

    let context = AppContext {
        state: AppState {
            current_view: *current_view,
            pending_decisions_count: *pending_count,
        },
        set_view,
        set_pending_count,
        refresh_pending_count,
    };

    html! {
        <ContextProvider<AppContext> context={context}>
            { props.children.clone() }
        </ContextProvider<AppContext>>
    }
}
