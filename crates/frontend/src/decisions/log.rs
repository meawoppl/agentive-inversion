use gloo_net::http::Request;
use shared_types::{AgentDecisionResponse, DecisionLogResponse, DecisionStats};
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[function_component(DecisionLog)]
pub(crate) fn decision_log() -> Html {
    let log = use_state(|| None::<DecisionLogResponse>);
    let stats = use_state(|| None::<DecisionStats>);
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    let filter = use_state(|| "all".to_string());
    let page = use_state(|| 0i64);

    {
        let stats = stats.clone();
        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(response) = Request::get("/api/decisions/stats").send().await {
                    if response.ok() {
                        if let Ok(s) = response.json::<DecisionStats>().await {
                            stats.set(Some(s));
                        }
                    }
                }
            });
            || ()
        });
    }

    {
        let log = log.clone();
        let loading = loading.clone();
        let error = error.clone();

        use_effect_with(((*filter).clone(), *page), move |(filter, page)| {
            let url = format!("/api/decisions?status={}&page={}", filter, page);
            wasm_bindgen_futures::spawn_local(async move {
                loading.set(true);
                match Request::get(&url).send().await {
                    Ok(response) if response.ok() => {
                        match response.json::<DecisionLogResponse>().await {
                            Ok(parsed) => {
                                error.set(None);
                                log.set(Some(parsed));
                            }
                            Err(e) => error.set(Some(format!("Failed to parse decisions: {}", e))),
                        }
                    }
                    Ok(response) => error.set(Some(format!("API error: {}", response.status()))),
                    Err(e) => error.set(Some(format!("Network error: {}", e))),
                }
                loading.set(false);
            });
            || ()
        });
    }

    let on_filter_change = {
        let filter = filter.clone();
        let page = page.clone();
        Callback::from(move |e: Event| {
            let target: HtmlInputElement = e.target_unchecked_into();
            page.set(0);
            filter.set(target.value());
        })
    };

    if *loading && log.is_none() {
        return html! {
            <div class="decision-log">
                <h2>{"Decision Log"}</h2>
                <p class="loading">{"Loading decisions..."}</p>
            </div>
        };
    }

    if let Some(err) = &*error {
        return html! {
            <div class="decision-log">
                <h2>{"Decision Log"}</h2>
                <p class="error">{err}</p>
            </div>
        };
    }

    let page_data = (*log).clone().unwrap_or(DecisionLogResponse {
        items: Vec::new(),
        page: 0,
        page_size: 0,
        total: 0,
    });
    let total_pages = if page_data.page_size > 0 {
        (page_data.total + page_data.page_size - 1) / page_data.page_size
    } else {
        0
    };

    let pager = {
        let page_state = page.clone();
        let current = page_data.page;
        let prev = {
            let page_state = page_state.clone();
            Callback::from(move |_| page_state.set((current - 1).max(0)))
        };
        let next = {
            let page_state = page_state.clone();
            Callback::from(move |_| page_state.set(current + 1))
        };
        html! {
            <div class="pager">
                <button class="btn-secondary" disabled={current == 0} onclick={prev}>{"< Prev"}</button>
                <span class="pager-position">{format!("Page {} of {}", current + 1, total_pages.max(1))}</span>
                <button class="btn-secondary" disabled={current + 1 >= total_pages} onclick={next}>{"Next >"}</button>
            </div>
        }
    };

    html! {
        <div class="decision-log">
            <h2>{"Decision Log"}</h2>

            // Stats overview
            {if let Some(s) = &*stats {
                html! {
                    <div class="stats-overview">
                        <div class="stat-card">
                            <span class="stat-value">{s.total}</span>
                            <span class="stat-label">{"Total"}</span>
                        </div>
                        <div class="stat-card pending">
                            <span class="stat-value">{s.pending}</span>
                            <span class="stat-label">{"Pending"}</span>
                        </div>
                        <div class="stat-card approved">
                            <span class="stat-value">{s.approved}</span>
                            <span class="stat-label">{"Approved"}</span>
                        </div>
                        <div class="stat-card rejected">
                            <span class="stat-value">{s.rejected}</span>
                            <span class="stat-label">{"Rejected"}</span>
                        </div>
                        <div class="stat-card auto">
                            <span class="stat-value">{s.auto_approved}</span>
                            <span class="stat-label">{"Auto-approved"}</span>
                        </div>
                        <div class="stat-card confidence">
                            <span class="stat-value">{format!("{:.0}%", s.average_confidence * 100.0)}</span>
                            <span class="stat-label">{"Avg Confidence"}</span>
                        </div>
                    </div>
                }
            } else {
                html! {}
            }}

            // Filter. `selected` per option, not `value` on the select: the
            // latter does not drive which option the browser displays, so the
            // control used to read "Executed" while showing everything.
            <div class="log-filter">
                <label>{"Filter by status: "}</label>
                <select onchange={on_filter_change}>
                    {[
                        ("all", "All"),
                        ("proposed", "Pending"),
                        ("approved", "Approved"),
                        ("rejected", "Rejected"),
                        ("auto_approved", "Auto-approved"),
                        ("executed", "Executed"),
                        ("withdrawn", "Withdrawn"),
                    ].into_iter().map(|(value, label)| html! {
                        <option key={value} value={value} selected={*filter == value}>{label}</option>
                    }).collect::<Html>()}
                </select>
            </div>

            <p class="decision-count">
                {format!(
                    "{} decisions - showing {}",
                    page_data.total,
                    page_data.items.len()
                )}
            </p>

            {pager.clone()}

            <div class="log-list">
                {if page_data.items.is_empty() {
                    html! { <p class="empty-state">{"No decisions found."}</p> }
                } else {
                    // The log is a chronology, so days are the grouping that
                    // makes a page scannable
                    group_by_day(&page_data.items).into_iter().map(|(day, items)| {
                        html! {
                            <div class="log-day" key={day.clone()}>
                                <div class="log-day-header">
                                    <span class="log-day-label">{day}</span>
                                    <span class="log-day-count">{format!("{} on this page", items.len())}</span>
                                </div>
                                {items.into_iter().map(|decision| {
                                    let status_class = match decision.status.as_str() {
                                        "proposed" => "status-pending",
                                        "approved" | "executed" => "status-approved",
                                        "rejected" => "status-rejected",
                                        "auto_approved" => "status-auto",
                                        "withdrawn" => "status-withdrawn",
                                        _ => "",
                                    };

                                    html! {
                                        <div key={decision.id.to_string()} class={format!("log-item {}", status_class)}>
                                            <div class="log-header">
                                                <span class={format!("status-badge {}", status_class)}>{&decision.status}</span>
                                                <span class="log-type">{&decision.decision_type}</span>
                                                <span class="log-source">{format!("via {}", decision.source_type)}</span>
                                                <span class="log-time">{decision.created_at.format("%b %d, %H:%M").to_string()}</span>
                                            </div>
                                            <div class="log-reasoning">{&decision.reasoning}</div>
                                            {if let Some(feedback) = &decision.user_feedback {
                                                html! { <div class="log-feedback">{format!("Feedback: {}", feedback)}</div> }
                                            } else {
                                                html! {}
                                            }}
                                        </div>
                                    }
                                }).collect::<Html>()}
                            </div>
                        }
                    }).collect::<Html>()
                }}
            </div>

            {pager}
        </div>
    }
}

/// Split a newest-first page of decisions into consecutive same-day runs.
/// The API already orders by created_at, so a single pass preserves order.
fn group_by_day(items: &[AgentDecisionResponse]) -> Vec<(String, Vec<AgentDecisionResponse>)> {
    let mut days: Vec<(String, Vec<AgentDecisionResponse>)> = Vec::new();
    for decision in items {
        let day = decision.created_at.format("%A, %B %-d, %Y").to_string();
        match days.last_mut() {
            Some((current, bucket)) if *current == day => bucket.push(decision.clone()),
            _ => days.push((day, vec![decision.clone()])),
        }
    }
    days
}
