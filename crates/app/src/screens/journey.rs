//! Detail spojení: svislá časová osa (chůze přerušovaně, jízdy v barvě linky).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::DateTime;
use chrono_tz::Tz;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::realtime::Delays;
use pid_core::timetable::query::{DelayKind, GoStatus, TimetableIndex, go_status, stops_text, time_display};
use pid_core::timetable::router::{Journey, Leg};
use pid_core::timetable::walk::transfer_seconds;

use super::search::{start_time, transfers_label, walk_min};
use crate::components::RouteBadge;
use crate::state::AppState;
use crate::util::{css_color, duration_label, now, page_hidden};

/// Jak často se přepočítá „Vyrazte do N min“ (jen místní čas, žádné stahování).
const TICK_MS: u32 = 15_000;

/// Čas události se zpožděním z živých dat (jen u jízdy, jinak holý čas).
fn time_cell(
    time: DateTime<Tz>,
    ids: Option<(String, String)>,
    delays: RwSignal<Delays>,
    at: RwSignal<DateTime<Tz>>,
) -> impl IntoView {
    move || {
        let delay = ids.as_ref().and_then(|(trip, stop)| delays.with(|d| d.at(trip, stop)));
        let t = time_display(time, delay, at.get());
        let (class, note) = match t.kind {
            DelayKind::Late => ("late", t.detail.split(" · ").next().map(str::to_string)),
            DelayKind::Early => ("early", t.detail.split(" · ").next().map(str::to_string)),
            DelayKind::OnTime | DelayKind::Unknown => ("", None),
        };
        let main = match t.actual {
            Some(actual) => view! { <s class="sched">{t.scheduled}</s>" "<b class=class>{actual}</b> }.into_any(),
            None => view! { <b>{t.scheduled}</b> }.into_any(),
        };
        view! { {main}{note.map(|n| view! { <small class=class>{n}</small> })} }
    }
}

/// Jeden řádek osy: čas | kolejnice (tečka + úsek pod ní) | obsah.
/// `seg`: `Some(Some(barva))` jízda, `Some(None)` chůze (přerušovaně), `None` konec osy.
fn row(
    time: impl IntoView + 'static,
    dot: String,
    seg: Option<Option<String>>,
    content: impl IntoView + 'static,
    class: &'static str,
) -> impl IntoView {
    let line = seg.map(|s| match s {
        Some(c) => view! { <span class="ln" style=format!("background:{c}")></span> }.into_any(),
        None => view! { <span class="ln walk"></span> }.into_any(),
    });
    view! {
        <div class=format!("jd-row {class}")>
            <div class="jd-time">{time}</div>
            <div class="jd-rail">{line}<span class="dot" style=format!("border-color:{dot}")></span></div>
            <div class="jd-body">{content}</div>
        </div>
    }
}

/// Zastávky jízdy mezi nástupem a výstupem, rozbalitelné.
fn ride_stops(
    ix: &Arc<TimetableIndex>,
    leg: &Leg,
    color: String,
    trip_id: String,
    delays: RwSignal<Delays>,
    at: RwSignal<DateTime<Tz>>,
) -> Option<AnyView> {
    let stops = ix.leg_stops(leg);
    let mids: Vec<_> = stops.get(1..stops.len().saturating_sub(1))?.to_vec();
    if mids.is_empty() {
        return None;
    }
    let open = RwSignal::new(false);
    let n = mids.len();
    let rows = {
        let color = color.clone();
        move || {
            mids.iter()
                .map(|s| {
                    let ids = Some((trip_id.clone(), s.gtfs_id.clone()));
                    row(
                        time_cell(s.time, ids, delays, at),
                        color.clone(),
                        Some(Some(color.clone())),
                        view! { <span class="jd-mid-name">{s.name.clone()}</span> },
                        "mid",
                    )
                    .into_any()
                })
                .collect_view()
        }
    };
    let toggle = view! {
        <button class="jd-exp" aria-expanded=move || open.get().to_string() on:click=move |_| open.update(|o| *o = !*o)>
            {move || format!("{} {}", if open.get() { "▾" } else { "▸" }, stops_text(n))}
        </button>
    };
    Some(
        view! {
            {row("", color.clone(), Some(Some(color)), toggle, "exp")}
            {move || open.get().then(&rows)}
        }
        .into_any(),
    )
}

