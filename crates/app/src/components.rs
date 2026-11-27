//! Sdílené UI prvky.

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::ride::{Ride, RideStatus};
use chrono::DateTime;
use chrono_tz::Tz;
use pid_core::timetable::query::{DataStatus, DelayKind, DepartureSource, time_display};

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
                    <button class="link" data-no-reauth="1" on:click=move |_| {
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
    let vehicle = (!ride.vehicle.is_empty()).then(|| match ride.license_plate.trim() {
        "" => ride.vehicle.clone(),
        plate => format!("{} · {plate}", ride.vehicle),
    });
    let has_note = !ride.note.trim().is_empty();
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
                    {has_note.then_some(" · 📝")}
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

/// Čas odjezdu: jízdní řád, při zpoždění/předčasu přeškrtnutý a nový čas, pod tím popisek.
#[component]
pub fn DepTime(scheduled: DateTime<Tz>, delay_s: Option<i32>, at: DateTime<Tz>) -> impl IntoView {
    let t = time_display(scheduled, delay_s, at);
    let (class, detail_class) = match t.kind {
        DelayKind::Late => ("actual late", "late"),
        DelayKind::Early => ("actual early", "early"),
        DelayKind::OnTime => ("", "ok"),
        DelayKind::Unknown => ("", "muted"),
    };
    let main = match t.actual {
        Some(actual) => view! { <s class="sched">{t.scheduled}</s>" "<b class=class>{actual}</b> }.into_any(),
        None => view! { <b>{t.scheduled}</b> }.into_any(),
    };
    view! {
        <span class="dep-time">
            {main}
            <small class=detail_class>{t.detail}</small>
        </span>
    }
}

/// Značka vozu (#13): bílý piktogram druhu dopravy na kolečku v barvě linky.
#[component]
pub fn VehicleIcon(kind: pid_core::ride::RouteKind, color: u32) -> impl IntoView {
    use pid_core::ride::RouteKind;
    let bg = css_color(color);
    // Výřezy oken v barvě linky, aby piktogram vypadal jako obrys vozu.
    let shape = match kind {
        RouteKind::Tram => view! {
            <path d="M8 1h8v1.5h-3.2l1.6 2.5H13l-1-1.6L11 5H9.6l1.6-2.5H8z" />
            <rect x="4" y="6" width="16" height="12" rx="2.5" />
            <rect x="6.5" y="8.5" width="11" height="4.5" rx="1" fill=bg.clone() />
            <rect x="3" y="19" width="18" height="2" rx="1" />
        }
        .into_any(),
        RouteKind::Trolleybus => view! {
            <path d="M9 1.5l3 4.5h-1.4L8 2.3zM15 1.5l3 4.5h-1.4L14 2.3z" />
            <rect x="3" y="6" width="18" height="11" rx="3" />
            <rect x="5.5" y="8.5" width="13" height="4" rx="1" fill=bg.clone() />
            <circle cx="7.5" cy="19" r="2" />
            <circle cx="16.5" cy="19" r="2" />
        }
        .into_any(),
        RouteKind::Metro => view! {
            <circle cx="12" cy="12" r="10" />
            <path d="M6.5 17V7h2.2L12 12l3.3-5h2.2v10h-2.3v-6.3L12 15.4l-3.2-4.7V17z" fill=bg.clone() />
        }
        .into_any(),
        RouteKind::Train => view! {
            <rect x="5" y="2" width="14" height="16" rx="4" />
            <rect x="7.5" y="5" width="9" height="5" rx="1" fill=bg.clone() />
            <circle cx="9" cy="14" r="1.3" fill=bg.clone() />
            <circle cx="15" cy="14" r="1.3" fill=bg.clone() />
            <path d="M7 22l2.5-3.5h1.8L9 22zM17 22l-2.5-3.5h-1.8L15 22z" />
        }
        .into_any(),
        _ => view! {
            <rect x="3" y="4" width="18" height="13" rx="3" />
            <rect x="5.5" y="6.5" width="13" height="5" rx="1" fill=bg.clone() />
            <circle cx="7.5" cy="19" r="2" />
            <circle cx="16.5" cy="19" r="2" />
        }
        .into_any(),
    };
    view! {
        <span class="veh-icon" style=format!("background:{bg}") aria-label=kind.label_cs()>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="#fff" aria-hidden="true">{shape}</svg>
        </span>
    }
}
