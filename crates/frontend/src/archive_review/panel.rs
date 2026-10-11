use super::queue::{
    bundle_by_sender, next_bundle_key, ReviewMsg, ReviewQueue, RETURNED_BUNDLE_COOLDOWN_MS,
};
use crate::context::use_app_context;
use gloo_net::http::Request;
use shared_types::{
    ArchiveReviewItem, ArchiveReviewResponse, BatchApproveDecisionsRequest, BatchOperationResponse,
    BatchRejectDecisionsRequest,
};
use uuid::Uuid;
use yew::prelude::*;

/// One-place rapid accept over archive proposals, in bundles of ten. Accept
/// removes the bundle instantly and fires the backend work; anything the
/// backend couldn't archive animates back into the queue.
#[function_component(ArchiveReviewPanel)]
pub(crate) fn archive_review_panel() -> Html {
    let queue = use_reducer(ReviewQueue::default);
    // Accepts/keeps here resolve pending decisions; the nav badge and the
    // decision inbox count must learn about it
    let refresh_pending = use_app_context().refresh_pending_count;

    let load = {
        let queue = queue.clone();
        Callback::from(move |_: ()| {
            let queue = queue.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match Request::get("/api/pipeline/archive-review").send().await {
                    Ok(resp) if resp.ok() => match resp.json::<ArchiveReviewResponse>().await {
                        Ok(data) => {
                            queue.dispatch(ReviewMsg::Loaded {
                                mode: data.archive_mode,
                                bundles: bundle_by_sender(data.items),
                            });
                        }
                        Err(e) => {
                            queue.dispatch(ReviewMsg::LoadFailed(format!("Bad response: {e}")))
                        }
                    },
                    Ok(resp) => queue.dispatch(ReviewMsg::LoadFailed(format!(
                        "API error: {}",
                        resp.status()
                    ))),
                    Err(e) => queue.dispatch(ReviewMsg::LoadFailed(format!("Request failed: {e}"))),
                }
            });
        })
    };

    {
        let load = load.clone();
        use_effect_with((), move |_| {
            load.emit(());
            || ()
        });
    }

    let on_accept = {
        let queue = queue.clone();
        let refresh_pending = refresh_pending.clone();
        Callback::from(move |_| {
            let Some(front) = queue.bundles.first() else {
                return;
            };
            if queue.disabled.contains(&front.key) {
                return;
            }
            let items = front.items.clone();
            let ids: Vec<Uuid> = items.iter().map(|i| i.decision_id).collect();
            // Optimistic: the bundle is gone before the network is involved,
            // so the next bundle is already under the cursor
            queue.dispatch(ReviewMsg::PopFront);
            let queue = queue.clone();
            let refresh_pending = refresh_pending.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let request = BatchApproveDecisionsRequest { decision_ids: ids };
                let outcome = match Request::post("/api/decisions/batch/approve")
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&request).unwrap())
                    .expect("body")
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => match resp.json::<BatchOperationResponse>().await {
                        Ok(out) if out.failed.is_empty() => Ok(()),
                        Ok(out) => {
                            let failed_ids: std::collections::HashSet<Uuid> =
                                out.failed.iter().map(|f| f.id).collect();
                            let error = out
                                .failed
                                .first()
                                .map(|f| f.error.clone())
                                .unwrap_or_default();
                            let failed_items: Vec<ArchiveReviewItem> = items
                                .into_iter()
                                .filter(|i| failed_ids.contains(&i.decision_id))
                                .collect();
                            Err((failed_items, error))
                        }
                        // The server said ok but the body didn't parse: the
                        // approvals executed, so returning items would only
                        // set up doomed retries
                        Err(e) => {
                            queue.dispatch(ReviewMsg::Banner(Some(format!(
                                "Accepted, but the response was unreadable: {e}"
                            ))));
                            Ok(())
                        }
                    },
                    Ok(resp) => Err((items, format!("server error {}", resp.status()))),
                    Err(e) => Err((items, format!("request failed: {e}"))),
                };
                if let Err((failed_items, error)) = outcome {
                    let key = next_bundle_key();
                    queue.dispatch(ReviewMsg::ReturnBundle {
                        key,
                        items: failed_items,
                        error,
                    });
                    let queue = queue.clone();
                    gloo::timers::callback::Timeout::new(RETURNED_BUNDLE_COOLDOWN_MS, move || {
                        queue.dispatch(ReviewMsg::CooldownOver(key));
                    })
                    .forget();
                }
                refresh_pending.emit(());
            });
        })
    };

    let on_keep = {
        let queue = queue.clone();
        let refresh_pending = refresh_pending.clone();
        Callback::from(move |(bundle_key, decision_id): (usize, Uuid)| {
            queue.dispatch(ReviewMsg::RemoveItem {
                bundle_key,
                decision_id,
            });
            let queue = queue.clone();
            let refresh_pending = refresh_pending.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let request = BatchRejectDecisionsRequest {
                    decision_ids: vec![decision_id],
                    feedback: None,
                };
                let failed = match Request::post("/api/decisions/batch/reject")
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&request).unwrap())
                    .expect("body")
                    .send()
                    .await
                {
                    Ok(resp) => !resp.ok(),
                    Err(_) => true,
                };
                if failed {
                    queue.dispatch(ReviewMsg::Banner(Some(
                        "Keep didn't reach the server - that email is still proposed for archive"
                            .to_string(),
                    )));
                }
                refresh_pending.emit(());
            });
        })
    };

    let on_unsubscribe = {
        let queue = queue.clone();
        Callback::from(move |(bundle_key, email_id): (usize, Uuid)| {
            queue.dispatch(ReviewMsg::UnsubStatus {
                key: bundle_key,
                label: "...".to_string(),
            });
            let queue = queue.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let label = match Request::post(&format!("/api/emails/{email_id}/unsubscribe"))
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        match resp.json::<shared_types::UnsubscribeResponse>().await {
                            Ok(out) => match out.method.as_str() {
                                "one_click" => "Unsubscribed".to_string(),
                                "mailto" => "Unsubscribe email sent".to_string(),
                                "link" => {
                                    if let Some(url) = out.url {
                                        let _ = gloo::utils::window()
                                            .open_with_url_and_target(&url, "_blank");
                                    }
                                    "Opened unsubscribe page".to_string()
                                }
                                _ => "No standard unsubscribe".to_string(),
                            },
                            Err(_) => "Failed".to_string(),
                        }
                    }
                    Ok(resp) => format!("Failed ({})", resp.status()),
                    Err(_) => "Failed".to_string(),
                };
                queue.dispatch(ReviewMsg::UnsubStatus {
                    key: bundle_key,
                    label,
                });
            });
        })
    };

    let total_emails: usize = queue.bundles.iter().map(|b| b.items.len()).sum();
    let front = queue.bundles.first().cloned();
    let front_disabled = front
        .as_ref()
        .map(|b| queue.disabled.contains(&b.key))
        .unwrap_or(true);

    html! {
        <div class="archive-review">
            <div class="view-header">
                <h2>{"Archive Review"}</h2>
                {if queue.archive_mode == "propose" {
                    html! { <span class="mode-badge mode-dry-run" title="The agent only proposes; accepting a bundle is what archives those emails in Gmail">{"dry-run"}</span> }
                } else if !queue.archive_mode.is_empty() {
                    html! { <span class="mode-badge mode-execute">{&queue.archive_mode}</span> }
                } else {
                    html! {}
                }}
                <button class="btn-secondary" onclick={let load = load.clone(); Callback::from(move |_| load.emit(()))}>
                    {"Refresh"}
                </button>
            </div>
            {if let Some(msg) = queue.banner.clone() {
                html! { <div class="review-banner">{msg}</div> }
            } else {
                html! {}
            }}
            {match front {
                None if !queue.loaded => html! { <p class="empty-state">{"Loading proposals..."}</p> },
                None => html! {
                    <p class="empty-state">{"Nothing awaiting review. The agent will keep proposing as the backlog drains."}</p>
                },
                Some(bundle) => {
                    let bundle_key = bundle.key;
                    html! {
                        <>
                            <div class="rapid-accept-bar">
                                <button
                                    class="btn-approve accept-big-btn"
                                    disabled={front_disabled}
                                    onclick={on_accept.clone()}
                                >
                                    {format!("Archive these {}", bundle.items.len())}
                                </button>
                                <span class="review-progress">
                                    {format!("{} emails in {} bundles remaining", total_emails, queue.bundles.len())}
                                </span>
                            </div>
                            <div class={if bundle.returned { "review-bundle-card bundle-returned" } else { "review-bundle-card" }}>
                                <div class="bundle-sender-header">
                                    {match &bundle.sender {
                                        Some(s) => html! {
                                            <>
                                                <span class="bundle-sender-name">{s}</span>
                                                <span class="bundle-sender-count">{format!("{} emails from this sender", bundle.items.len())}</span>
                                                {match queue.unsub.get(&bundle_key) {
                                                    Some(status) if status == "..." => html! {
                                                        <span class="unsub-status">{"Unsubscribing..."}</span>
                                                    },
                                                    Some(status) => html! {
                                                        <span class="unsub-status">{status.clone()}</span>
                                                    },
                                                    None => {
                                                        let unsub = on_unsubscribe.clone();
                                                        let email_id = bundle.items.first().map(|i| i.email_id);
                                                        html! {
                                                            <button
                                                                class="btn-secondary unsub-btn"
                                                                disabled={front_disabled || email_id.is_none()}
                                                                onclick={Callback::from(move |_| {
                                                                    if let Some(eid) = email_id {
                                                                        unsub.emit((bundle_key, eid));
                                                                    }
                                                                })}
                                                            >
                                                                {"Unsubscribe"}
                                                            </button>
                                                        }
                                                    }
                                                }}
                                            </>
                                        },
                                        None => html! {
                                            <span class="bundle-sender-count">{format!("{} assorted senders", bundle.items.len())}</span>
                                        },
                                    }}
                                </div>
                                {if let Some(err) = bundle.error.clone() {
                                    html! {
                                        <div class="bundle-error-note">
                                            {format!("Returned after a failed attempt: {err}")}
                                            <button
                                                class="btn-secondary"
                                                disabled={front_disabled}
                                                onclick={let queue = queue.clone(); Callback::from(move |_| queue.dispatch(ReviewMsg::DismissBundle(bundle_key)))}
                                            >
                                                {"Dismiss bundle"}
                                            </button>
                                        </div>
                                    }
                                } else {
                                    html! {}
                                }}
                                {for bundle.items.iter().map(|item| {
                                    let keep = on_keep.clone();
                                    let decision_id = item.decision_id;
                                    // Hover preview: enough of the message to
                                    // resolve an uncertain call without
                                    // leaving the accept flow
                                    let preview = item
                                        .body_preview
                                        .clone()
                                        .or_else(|| item.snippet.clone());
                                    html! {
                                        <div class="review-item-row" key={item.decision_id.to_string()}>
                                            <div class="review-item-main">
                                                <span class="review-item-from">
                                                    {item.from_name.clone().unwrap_or_else(|| item.from_address.clone())}
                                                </span>
                                                <a
                                                    class="review-item-subject email-link"
                                                    href={item.gmail_link.clone()}
                                                    target="_blank"
                                                    rel="noopener noreferrer"
                                                    title="Open the original in Gmail"
                                                >
                                                    {&item.subject}
                                                </a>
                                                <span class="review-item-reason" title={item.reasoning.clone()}>
                                                    {&item.reasoning}
                                                </span>
                                                {if let Some(preview) = preview {
                                                    html! {
                                                        <div class="review-tooltip">
                                                            <div class="review-tooltip-header">
                                                                <span>{&item.from_address}</span>
                                                                <span>{item.received_at.format("%Y-%m-%d %H:%M").to_string()}</span>
                                                            </div>
                                                            <div class="review-tooltip-subject">{&item.subject}</div>
                                                            <div class="review-tooltip-body">{preview}</div>
                                                        </div>
                                                    }
                                                } else {
                                                    html! {}
                                                }}
                                            </div>
                                            <span class="review-item-date">
                                                {item.received_at.format("%Y-%m-%d").to_string()}
                                            </span>
                                            <button
                                                class="btn-secondary review-keep-btn"
                                                disabled={front_disabled}
                                                onclick={Callback::from(move |_| keep.emit((bundle_key, decision_id)))}
                                            >
                                                {"Keep"}
                                            </button>
                                        </div>
                                    }
                                })}
                            </div>
                            {if queue.bundles.len() > 1 {
                                html! {
                                    <p class="review-queue-hint">
                                        {format!("{} more bundles queued behind this one", queue.bundles.len() - 1)}
                                    </p>
                                }
                            } else {
                                html! {}
                            }}
                        </>
                    }
                }
            }}
        </div>
    }
}
