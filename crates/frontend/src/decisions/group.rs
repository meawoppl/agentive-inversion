use super::formatting::humanize_type;
use super::row::DecisionRow;
use shared_types::{PendingDecisionGroup, PendingDecisionItem, ProposedForwardAction};
use uuid::Uuid;
use yew::prelude::*;

/// Distinct destinations across a group of forward proposals.
///
/// Approving a forward sends mail, so the address has to be legible on the
/// control that sends it — "Approve all (5)" one click away from an expand
/// toggle is not the place to hide where five emails are going. Normally
/// there is exactly one address (the destination is server policy), but the
/// group is rendered from whatever the decisions actually say.
fn forward_destinations(group: &PendingDecisionGroup) -> Vec<String> {
    let mut addresses: Vec<String> = Vec::new();
    for item in &group.items {
        let Ok(action) =
            serde_json::from_value::<ProposedForwardAction>(item.decision.proposed_action.clone())
        else {
            continue;
        };
        if !addresses.contains(&action.to_address) {
            addresses.push(action.to_address);
        }
    }
    addresses
}

#[derive(Properties, PartialEq)]
pub(super) struct DecisionGroupCardProps {
    pub(super) group: PendingDecisionGroup,
    pub(super) expanded: bool,
    pub(super) selected: std::collections::HashSet<Uuid>,
    pub(super) on_toggle_expanded: Callback<String>,
    pub(super) on_toggle_group: Callback<Vec<Uuid>>,
    pub(super) on_toggle_item: Callback<Uuid>,
    pub(super) on_approve: Callback<Uuid>,
    pub(super) on_reject: Callback<(Uuid, Option<String>)>,
    pub(super) on_approve_many: Callback<Vec<Uuid>>,
    pub(super) on_reject_many: Callback<Vec<Uuid>>,
    pub(super) on_open: Callback<PendingDecisionItem>,
}

/// One sender's pending decisions of a single kind. The header carries the
/// verdict for the whole group; the body is the individual rows.
///
/// A group of one has nothing to collapse and no batch to act on, so it
/// renders as a bare row — the row already names the sender and the subject.
#[function_component(DecisionGroupCard)]
pub(super) fn decision_group_card(props: &DecisionGroupCardProps) -> Html {
    let group = &props.group;
    let ids: Vec<Uuid> = group.items.iter().map(|i| i.decision.id).collect();
    let count = ids.len();
    let all_selected = !ids.is_empty() && ids.iter().all(|id| props.selected.contains(id));
    let destinations = forward_destinations(group);

    if let [item] = group.items.as_slice() {
        return html! {
            <DecisionRow
                item={item.clone()}
                selected={props.selected.contains(&item.decision.id)}
                on_toggle={props.on_toggle_item.clone()}
                on_approve={props.on_approve.clone()}
                on_reject={props.on_reject.clone()}
                on_open={props.on_open.clone()}
            />
        };
    }

    let sender_label = group
        .sender_name
        .clone()
        .or_else(|| group.sender_address.clone())
        .unwrap_or_else(|| format!("{} (no source email)", humanize_type(&group.decision_type)));

    let toggle_expanded = {
        let on_toggle_expanded = props.on_toggle_expanded.clone();
        let key = group.key.clone();
        Callback::from(move |_| on_toggle_expanded.emit(key.clone()))
    };
    let toggle_group = {
        let on_toggle_group = props.on_toggle_group.clone();
        let ids = ids.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            on_toggle_group.emit(ids.clone());
        })
    };
    let approve_all = {
        let on_approve_many = props.on_approve_many.clone();
        let ids = ids.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            on_approve_many.emit(ids.clone());
        })
    };
    let reject_all = {
        let on_reject_many = props.on_reject_many.clone();
        let ids = ids.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            on_reject_many.emit(ids.clone());
        })
    };

    html! {
        <div class="decision-group">
            <div class="decision-group-header" onclick={toggle_expanded.clone()}>
                <input
                    type="checkbox"
                    checked={all_selected}
                    onclick={toggle_group}
                />
                <div class="decision-group-title">
                    <span class="group-sender">{sender_label}</span>
                    <span class="group-type">{humanize_type(&group.decision_type)}</span>
                    {if group.sender_address.is_some() {
                        html! { <span class="group-address">{group.sender_address.clone().unwrap_or_default()}</span> }
                    } else {
                        html! {}
                    }}
                    {if destinations.is_empty() {
                        html! {}
                    } else {
                        html! {
                            <span class="group-forward-to" title="Approving forwards these emails to this address">
                                {format!("\u{2192} {}", destinations.join(", "))}
                            </span>
                        }
                    }}
                </div>
                <span class="group-count">{format!("{}", count)}</span>
                <span class="decision-time" title="Newest email in this group">
                    {group
                        .items
                        .iter()
                        .filter_map(|i| i.email.as_ref().map(|e| e.received_at))
                        .max()
                        .unwrap_or(group.latest_at)
                        .format("%b %d, %H:%M")
                        .to_string()}
                </span>
                <div class="decision-group-actions">
                    <button class="btn-approve" onclick={approve_all}>
                        {format!("Approve all ({})", count)}
                    </button>
                    <button class="btn-reject" onclick={reject_all}>
                        {format!("Reject all ({})", count)}
                    </button>
                    <button class="btn-secondary group-toggle" onclick={toggle_expanded}>
                        {if props.expanded { "Hide" } else { "Show" }}
                    </button>
                </div>
            </div>

            {if props.expanded {
                html! {
                    <div class="decision-list">
                        {group.items.iter().map(|item| {
                            html! {
                                <DecisionRow
                                    key={item.decision.id.to_string()}
                                    item={item.clone()}
                                    selected={props.selected.contains(&item.decision.id)}
                                    on_toggle={props.on_toggle_item.clone()}
                                    on_approve={props.on_approve.clone()}
                                    on_reject={props.on_reject.clone()}
                                    on_open={props.on_open.clone()}
                                />
                            }
                        }).collect::<Html>()}
                    </div>
                }
            } else {
                html! {
                    <div class="decision-group-preview">
                        {group.items.iter().take(3).map(|item| html! {
                            <div class="group-preview-line">
                                {match item.email.as_ref() {
                                    Some(email) => html! {
                                        <a
                                            class="email-link"
                                            href={email.gmail_link.clone()}
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            title="Open the original in Gmail"
                                        >
                                            {&email.subject}
                                        </a>
                                    },
                                    None => html! { {&item.decision.reasoning} },
                                }}
                            </div>
                        }).collect::<Html>()}
                        {if count > 3 {
                            html! { <div class="group-preview-more">{format!("+{} more", count - 3)}</div> }
                        } else {
                            html! {}
                        }}
                    </div>
                }
            }}
        </div>
    }
}
