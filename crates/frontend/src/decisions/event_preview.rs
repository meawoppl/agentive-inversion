use shared_types::ProposedCalendarEventAction;
use yew::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub(super) struct EventPreviewProps {
    pub(super) event: ProposedCalendarEventAction,
    #[prop_or(false)]
    pub(super) compact: bool,
}

#[function_component(EventPreviewCard)]
pub(super) fn event_preview_card(props: &EventPreviewProps) -> Html {
    let e = &props.event;
    let same_day = e.start.date_naive() == e.end.date_naive();
    let date_line = if same_day {
        e.start.format("%A, %B %-d, %Y").to_string()
    } else {
        format!(
            "{} \u{2013} {}",
            e.start.format("%a %b %-d"),
            e.end.format("%a %b %-d, %Y")
        )
    };
    let time_line = format!(
        "{} \u{2013} {} UTC",
        e.start.format("%H:%M"),
        e.end.format("%H:%M")
    );

    html! {
        <div class={if props.compact { "event-preview event-preview-compact" } else { "event-preview" }}>
            <div class="event-preview-title">{&e.summary}</div>
            <div class="event-preview-when">
                <span>{date_line}</span>
                <span class="event-preview-time">{time_line}</span>
            </div>
            {if let Some(loc) = e.location.as_ref().filter(|l| !l.is_empty()) {
                html! { <div class="event-preview-location">{"\u{1F4CD} "}{loc}</div> }
            } else {
                html! {}
            }}
            {if !props.compact {
                html! {
                    <>
                        {if let Some(desc) = e.description.as_ref().filter(|d| !d.is_empty()) {
                            html! { <div class="event-preview-desc">{desc}</div> }
                        } else {
                            html! {}
                        }}
                        <div class="event-preview-meta">
                            <span>{format!("Calendar: {}", e.calendar_name.clone().unwrap_or_else(|| "Agent".to_string()))}</span>
                            {if let Some(link) = &e.email_link {
                                html! { <a href={link.clone()} target="_blank" rel="noopener">{"Source email"}</a> }
                            } else {
                                html! {}
                            }}
                        </div>
                    </>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
