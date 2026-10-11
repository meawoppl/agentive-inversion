use crate::claude_auth::ClaudeAuthCard;
use crate::context::use_app_context;
use crate::time::{format_countdown, format_local_clock, format_local_time};
use gloo_net::http::Request;
use shared_types::{PipelineStatsResponse, RetriageResponse};
use yew::prelude::*;

/// Pipeline infrastructure screen: triage mode, health, and stage counts
/// How often the pipeline view re-polls while open. Short enough that the
/// next-run countdown stays roughly true, long enough not to hammer a
/// group-by over the whole email table.
const PIPELINE_REFRESH_MS: u32 = 20_000;

#[function_component(PipelineView)]
pub(crate) fn pipeline_view() -> Html {
    let ctx = use_app_context();
    let stats = use_state(|| None::<PipelineStatsResponse>);
    let error = use_state(|| None::<String>);
    let reload = use_state(|| 0u32);
    // A countdown that never re-renders is a lie after its first minute
    let tick = use_state(|| 0u32);
    // Re-triage discards the whole review queue, so it is deliberately two
    // clicks: the second button only exists once the first is pressed.
    let confirming_retriage = use_state(|| false);
    let retriaging = use_state(|| false);
    let retriage_result = use_state(|| None::<RetriageResponse>);

    {
        let stats = stats.clone();
        let error = error.clone();
        use_effect_with((*reload, *tick), move |_| {
            let stats = stats.clone();
            let error = error.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match Request::get("/api/pipeline/stats").send().await {
                    Ok(resp) if resp.ok() => match resp.json::<PipelineStatsResponse>().await {
                        Ok(data) => stats.set(Some(data)),
                        Err(e) => error.set(Some(format!("Bad response: {e}"))),
                    },
                    Ok(resp) => error.set(Some(format!("API error: {}", resp.status()))),
                    Err(e) => error.set(Some(format!("Request failed: {e}"))),
                }
            });
            || ()
        });
    }

    // Re-poll while the tab is open: keeps the countdown honest and picks up
    // a cycle finishing without the user reloading.
    {
        let tick = tick.clone();
        use_effect_with((), move |_| {
            let counter = std::rc::Rc::new(std::cell::Cell::new(0u32));
            let interval = gloo::timers::callback::Interval::new(PIPELINE_REFRESH_MS, move || {
                counter.set(counter.get().wrapping_add(1));
                tick.set(counter.get());
            });
            move || drop(interval)
        });
    }

    let on_retriage = {
        let confirming_retriage = confirming_retriage.clone();
        let retriaging = retriaging.clone();
        let retriage_result = retriage_result.clone();
        let reload = reload.clone();
        let refresh_pending = ctx.refresh_pending_count.clone();
        Callback::from(move |_| {
            let confirming_retriage = confirming_retriage.clone();
            let retriaging = retriaging.clone();
            let retriage_result = retriage_result.clone();
            let reload = reload.clone();
            let refresh_pending = refresh_pending.clone();
            wasm_bindgen_futures::spawn_local(async move {
                retriaging.set(true);
                if let Ok(response) = Request::post("/api/pipeline/retriage").send().await {
                    if response.ok() {
                        if let Ok(result) = response.json::<RetriageResponse>().await {
                            retriage_result.set(Some(result));
                            reload.set(*reload + 1);
                            refresh_pending.emit(());
                        }
                    }
                }
                retriaging.set(false);
                confirming_retriage.set(false);
            });
        })
    };

    let stage_order = [
        ("pending", "Pending screening"),
        ("archive_candidate", "Awaiting archive determination"),
        ("archived", "Archived by agent"),
        ("event_proposed", "Calendar events proposed"),
        ("event_duplicate", "Events already on calendar"),
        ("forward_proposed", "Forwards proposed"),
        ("forwarded", "Forwarded"),
        ("action_queued", "Queued for action pass"),
        ("todo_proposed", "Todos proposed"),
        ("kept", "Kept in inbox"),
        ("ignored", "Ignored"),
    ];

    html! {
        <div class="pipeline-view">
            <h2>{"Triage Pipeline"}</h2>
            <ClaudeAuthCard />
            {if let Some(err) = (*error).clone() {
                html! { <div class="error-banner">{err}</div> }
            } else {
                html! {}
            }}
            {if let Some(s) = (*stats).clone() {
                let count_for = |key: &str| {
                    s.stage_counts
                        .iter()
                        .find(|c| c.status == key)
                        .map(|c| c.count)
                        .unwrap_or(0)
                };
                html! {
                    <>
                        <div class={if s.mode == "agentic" { "pipeline-mode mode-agentic" } else { "pipeline-mode mode-disabled" }}>
                            <span class="pipeline-mode-label">{"Mode: "}</span>
                            <strong>{&s.mode}</strong>
                            {if let Some(t) = &s.last_cycle_at {
                                html! {
                                    <span class="pipeline-last-cycle" title={format!("{} UTC", t.format("%Y-%m-%d %H:%M:%S"))}>
                                        {format!(" — last cycle {}", format_local_time(t))}
                                    </span>
                                }
                            } else {
                                html! {}
                            }}
                            {if let Some(t) = &s.next_cycle_at {
                                html! {
                                    <span class="pipeline-next-cycle" title={format!("{} UTC", t.format("%Y-%m-%d %H:%M:%S"))}>
                                        {format!(" · next run {} ({})", format_local_clock(t), format_countdown(t))}
                                    </span>
                                }
                            } else {
                                html! {}
                            }}
                        </div>
                        {if let Some(e) = &s.last_cycle_error {
                            html! { <div class="error-banner">{format!("Last cycle error ({} consecutive): {}", s.consecutive_failures, e)}</div> }
                        } else {
                            html! {}
                        }}
                        <div class="pipeline-stages">
                            {stage_order.iter().map(|(key, label)| {
                                html! {
                                    <div class="pipeline-stage-card" key={*key}>
                                        <div class="pipeline-stage-count">{count_for(key)}</div>
                                        <div class="pipeline-stage-label">{*label}</div>
                                    </div>
                                }
                            }).collect::<Html>()}
                        </div>

                        <div class="pipeline-danger-zone">
                            <h4>{"Flush and re-triage"}</h4>
                            <p class="pipeline-danger-note">
                                {"Sends every live email back to the front of the pipeline so all                                   three agent passes run again — for when the rules have changed                                   and the existing verdicts are stale. Emails already archived in                                   Gmail are left settled, and nothing is re-fetched from Gmail.                                   Pending proposals are withdrawn, not deleted, so the old queue                                   stays in the Decision Log. Expect a full triage run, and expect                                   it to take many cycles to drain."}
                            </p>
                            {if *confirming_retriage {
                                let cancel = {
                                    let confirming_retriage = confirming_retriage.clone();
                                    Callback::from(move |_| confirming_retriage.set(false))
                                };
                                html! {
                                    <div class="pipeline-danger-actions">
                                        <button class="btn-reject" disabled={*retriaging} onclick={on_retriage.clone()}>
                                            {if *retriaging { "Flushing..." } else { "Yes, flush and re-triage everything" }}
                                        </button>
                                        <button class="btn-secondary" disabled={*retriaging} onclick={cancel}>
                                            {"Cancel"}
                                        </button>
                                    </div>
                                }
                            } else {
                                let start = {
                                    let confirming_retriage = confirming_retriage.clone();
                                    Callback::from(move |_| confirming_retriage.set(true))
                                };
                                html! {
                                    <button class="btn-secondary" onclick={start}>
                                        {"Flush and re-triage..."}
                                    </button>
                                }
                            }}
                            {if let Some(r) = &*retriage_result {
                                html! {
                                    <div class="rescan-result">
                                        <span class="rescan-summary">
                                            {format!(
                                                "Reset {} email(s); withdrew {} pending proposal(s)",
                                                r.emails_reset, r.decisions_withdrawn
                                            )}
                                        </span>
                                        <div class="rescan-detail">
                                            {format!(
                                                "{} already archived in Gmail were left alone.                                                  Re-triage runs in the background over the next cycles.",
                                                r.emails_skipped
                                            )}
                                        </div>
                                    </div>
                                }
                            } else {
                                html! {}
                            }}
                        </div>
                    </>
                }
            } else {
                html! { <p>{"Loading..."}</p> }
            }}
        </div>
    }
}
