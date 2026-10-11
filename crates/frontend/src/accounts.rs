use crate::time::format_relative_time;
use gloo_net::http::Request;
use shared_types::GoogleAccountResponse;
use yew::prelude::*;

#[function_component(AccountStatus)]
pub(crate) fn account_status() -> Html {
    let accounts = use_state(Vec::<GoogleAccountResponse>::new);
    let expanded = use_state(|| false);

    // Fetch accounts on mount
    {
        let accounts = accounts.clone();
        use_effect_with((), move |_| {
            let accounts = accounts.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(response) = Request::get("/api/google-accounts").send().await {
                    if response.ok() {
                        if let Ok(data) = response.json::<Vec<GoogleAccountResponse>>().await {
                            accounts.set(data);
                        }
                    }
                }
            });
            || ()
        });
    }

    let toggle_expanded = {
        let expanded = expanded.clone();
        Callback::from(move |_| expanded.set(!*expanded))
    };

    let collapse = {
        let expanded = expanded.clone();
        Callback::from(move |_| expanded.set(false))
    };

    let total_accounts = accounts.len();
    let error_count = accounts
        .iter()
        .filter(|a| a.last_sync_error.is_some())
        .count();

    html! {
        <div class="account-status-container">
            <button class="account-status-btn" onclick={toggle_expanded.clone()}>
                <span class="account-status-icon">{"@"}</span>
                <span class="account-status-count">{total_accounts}</span>
                {if error_count > 0 {
                    html! { <span class="account-status-alert">{error_count}</span> }
                } else {
                    html! {}
                }}
            </button>

            {if *expanded {
                let account_list = (*accounts).clone();
                html! {
                    <>
                    // Invisible full-screen layer under the dropdown: a click
                    // anywhere outside it dismisses the popover
                    <div class="popover-backdrop" onclick={collapse}></div>
                    <div class="account-status-dropdown">
                        <div class="account-section">
                            <div class="account-section-header">
                                <span>{"Connected Accounts"}</span>
                                <span class="account-section-count">{account_list.len()}</span>
                            </div>
                            {if account_list.is_empty() {
                                html! { <div class="account-empty">{"No accounts connected"}</div> }
                            } else {
                                account_list.iter().map(|account| {
                                    let (item_class, dot_class, status_text, status_title) =
                                        if let Some(error) = &account.last_sync_error {
                                            (
                                                "account-item status-error",
                                                "status-dot status-error",
                                                format!("Sync failing ({}x)", account.consecutive_failures),
                                                error.clone(),
                                            )
                                        } else if let Some(synced) = &account.last_synced_at {
                                            (
                                                "account-item status-ok",
                                                "status-dot status-ok",
                                                format!("Synced {}", format_relative_time(synced)),
                                                String::new(),
                                            )
                                        } else {
                                            (
                                                "account-item status-ok",
                                                "status-dot status-syncing",
                                                "Waiting for first sync".to_string(),
                                                String::new(),
                                            )
                                        };
                                    html! {
                                        <div key={account.id.to_string()} class={item_class}>
                                            <div class="account-info">
                                                <span class="account-email">{&account.email}</span>
                                                {if let Some(name) = &account.name {
                                                    html! { <span class="account-name">{name}</span> }
                                                } else {
                                                    html! {}
                                                }}
                                            </div>
                                            <div class="account-status-indicator" title={status_title}>
                                                <span class={dot_class}></span>
                                                <span class="status-text">{status_text}</span>
                                            </div>
                                        </div>
                                    }
                                }).collect::<Html>()
                            }}
                        </div>
                    </div>
                    </>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