/// Detail spojení; `delays` a hodiny se obnovují s obrazovkou hledání.
#[component]
pub fn JourneyDetail(j: Journey, delays: RwSignal<Delays>, on_close: Callback<()>) -> impl IntoView {
    let state = expect_context::<AppState>();
    let at = RwSignal::new(now());
    let alive = Arc::new(AtomicBool::new(true));
    {
        let flag = alive.clone();
        on_cleanup(move || flag.store(false, Ordering::Relaxed));
        let alive = alive.clone();
        spawn_local(async move {
            loop {
                TimeoutFuture::new(TICK_MS).await;
                if !alive.load(Ordering::Relaxed) {
                    break;
                }
                if !page_hidden() {
                    at.set(now());
                }
            }
        });
    }
    // Návrat z pozadí: hodiny hned přepočítat.
    Effect::new(move |prev: Option<u32>| {
        let f = state.foreground.get();
        if prev.is_some_and(|p| p != f) {
            at.set(now());
        }
        f
    });

    // Ze zastávky začíná spojení odjezdem (rezerva je jen v „Vyrazte do“).
    let start = start_time(&j);
    let minutes = j.arrive.timestamp().div_euclid(60) - start.timestamp().div_euclid(60);
    let mut meta = format!("{} · {}", duration_label(minutes), transfers_label(j.transfers));
    if j.walk_s > 0 {
        meta.push_str(&format!(" · pěšky {} min", walk_min(j.walk_s)));
    }

    let ix = state.tt.get_untracked().index();

    // Čísla vozů ze sdíleného úložiště poloh (#14), jen má-li spojení nějakou jízdu.
    let has_ride = j.legs.iter().any(|l| matches!(l, Leg::Ride { .. }));
    if has_ride {
        crate::positions::subscribe(state);
    }
    // První jízda: kontrola „Ujelo“ podle živého zpoždění.
    let first = ix.as_ref().and_then(|ix| {
        j.legs.iter().find_map(|l| match *l {
            Leg::Ride { trip, board_stop, dep, .. } => {
                Some((dep, ix.tt.trips[trip as usize].gtfs_id.clone(), ix.tt.stops[board_stop as usize].gtfs_id.clone()))
            }
            Leg::Walk { .. } => None,
        })
    });
    let leave = j.leave;
    let go_class = RwSignal::new("jd-go");
    let go = move || {
        let t = at.get();
        let (dep, delay) = match first.as_ref() {
            Some((dep, trip, stop)) => (Some(*dep), delays.with(|d| d.at(trip, stop))),
            None => (None, None),
        };
        let status = go_status(leave, dep, delay, t);
        go_class.set(if status == GoStatus::Gone { "jd-go gone" } else { "jd-go" });
        status.text()
    };

    let mut rows: Vec<AnyView> = Vec::new();
    if let Some(ix) = ix.as_ref() {
        let n = j.legs.len();
        for (i, leg) in j.legs.iter().enumerate() {
            let next_walk = j.legs.get(i + 1).and_then(|l| match *l {
                Leg::Walk { seconds, .. } => Some(seconds),
                Leg::Ride { .. } => None,
            });
            // Přestup na stejném stanovišti (bez úseku chůze): další jízda hned navazuje.
            let next_board = j.legs.get(i + 1).and_then(|l| match *l {
                Leg::Ride { board_stop, .. } => Some(board_stop),
                Leg::Walk { .. } => None,
            });
            match *leg {
                Leg::Walk { from: None, to, seconds } => {
                    if seconds == 0 {
                        continue;
                    }
                    rows.push(
                        row(
                            view! { <b>{j.leave.format("%H:%M").to_string()}</b> },
                            "var(--muted)".into(),
                            Some(None),
                            view! {
                                <div class="jd-name">"📍 Moje poloha"</div>
                                <div class="jd-sub">{format!("🚶 {} min na zastávku {}", walk_min(seconds), ix.tt.stop_name(to))}</div>
                            },
                            "",
                        )
                        .into_any(),
                    );
                }
                // Chůze mezi jízdami je popisek výstupního řádku, na konci i řádek cíle.
                Leg::Walk { from: Some(_), to, .. } => {
                    if i + 1 == n {
                        rows.push(
                            row(
                                view! { <b>{j.arrive.format("%H:%M").to_string()}</b> },
                                "var(--muted)".into(),
                                None,
                                view! { <div class="jd-name">{format!("🏁 {}", ix.tt.stop_name(to))}</div> },
                                "",
                            )
                            .into_any(),
                        );
                    }
                }
                Leg::Ride { trip, board_stop, alight_stop, dep, arr, .. } => {
                    let tt = &ix.tt;
                    let t = &tt.trips[trip as usize];
                    let route = &tt.routes[tt.patterns[tt.profiles[t.profile as usize].pattern as usize].route as usize];
                    let color = css_color(route.color);
                    let trip_id = t.gtfs_id.clone();
                    let board_ids = Some((trip_id.clone(), tt.stops[board_stop as usize].gtfs_id.clone()));
                    let alight_ids = Some((trip_id.clone(), tt.stops[alight_stop as usize].gtfs_id.clone()));
                    let platform = ix.platform(board_stop).map(|p| format!("nástupiště {p}"));
                    let sub = {
                        let trip_id = trip_id.clone();
                        move || {
                            let vehicle = state.positions.with(|p| {
                                p.as_ref().and_then(|p| p.fixes.get(&trip_id)?.label.as_ref().map(|l| format!("🚋 vůz {l}")))
                            });
                            let parts: Vec<String> = platform.iter().cloned().chain(vehicle).collect();
                            (!parts.is_empty()).then(|| view! { <div class="jd-sub">{parts.join(" · ")}</div> })
                        }
                    };
                    rows.push(
                        row(
                            time_cell(dep, board_ids, delays, at),
                            color.clone(),
                            Some(Some(color.clone())),
                            view! {
                                <div class="jd-name">
                                    <b>{tt.stop_name(board_stop).to_string()}</b>
                                    " "
                                    <RouteBadge name=route.short_name.clone() color=route.color />
                                    " → "
                                    {tt.string(t.headsign).to_string()}
                                </div>
                                {sub}
                            },
                            "",
                        )
                        .into_any(),
                    );
                    rows.extend(ride_stops(ix, leg, color.clone(), trip_id, delays, at));
                    // Za jízdou může následovat chůze; je-li ta poslední, jde do cíle, ne o přestup.
                    let last = i + 1 + usize::from(next_walk.is_some()) == n;
                    let walk_label = next_walk
                        .filter(|&s| s > 0)
                        .map(|s| {
                            let (prefix, suffix) = if last { ("", " do cíle") } else { ("přestup ", "") };
                            format!("{prefix}🚶 {} min{suffix}", walk_min(s))
                        })
                        .or_else(|| {
                            // Nejkratší přestup na místě (metro 4 min, jinak 2 min).
                            next_board.map(|b| {
                                let min = transfer_seconds(0, ix.is_metro_stop(alight_stop), ix.is_metro_stop(b));
                                format!("přestup {} min", min.div_ceil(60))
                            })
                        });
                    let seg = if next_walk.is_some() || next_board.is_some() { Some(None) } else { None };
                    rows.push(
                        row(
                            time_cell(arr, alight_ids, delays, at),
                            color.clone(),
                            seg,
                            view! {
                                <div class="jd-name"><b>{tt.stop_name(alight_stop).to_string()}</b></div>
                                {walk_label.map(|w| view! { <div class="jd-sub">{w}</div> })}
                            },
                            "",
                        )
                        .into_any(),
                    );
                }
            }
        }
    }

    view! {
        <section class="trip-panel journey-detail">
            <div class="trip-head">
                <span class="trip-title">{format!("{} → {}", start.format("%H:%M"), j.arrive.format("%H:%M"))}</span>
                <button class="link" on:click=move |_| on_close.run(())>"Zpět"</button>
            </div>
            <div class="jd-meta">{meta}</div>
            <div class=move || go_class.get()>{go}</div>
            <div class="jd-line">{rows}</div>
        </section>
    }
}
