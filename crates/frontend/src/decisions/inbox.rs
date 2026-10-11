use super::detail::DecisionDetailView;
use super::formatting::humanize_type;
use super::group::DecisionGroupCard;
use crate::context::use_app_context;
use gloo_net::http::Request;
use shared_types::{
    ApproveDecisionRequest, BatchApproveDecisionsRequest, BatchRejectDecisionsRequest,
    PendingDecisionItem, PendingDecisionsResponse, RejectDecisionRequest, RescanEventsResponse,
};
use uuid::Uuid;
use yew::prelude::*;

#[function_component(DecisionInbox)]
pub(crate) fn decision_inbox() -> Html {
    let ctx = use_app_context();
    let data = use_state(|| None::<PendingDecisionsResponse>);
    let page = use_state(|| 0i64);
    let type_filter = use_state(|| "all".to_string());
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    let refresh_trigger = use_state(|| 0u32);
    let expanded = use_state(std::collections::HashSet::<String>::new);
    let selected_decisions = use_state(std::collections::HashSet::<Uuid>::new);
    let selected_decision = use_state(|| None::<PendingDecisionItem>);
    let rescanning = use_state(|| false);
    let rescan_result = use_state(|| None::<RescanEventsResponse>);

    // One request per page: the response already carries each decision's
    // email, so there is no per-row fetch
    {
        let data = data.clone();
        let loading = loading.clone();
        let error = error.clone();
        let selected_decisions = selected_decisions.clone();
        let deps = (*page, (*type_filter).clone(), *refresh_trigger);

        use_effect_with(deps, move |(page, type_filter, _)| {
            let url = format!(
                "/api/decisions/pending?page={}&decision_type={}",
                page, type_filter
            );
            wasm_bindgen_futures::spawn_local(async move {
                loading.set(true);
                match Request::get(&url).send().await {
                    Ok(response) if response.ok() => {
                        match response.json::<PendingDecisionsResponse>().await {
                            Ok(parsed) => {
                                // Selections name rows that just left the page
                                selected_decisions.set(std::collections::HashSet::new());
                                error.set(None);
                                data.set(Some(parsed));
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

    let reload = {
        let refresh_trigger = refresh_trigger.clone();
        Callback::from(move |_: ()| refresh_trigger.set(*refresh_trigger + 1))
    };

    let on_approve = {
        let reload = reload.clone();
        let selected_decision = selected_decision.clone();
        let refresh_pending = ctx.refresh_pending_count.clone();
        Callback::from(move |id: Uuid| {
            let reload = reload.clone();
            let selected_decision = selected_decision.clone();
            let refresh_pending = refresh_pending.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let request = ApproveDecisionRequest {
                    modifications: None,
                };
                if let Ok(response) = Request::post(&format!("/api/decisions/{}/approve", id))
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&request).unwrap())
                    .unwrap()
                    .send()
                    .await
                {
                    if response.ok() {
                        selected_decision.set(None);
                        reload.emit(());
                        refresh_pending.emit(());
                    }
                }
            });
        })
    };

    let on_reject = {
        let reload = reload.clone();
        let selected_decision = selected_decision.clone();
        let refresh_pending = ctx.refresh_pending_count.clone();
        Callback::from(move |(id, feedback): (Uuid, Option<String>)| {
            let reload = reload.clone();
            let selected_decision = selected_decision.clone();
            let refresh_pending = refresh_pending.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let request = RejectDecisionRequest { feedback };
                if let Ok(response) = Request::post(&format!("/api/decisions/{}/reject", id))
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&request).unwrap())
                    .unwrap()
                    .send()
                    .await
                {
                    if response.ok() {
                        selected_decision.set(None);
                        reload.emit(());
                        refresh_pending.emit(());
                    }
                }
            });
        })
    };

    // Batch verdicts take explicit ids so a group's "approve all" and the
    // checkbox selection share one code path
    let batch_approve_ids = {
        let reload = reload.clone();
        let refresh_pending = ctx.refresh_pending_count.clone();
        Callback::from(move |ids: Vec<Uuid>| {
            if ids.is_empty() {
                return;
            }
            let reload = reload.clone();
            let refresh_pending = refresh_pending.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let request = BatchApproveDecisionsRequest { decision_ids: ids };
                if let Ok(response) = Request::post("/api/decisions/batch/approve")
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&request).unwrap())
                    .unwrap()
                    .send()
                    .await
                {
                    if response.ok() {
                        reload.emit(());
                        refresh_pending.emit(());
                    }
                }
            });
        })
    };

    let batch_reject_ids = {
        let reload = reload.clone();
        let refresh_pending = ctx.refresh_pending_count.clone();
        Callback::from(move |ids: Vec<Uuid>| {
            if ids.is_empty() {
                return;
            }
            let reload = reload.clone();
            let refresh_pending = refresh_pending.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let request = BatchRejectDecisionsRequest {
                    decision_ids: ids,
                    feedback: None,
                };
                if let Ok(response) = Request::post("/api/decisions/batch/reject")
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&request).unwrap())
                    .unwrap()
                    .send()
                    .await
                {
                    if response.ok() {
                        reload.emit(());
                        refresh_pending.emit(());
                    }
                }
            });
        })
    };

    // Proposals queued before the submission-path check existed were never
    // screened, and one queued last week may have been accepted since.
    let on_rescan_events = {
        let rescanning = rescanning.clone();
        let rescan_result = rescan_result.clone();
        let reload = reload.clone();
        let refresh_pending = ctx.refresh_pending_count.clone();
        Callback::from(move |_| {
            if *rescanning {
                return;
            }
            let rescanning = rescanning.clone();
            let rescan_result = rescan_result.clone();
            let reload = reload.clone();
            let refresh_pending = refresh_pending.clone();
            wasm_bindgen_futures::spawn_local(async move {
                rescanning.set(true);
                if let Ok(response) = Request::post("/api/decisions/rescan-events").send().await {
                    if response.ok() {
                        if let Ok(result) = response.json::<RescanEventsResponse>().await {
                            rescan_result.set(Some(result));
                            reload.emit(());
                            refresh_pending.emit(());
                        }
                    }
                }
                rescanning.set(false);
            });
        })
    };

    let toggle_selection = {
        let selected_decisions = selected_decisions.clone();
        Callback::from(move |id: Uuid| {
            let mut next = (*selected_decisions).clone();
            if !next.remove(&id) {
                next.insert(id);
            }
            selected_decisions.set(next);
        })
    };

    let toggle_group_selection = {
        let selected_decisions = selected_decisions.clone();
        Callback::from(move |ids: Vec<Uuid>| {
            let mut next = (*selected_decisions).clone();
            if ids.iter().all(|id| next.contains(id)) {
                for id in &ids {
                    next.remove(id);
                }
            } else {
                next.extend(ids);
            }
            selected_decisions.set(next);
        })
    };

    let toggle_group_expanded = {
        let expanded = expanded.clone();
        Callback::from(move |key: String| {
            let mut next = (*expanded).clone();
            if !next.remove(&key) {
                next.insert(key);
            }
            expanded.set(next);
        })
    };

    if *loading && data.is_none() {
        return html! {
            <div class="decision-inbox">
                <h2>{"Inbox - Pending Decisions"}</h2>
                <p class="loading">{"Loading decisions..."}</p>
            </div>
        };
    }

    if let Some(err) = &*error {
        return html! {
            <div class="decision-inbox">
                <h2>{"Inbox - Pending Decisions"}</h2>
                <p class="error">{err}</p>
            </div>
        };
    }

    let Some(page_data) = (*data).clone() else {
        return html! {
            <div class="decision-inbox">
                <h2>{"Inbox - Pending Decisions"}</h2>
                <p class="empty-state">{"No pending decisions. All caught up!"}</p>
            </div>
        };
    };

    let page_ids: Vec<Uuid> = page_data
        .groups
        .iter()
        .flat_map(|g| g.items.iter().map(|i| i.decision.id))
        .collect();
    // The rescan is only worth offering when there is an event backlog to screen
    let queued_events = page_data
        .type_counts
        .iter()
        .find(|c| c.decision_type == "create_calendar_event")
        .map(|c| c.count)
        .unwrap_or(0);
    let selected_count = selected_decisions.len();
    let total_pages = if page_data.page_size > 0 {
        (page_data.total_groups + page_data.page_size - 1) / page_data.page_size
    } else {
        0
    };

    let select_page = {
        let selected_decisions = selected_decisions.clone();
        let page_ids = page_ids.clone();
        Callback::from(move |_| {
            selected_decisions.set(page_ids.iter().copied().collect());
        })
    };
    let clear_selection = {
        let selected_decisions = selected_decisions.clone();
        Callback::from(move |_| selected_decisions.set(std::collections::HashSet::new()))
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
                <span class="pager-position">
                    {format!("Page {} of {}", current + 1, total_pages.max(1))}
                </span>
                <button class="btn-secondary" disabled={current + 1 >= total_pages} onclick={next}>{"Next >"}</button>
            </div>
        }
    };

    let filter_chips = {
        let type_filter_state = type_filter.clone();
        let page_state = page.clone();
        let active = (*type_filter).clone();
        let total: i64 = page_data.type_counts.iter().map(|c| c.count).sum();
        let mut chips: Vec<(String, String, i64)> =
            vec![("all".to_string(), "All".to_string(), total)];
        chips.extend(page_data.type_counts.iter().map(|c| {
            (
                c.decision_type.clone(),
                humanize_type(&c.decision_type),
                c.count,
            )
        }));

        html! {
            <div class="inbox-filters">
                {chips.into_iter().map(|(value, label, count)| {
                    let is_active = active == value;
                    let type_filter_state = type_filter_state.clone();
                    let page_state = page_state.clone();
                    let value_for_click = value.clone();
                    html! {
                        <button
                            key={value}
                            class={if is_active { "filter-chip active" } else { "filter-chip" }}
                            onclick={Callback::from(move |_| {
                                page_state.set(0);
                                type_filter_state.set(value_for_click.clone());
                            })}
                        >
                            {label}
                            <span class="filter-chip-count">{count}</span>
                        </button>
                    }
                }).collect::<Html>()}
            </div>
        }
    };

    html! {
        <div class="decision-inbox">
            <div class="inbox-header">
                <h2>{"Inbox - Pending Decisions"}</h2>
                <p class="decision-count">
                    {format!(
                        "{} decisions in {} groups awaiting review",
                        page_data.total_decisions, page_data.total_groups
                    )}
                </p>
            </div>

            {filter_chips}

            {if queued_events == 0 {
                    html! {}
                } else {
                    html! {
                        <div class="inbox-tools">
                            <button
                                class="btn-secondary"
                                disabled={*rescanning}
                                onclick={on_rescan_events.clone()}
                            >
                                {if *rescanning {
                                    "Checking calendars...".to_string()
                                } else {
                                    format!("Re-check {} event proposal(s) against calendars", queued_events)
                                }}
                            </button>
                            {if let Some(result) = &*rescan_result {
                                html! {
                                    <div class="rescan-result">
                                        <span class="rescan-summary">
                                            {if result.withdrawn == 0 {
                                                format!("Checked {} - none were already on a calendar", result.checked)
                                            } else {
                                                format!(
                                                    "Checked {} - withdrew {} already on a calendar",
                                                    result.checked, result.withdrawn
                                                )
                                            }}
                                            {if result.more_remaining { " (more remaining, run again)" } else { "" }}
                                        </span>
                                        {result.details.iter().map(|w| html! {
                                            <div class="rescan-detail" key={w.decision_id.to_string()}>
                                                {format!("\u{201c}{}\u{201d} \u{2192} {}", w.summary, w.matched)}
                                            </div>
                                        }).collect::<Html>()}
                                    </div>
                                }
                            } else {
                                html! {}
                            }}
                        </div>
                    }
                }}

            {if page_data.groups.is_empty() {
                html! { <p class="empty-state">{"No pending decisions. All caught up!"}</p> }
            } else {
                html! {
                    <>
                        <div class="batch-actions">
                            <button class="btn-secondary" onclick={select_page}>
                                {format!("Select Page ({})", page_ids.len())}
                            </button>
                            <button class="btn-secondary" onclick={clear_selection}>{"Clear Selection"}</button>
                            {if selected_count > 0 {
                                let ids: Vec<Uuid> = selected_decisions.iter().copied().collect();
                                let approve = batch_approve_ids.clone();
                                let reject = batch_reject_ids.clone();
                                let approve_ids = ids.clone();
                                html! {
                                    <>
                                        <button class="btn-approve" onclick={Callback::from(move |_| approve.emit(approve_ids.clone()))}>
                                            {format!("Approve Selected ({})", selected_count)}
                                        </button>
                                        <button class="btn-reject" onclick={Callback::from(move |_| reject.emit(ids.clone()))}>
                                            {format!("Reject Selected ({})", selected_count)}
                                        </button>
                                    </>
                                }
                            } else {
                                html! {}
                            }}
                        </div>

                        {pager.clone()}

                        <div class="decision-groups">
                            {page_data.groups.iter().map(|group| {
                                html! {
                                    <DecisionGroupCard
                                        key={group.key.clone()}
                                        group={group.clone()}
                                        expanded={expanded.contains(&group.key)}
                                        selected={(*selected_decisions).clone()}
                                        on_toggle_expanded={toggle_group_expanded.clone()}
                                        on_toggle_group={toggle_group_selection.clone()}
                                        on_toggle_item={toggle_selection.clone()}
                                        on_approve={on_approve.clone()}
                                        on_reject={on_reject.clone()}
                                        on_approve_many={batch_approve_ids.clone()}
                                        on_reject_many={batch_reject_ids.clone()}
                                        on_open={
                                            let selected_decision = selected_decision.clone();
                                            Callback::from(move |item: PendingDecisionItem| selected_decision.set(Some(item)))
                                        }
                                    />
                                }
                            }).collect::<Html>()}
                        </div>

                        {pager}
                    </>
                }
            }}

            // Decision detail modal
            {if let Some(item) = &*selected_decision {
                let decision = &item.decision;
                let close_modal = {
                    let selected_decision = selected_decision.clone();
                    Callback::from(move |_| selected_decision.set(None))
                };
                let decision_for_approve = decision.clone();
                let decision_for_reject = decision.clone();
                let approve = on_approve.clone();
                let reject = on_reject.clone();

                html! {
                    <div class="modal-overlay" onclick={close_modal.clone()}>
                        <div class="modal-content" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                            <div class="modal-header">
                                <h3>{"Decision Details"}</h3>
                                <button class="modal-close" onclick={close_modal}>{"x"}</button>
                            </div>
                            <div class="modal-body">
                                <DecisionDetailView decision={decision.clone()} email={item.email.clone()} />
                            </div>
                            <div class="modal-footer">
                                <button class="btn-approve" onclick={Callback::from(move |_| approve.emit(decision_for_approve.id))}>
                                    {"Approve Decision"}
                                </button>
                                <button class="btn-reject" onclick={Callback::from(move |_| reject.emit((decision_for_reject.id, None)))}>
                                    {"Reject Decision"}
                                </button>
                            </div>
                        </div>
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
