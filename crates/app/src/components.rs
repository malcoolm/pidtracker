//! Sdílené UI prvky.

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::ride::{Ride, RideStatus};
use pid_core::timetable::query::{DataStatus, DepartureSource};

use crate::google::auth::sign_in;
use crate::state::{AppState, Screen, SyncStatus, TtState};
use crate::sync::request_sync;
use crate::util::{css_color, duration_label, hhmm, kind_color, local, now};

/// Barevný štítek s číslem linky.
#[component]
pub fn RouteBadge(#[prop(into)] name: String, color: u32) -> impl IntoView {
    view! { <span class="badge" style=format!("background:{}", css_color(color))>{name}</span> }
}

/// Barva linky jízdy: z jízdního řádu, jinak podle typu dopravy.
pub fn ride_color(state: &AppState, ride: &Ride) -> u32 {
    state
        .tt
        .get_untracked()
        .index()
        .and_then(|ix| ix.tt.routes.iter().find(|r| r.gtfs_id == ride.route_id).map(|r| r.color))
        .unwrap_or_else(|| kind_color(ride.route_type))
}

#[component]
pub fn SyncBadge() -> impl IntoView {
    let state = expect_context::<AppState>();
    let me = move || state.me.get();
    view! {
        <div class="sync">
            {move || match state.sync.get() {
                SyncStatus::Idle { last: Some(t) } => view! { <span class="ok">"✓ Synchronizováno " {t}</span> }.into_any(),
                SyncStatus::Idle { last: None } => view! { <span class="muted">"Zatím nesynchronizováno"</span> }.into_any(),
                SyncStatus::Running => view! { <span class="muted">"Synchronizuji…"</span> }.into_any(),
                SyncStatus::Pending(n) => view! { <span class="warn">{format!("Čeká {n} změn na odeslání")}</span> }.into_any(),
                SyncStatus::NeedsSignIn(n) => view! {
                    <button class="link" on:click=move |_| {
                        let fut = sign_in(me().as_deref());
                        spawn_local(async move {
                            match fut.await {
                                Ok(_) => request_sync(state),
                                Err(e) => state.show_toast(e.message()),
                            }
                        });
                    }>{if n > 0 { format!("Přihlásit pro synchronizaci ({n})") } else { "Přihlásit pro synchronizaci".to_string() }}</button>
                }.into_any(),
                SyncStatus::Error(m) => view! {
                    <button class="link danger" on:click=move |_| request_sync(state)>{format!("Chyba synchronizace: {m} – zkusit znovu")}</button>
                }.into_any(),
            }}
        </div>
    }
}

#[component]
pub fn Toast() -> impl IntoView {
    let state = expect_context::<AppState>();
    Effect::new(move |_| {
        if state.toast.get().is_some() {
            spawn_local(async move {
                TimeoutFuture::new(4_000).await;
                state.toast.set(None);
            });
        }
    });
    view! {
        <Show when=move || state.toast.get().is_some()>
            <div class="toast" on:click=move |_| state.toast.set(None)>{move || state.toast.get()}</div>
        </Show>
    }
}

#[component]
pub fn BottomNav() -> impl IntoView {
    let state = expect_context::<AppState>();
    let item = move |label: &'static str, target: Screen| {
        let t = target.clone();
        let active = move || {
            let s = state.screen.get();
            std::mem::discriminant(&s) == std::mem::discriminant(&t)
                || (matches!(t, Screen::Home) && matches!(s, Screen::Board { .. } | Screen::Alight))
                || (matches!(t, Screen::History) && matches!(s, Screen::Edit(_)))
        };
        view! {
            <button class:active=active on:click=move |_| state.screen.set(target.clone())>{label}</button>
        }
    };
    view! {
        <nav class="bottom-nav">
            {item("Jízda", Screen::Home)}
            {item("Historie", Screen::History)}
            {item("Statistiky", Screen::Stats)}
            {item("Nastavení", Screen::Settings)}
        </nav>
    }
}

/// Upozornění na stav dat jízdního řádu.
#[component]
pub fn DataBanner() -> impl IntoView {
    let state = expect_context::<AppState>();
    move || match state.tt.get() {
        TtState::Loading => Some(view! { <div class="banner muted">"Načítám jízdní řád…"</div> }.into_any()),
        TtState::Missing(m) => Some(view! { <div class="banner warn">{m}</div> }.into_any()),
        TtState::Ready(ix) => match ix.status(now().date_naive()) {
            DataStatus::Valid => None,
            DataStatus::ExpiringSoon { days_left } => Some(
                view! { <div class="banner warn">{format!("Jízdní řád končí za {days_left} dní – aktualizace by měla přijít automaticky.")}</div> }
                    .into_any(),
            ),
            DataStatus::Expired => Some(
                view! { <div class="banner danger">"Jízdní řád je prošlý – návrhy linek nemusí sedět. Zastávky a linky můžete zadat ručně."</div> }
                    .into_any(),
            ),
        },
    }
}

/// Řádek jízdy v seznamu.
#[component]
pub fn RideRow(ride: Ride) -> impl IntoView {
    let state = expect_context::<AppState>();
    let color = ride_color(&state, &ride);
    let id = ride.id.clone();
    let mine = state.me.get_untracked().is_some_and(|me| ride.can_edit(&me));
    let who = (!mine).then(|| state.nickname(&ride.person));
    let to = ride.alight_stop_name.clone().unwrap_or_else(|| "…".into());
    let times = match &ride.alight_time {
        Some(a) => format!("{} – {}", hhmm(&ride.board_time), hhmm(a)),
        None => hhmm(&ride.board_time),
    };
    let duration = ride.duration_minutes().filter(|m| *m >= 0).map(duration_label);
    let unfinished = ride.status == RideStatus::InProgress;
    let vehicle = (!ride.vehicle.is_empty()).then(|| ride.vehicle.clone());
    view! {
        <button class="ride-row" on:click=move |_| state.screen.set(Screen::Edit(id.clone()))>
            <RouteBadge name=ride.route_short_name.clone() color=color />
            <span class="ride-main">
                <span class="ride-stops">{ride.board_stop_name.clone()} " → " {to}</span>
                <span class="ride-meta">
                    {times}
                    {duration.map(|d| format!(" · {d}"))}
                    {vehicle.map(|v| format!(" · vůz {v}"))}
                    {who.map(|w| format!(" · {w}"))}
                    {unfinished.then(|| view! { <span class="tag">"neukončeno"</span> })}
                </span>
            </span>
        </button>
    }
}

/// „Dnes“, „Včera“, „po 26. 10.“
pub fn day_label(ride: &Ride) -> String {
    use chrono::Datelike;
    let d = local(&ride.board_time).date_naive();
    let today = now().date_naive();
    if d == today {
        "Dnes".into()
    } else if Some(d) == today.pred_opt() {
        "Včera".into()
    } else {
        let wd = ["po", "út", "st", "čt", "pá", "so", "ne"][d.weekday().num_days_from_monday() as usize];
        if d.year() == today.year() {
            format!("{wd} {}. {}.", d.day(), d.month())
        } else {
            format!("{wd} {}. {}. {}", d.day(), d.month(), d.year())
        }
    }
}
