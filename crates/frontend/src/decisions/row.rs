use super::event_preview::EventPreviewCard;
use super::formatting::{date_tooltip, email_or_decision_date};
use shared_types::{
    PendingDecisionItem, ProposedCalendarEventAction, ProposedForwardAction, ProposedTodoAction,
};
use uuid::Uuid;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(super) struct DecisionRowProps {
    pub(super) item: PendingDecisionItem,
    pub(super) selected: bool,
    pub(super) on_toggle: Callback<Uuid>,
    pub(super) on_approve: Callback<Uuid>,
    pub(super) on_reject: Callback<(Uuid, Option<String>)>,
    pub(super) on_open: Callback<PendingDecisionItem>,
}

#[function_component(DecisionRow)]
pub(super) fn decision_row(props: &DecisionRowProps) -> Html {
    let decision = &props.item.decision;
    let decision_id = decision.id;

    // Parse proposed action into the type-appropriate shape so the card says
    // what approving actually does, not just a type name
    let proposed: Option<ProposedTodoAction> = (decision.decision_type == "create_todo")
        .then(|| serde_json::from_value(decision.proposed_action.clone()).ok())
        .flatten();
    let proposed_forward: Option<ProposedForwardAction> = (decision.decision_type
        == "forward_email")
        .then(|| serde_json::from_value(decision.proposed_action.clone()).ok())
        .flatten();
    let proposed_event: Option<ProposedCalendarEventAction> = (decision.decision_type
        == "create_calendar_event")
        .then(|| serde_json::from_value(decision.proposed_action.clone()).ok())
        .flatten();

    let toggle = {
        let on_toggle = props.on_toggle.clone();
        Callback::from(move |_| on_toggle.emit(decision_id))
    };
    let open = {
        let on_open = props.on_open.clone();
        let item = props.item.clone();
        Callback::from(move |_| on_open.emit(item.clone()))
    };
    let approve = {
        let on_approve = props.on_approve.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            on_approve.emit(decision_id);
        })
    };
    let reject = {
        let on_reject = props.on_reject.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            on_reject.emit((decision_id, None));
        })
    };

    html! {
        <div class={format!("decision-item {} {}",
            if props.selected { "selected" } else { "" },
            decision.confidence_level.as_str())}>
            <div class="decision-select">
                <input type="checkbox" checked={props.selected} onchange={toggle} />
            </div>
            <div class="decision-content" onclick={open}>
                <div class="decision-header">
                    <span class="decision-source">
                        {if decision.source_type == "email" { "Email" } else { &decision.source_type }}
                    </span>
                    <span class={format!("confidence-badge {}", decision.confidence_level)}>
                        {format!("{}% confident", (decision.confidence * 100.0) as i32)}
                    </span>
                    <span class="decision-time" title={date_tooltip(props.item.email.as_ref(), decision.created_at)}>
                        {email_or_decision_date(props.item.email.as_ref(), decision.created_at)
                            .format("%b %d, %H:%M")
                            .to_string()}
                    </span>
                </div>

                {if let Some(email) = &props.item.email {
                    html! {
                        <div class="decision-email-info">
                            <span class="email-from">{email.from_name.clone().unwrap_or_else(|| email.from_address.clone())}</span>
                            // The row itself opens the detail modal, so the
                            // link has to swallow the click that follows it
                            <a
                                class="email-subject email-link"
                                href={email.gmail_link.clone()}
                                target="_blank"
                                rel="noopener noreferrer"
                                title="Open the original in Gmail"
                                onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                            >
                                {&email.subject}
                            </a>
                        </div>
                    }
                } else {
                    html! {}
                }}

                <div class="decision-proposed">
                    <span class="proposed-label">{"Proposed: "}</span>
                    <span class="proposed-action">{&decision.decision_type}</span>
                    {if let Some(action) = &proposed {
                        html! {
                            <span class="proposed-title">{format!(" - \"{}\"", action.todo_title)}</span>
                        }
                    } else {
                        html! {}
                    }}
                    {if let Some(fwd) = &proposed_forward {
                        html! {
                            <span class="proposed-title">{format!(" \u{2192} {}", fwd.to_address)}</span>
                        }
                    } else {
                        html! {}
                    }}
                </div>

                {if let Some(event) = &proposed_event {
                    html! { <EventPreviewCard event={event.clone()} compact={true} /> }
                } else {
                    html! {}
                }}

                <div class="decision-reasoning">
                    {&decision.reasoning}
                </div>
            </div>
            <div class="decision-actions">
                <button class="btn-approve" onclick={approve}>{"Approve"}</button>
                <button class="btn-reject" onclick={reject}>{"Reject"}</button>
            </div>
        </div>
    }
}
