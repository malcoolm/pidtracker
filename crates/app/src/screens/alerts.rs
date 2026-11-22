//! Seznam mimořádností: relevantní zvýrazněné, klepnutím se rozbalí popis.

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::alerts::{AlertPhase, ListedAlert, list_alerts};

use crate::alerts::{now_s, route_badge_info, when_label};
use crate::components::RouteBadge;
use crate::state::AppState;
use crate::season::{Vignette, XmasVignette};

#[component]
fn AlertItem(item: ListedAlert, expanded: RwSignal<Option<String>>) -> impl IntoView {
    let state = expect_context::<AppState>();
    let a = item.alert;
    let id = a.id.clone();
    let dom_id = format!("alert-{}", a.id);
    let toggle_id = id.clone();
    let open_id = id.clone();
    let planned = (item.phase == AlertPhase::Planned).then(|| a.start.map(when_label)).flatten();
    let badges = a
        .route_ids
        .iter()
        .map(|r| {
            let (name, color) = route_badge_info(state, r);
            view! { <RouteBadge name=name color=color /> }
        })
        .collect_view();
    let description = a.description.clone();
    let url = a.url.clone();
    view! {
        <div class="alert-item" class:relevant=item.relevant id=dom_id>
            <button
                class="alert-head"
                aria-expanded=move || (expanded.get().as_deref() == Some(open_id.as_str())).to_string()
                on:click=move |_| expanded.update(|e| *e = if e.as_deref() == Some(toggle_id.as_str()) { None } else { Some(toggle_id.clone()) })
            >
                <span class="alert-badges">{badges}</span>
                <span class="alert-title">{a.header.clone()}</span>
                {planned.map(|p| view! { <span class="alert-planned">{format!("plánováno od {p}")}</span> })}
            </button>
            <Show when=move || expanded.get().as_deref() == Some(id.as_str())>
                <div class="alert-body">
                    <p>{description.clone()}</p>
                    {url.clone().map(|u| view! { <a href=u target="_blank" rel="noopener">"Více na pid.cz"</a> })}
                </div>
            </Show>
        </div>
    }
}

#[component]
pub fn AlertsScreen(open: Option<String>) -> impl IntoView {
    let state = expect_context::<AppState>();
    let expanded = RwSignal::new(open.clone());
    let list = move || {
        let watched = state.watched.get();
        state.alerts.with(|a| list_alerts(a, &watched, now_s()))
    };

    // Předvybraná mimořádnost se odscrolluje po vykreslení.
    if let Some(id) = open {
        spawn_local(async move {
            TimeoutFuture::new(50).await;
            if let Some(el) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id(&format!("alert-{id}"))) {
                el.scroll_into_view();
            }
        });
    }

    view! {
        <section class="alerts">
            <XmasVignette kind=Vignette::Candle />
            <h2>"Mimořádnosti"</h2>
            {move || {
                let items = list();
                if items.is_empty() {
                    let text = if state.alerts_known.get() { "Žádné mimořádnosti" } else { "Mimořádnosti nejsou k dispozici (offline)" };
                    view! { <p class="muted">{text}</p> }.into_any()
                } else {
                    view! {
                        <div class="list">
                            {items.into_iter().map(|l| view! { <AlertItem item=l expanded=expanded /> }).collect_view()}
                        </div>
                    }
                    .into_any()
                }
            }}
        </section>
    }
}
