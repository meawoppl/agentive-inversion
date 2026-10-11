use super::event_preview::EventPreviewCard;
use shared_types::{
    AgentDecisionResponse, DecisionEmailContext, ProposedCalendarEventAction,
    ProposedForwardAction, ProposedTodoAction,
};
use yew::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub(super) struct DecisionDetailProps {
    pub(super) decision: AgentDecisionResponse,
    /// Source email, when the decision came from one
    #[prop_or_default]
    pub(super) email: Option<DecisionEmailContext>,
}

#[function_component(DecisionDetailView)]
pub(super) fn decision_detail_view(props: &DecisionDetailProps) -> Html {
    let decision = &props.decision;
    let proposed: Option<ProposedTodoAction> = if decision.decision_type == "create_todo" {
        serde_json::from_value(decision.proposed_action.clone()).ok()
    } else {
        None
    };
    let proposed_event: Option<ProposedCalendarEventAction> =
        if decision.decision_type == "create_calendar_event" {
            serde_json::from_value(decision.proposed_action.clone()).ok()
        } else {
            None
        };
    let proposed_forward: Option<ProposedForwardAction> =
        if decision.decision_type == "forward_email" {
            serde_json::from_value(decision.proposed_action.clone()).ok()
        } else {
            None
        };

    html! {
        <div class="decision-detail">
            <div class="detail-section">
                <h4>{"Source"}</h4>
                <p>{format!("Type: {}", decision.source_type)}</p>
                {if let Some(ext_id) = &decision.source_external_id {
                    html! { <p>{format!("External ID: {}", ext_id)}</p> }
                } else {
                    html! {}
                }}
            </div>

            <div class="detail-section">
                <h4>{"Decision"}</h4>
                <p>{format!("Type: {}", decision.decision_type)}</p>
                <p>{format!("Status: {}", decision.status)}</p>
                <p class={format!("confidence {}", decision.confidence_level)}>
                    {format!("Confidence: {}% ({})", (decision.confidence * 100.0) as i32, decision.confidence_level)}
                </p>
            </div>

            {if let Some(event) = proposed_event {
                html! {
                    <div class="detail-section">
                        <h4>{"Proposed Calendar Event"}</h4>
                        <EventPreviewCard event={event} />
                        <p class="gate-note">{"Approving creates this event on your Agent calendar."}</p>
                    </div>
                }
            } else {
                html! {}
            }}

            {if let Some(fwd) = proposed_forward {
                html! {
                    <div class="detail-section">
                        <h4>{"Proposed Forward"}</h4>
                        <p><strong>{"Subject: "}</strong>{&fwd.subject}</p>
                        <p><strong>{"From account: "}</strong>{&fwd.from_account}</p>
                        <p><strong>{"To: "}</strong>{&fwd.to_address}</p>
                        <p class="gate-note">{"Approving forwards the original email (with attachments) to this address, then labels it agent-forwarded and archives it."}</p>
                    </div>
                }
            } else {
                html! {}
            }}

            {if let Some(action) = proposed {
                html! {
                    <div class="detail-section">
                        <h4>{"Proposed Todo"}</h4>
                        <p><strong>{"Title: "}</strong>{&action.todo_title}</p>
                        {if let Some(desc) = &action.todo_description {
                            html! { <p><strong>{"Description: "}</strong>{desc}</p> }
                        } else {
                            html! {}
                        }}
                        {if let Some(due) = &action.due_date {
                            html! { <p><strong>{"Due Date: "}</strong>{due.format("%Y-%m-%d %H:%M").to_string()}</p> }
                        } else {
                            html! {}
                        }}
                        {if let Some(priority) = &action.priority {
                            html! { <p><strong>{"Priority: "}</strong>{priority}</p> }
                        } else {
                            html! {}
                        }}
                    </div>
                }
            } else {
                html! {}
            }}

            <div class="detail-section">
                <h4>{"Reasoning"}</h4>
                <p class="reasoning-text">{&decision.reasoning}</p>
            </div>

            {if let Some(details) = &decision.reasoning_details {
                html! {
                    <div class="detail-section">
                        <h4>{"Analysis Details"}</h4>
                        <pre class="reasoning-details">{serde_json::to_string_pretty(details).unwrap_or_default()}</pre>
                    </div>
                }
            } else {
                html! {}
            }}

            <div class="detail-section">
                <h4>{"Timeline"}</h4>
                {if let Some(email) = &props.email {
                    html! {
                        <>
                            <p>{format!("Email received: {}", email.received_at.format("%Y-%m-%d %H:%M:%S"))}</p>
                            <p>
                                <a
                                    class="source-email-link"
                                    href={email.gmail_link.clone()}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                >
                                    {format!("Open \u{201c}{}\u{201d} in Gmail", email.subject)}
                                </a>
                            </p>
                        </>
                    }
                } else {
                    html! {}
                }}
                <p>{format!("Proposed: {}", decision.created_at.format("%Y-%m-%d %H:%M:%S"))}</p>
                {if let Some(reviewed) = decision.reviewed_at {
                    html! { <p>{format!("Reviewed: {}", reviewed.format("%Y-%m-%d %H:%M:%S"))}</p> }
                } else {
                    html! {}
                }}
                {if let Some(executed) = decision.executed_at {
                    html! { <p>{format!("Executed: {}", executed.format("%Y-%m-%d %H:%M:%S"))}</p> }
                } else {
                    html! {}
                }}
            </div>
        </div>
    }
}
