use gloo_net::http::Request;
use shared_types::CalendarEventResponse;
use yew::prelude::*;

#[derive(Clone, PartialEq)]
enum CalendarTab {
    Today,
    Week,
    All,
}

#[function_component(CalendarView)]
pub(crate) fn calendar_view() -> Html {
    let events = use_state(Vec::<CalendarEventResponse>::new);
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    let current_tab = use_state(|| CalendarTab::Week);

    // Fetch events based on current tab
    {
        let events = events.clone();
        let loading = loading.clone();
        let error = error.clone();
        let tab = (*current_tab).clone();
        use_effect_with(tab, move |tab| {
            let events = events.clone();
            let loading = loading.clone();
            let error = error.clone();
            let tab = tab.clone();
            wasm_bindgen_futures::spawn_local(async move {
                loading.set(true);
                error.set(None);

                let endpoint = match tab {
                    CalendarTab::Today => "/api/calendar-events/today",
                    CalendarTab::Week => "/api/calendar-events/week",
                    CalendarTab::All => "/api/calendar-events?limit=100",
                };

                match Request::get(endpoint).send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<Vec<CalendarEventResponse>>().await {
                                Ok(data) => {
                                    events.set(data);
                                }
                                Err(e) => {
                                    error.set(Some(format!("Failed to parse events: {}", e)));
                                }
                            }
                        } else {
                            error.set(Some(format!("Server error: {}", response.status())));
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Failed to fetch events: {}", e)));
                    }
                }
                loading.set(false);
            });
            || ()
        });
    }

    let set_tab_today = {
        let current_tab = current_tab.clone();
        Callback::from(move |_| current_tab.set(CalendarTab::Today))
    };

    let set_tab_week = {
        let current_tab = current_tab.clone();
        Callback::from(move |_| current_tab.set(CalendarTab::Week))
    };

    let set_tab_all = {
        let current_tab = current_tab.clone();
        Callback::from(move |_| current_tab.set(CalendarTab::All))
    };

    let events_list = (*events).clone();

    html! {
        <div class="calendar-view">
            <div class="calendar-header">
                <h2>{"Calendar Events"}</h2>
                <div class="calendar-tabs">
                    <button
                        class={if *current_tab == CalendarTab::Today { "tab-btn active" } else { "tab-btn" }}
                        onclick={set_tab_today}
                    >
                        {"Today"}
                    </button>
                    <button
                        class={if *current_tab == CalendarTab::Week { "tab-btn active" } else { "tab-btn" }}
                        onclick={set_tab_week}
                    >
                        {"This Week"}
                    </button>
                    <button
                        class={if *current_tab == CalendarTab::All { "tab-btn active" } else { "tab-btn" }}
                        onclick={set_tab_all}
                    >
                        {"All Events"}
                    </button>
                </div>
            </div>

            {if *loading {
                html! { <div class="loading">{"Loading events..."}</div> }
            } else if let Some(err) = (*error).as_ref() {
                html! { <div class="error">{err}</div> }
            } else if events_list.is_empty() {
                html! {
                    <div class="empty-state">
                        <p>{"No calendar events found."}</p>
                        <p class="hint">{"Connect a calendar account to see events here."}</p>
                    </div>
                }
            } else {
                html! {
                    <div class="calendar-events-list">
                        {events_list.iter().map(|event| {
                            html! {
                                <CalendarEventCard event={event.clone()} />
                            }
                        }).collect::<Html>()}
                    </div>
                }
            }}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct CalendarEventCardProps {
    event: CalendarEventResponse,
}

#[function_component(CalendarEventCard)]
fn calendar_event_card(props: &CalendarEventCardProps) -> Html {
    let event = &props.event;

    // Format date and time
    let start_date = event.start_time.format("%a, %b %d").to_string();
    let start_time = event.start_time.format("%H:%M").to_string();
    let end_time = event.end_time.format("%H:%M").to_string();

    let time_display = if event.all_day {
        "All Day".to_string()
    } else {
        format!("{} - {}", start_time, end_time)
    };

    let status_class = match event.status.as_str() {
        "confirmed" => "status-confirmed",
        "tentative" => "status-tentative",
        "cancelled" => "status-cancelled",
        _ => "status-default",
    };

    html! {
        <div class={format!("calendar-event-card {}", status_class)}>
            <div class="event-datetime">
                <div class="event-date">{start_date}</div>
                <div class="event-time">{time_display}</div>
            </div>
            <div class="event-details">
                <h3 class="event-title">{event.summary.as_deref().unwrap_or("(No title)")}</h3>
                {if let Some(location) = &event.location {
                    html! { <div class="event-location">{"📍 "}{location}</div> }
                } else {
                    html! {}
                }}
                {if let Some(link) = &event.conference_link {
                    html! {
                        <div class="event-conference">
                            <a href={link.clone()} target="_blank" rel="noopener">{"🔗 Join video call"}</a>
                        </div>
                    }
                } else {
                    html! {}
                }}
                {if !event.attendees.is_empty() {
                    html! {
                        <div class="event-attendees">
                            <span class="attendees-label">{"Attendees: "}</span>
                            {event.attendees.iter().take(3).map(|a| {
                                let status_icon = match a.response_status.as_deref() {
                                    Some("accepted") => "✓",
                                    Some("declined") => "✗",
                                    Some("tentative") => "?",
                                    _ => "•",
                                };
                                html! {
                                    <span class="attendee" title={a.email.clone()}>
                                        {status_icon}{" "}{a.display_name.as_deref().unwrap_or(&a.email)}
                                    </span>
                                }
                            }).collect::<Html>()}
                            {if event.attendees.len() > 3 {
                                html! { <span class="more-attendees">{format!(" +{} more", event.attendees.len() - 3)}</span> }
                            } else {
                                html! {}
                            }}
                        </div>
                    }
                } else {
                    html! {}
                }}
                {if event.recurring {
                    html! { <span class="event-recurring">{"🔄 Recurring"}</span> }
                } else {
                    html! {}
                }}
            </div>
            <div class="event-status">
                <span class={format!("status-badge {}", status_class)}>{&event.status}</span>
            </div>
        </div>
    }
}
