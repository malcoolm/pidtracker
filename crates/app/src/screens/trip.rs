//! Detail spoje: všechny zastávky z jízdního řádu, časy se zpožděním a poloha vozu.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::alerts::ServiceAlert;
use pid_core::realtime::{Delays, FixFreshness, VehicleFix, VehiclePlace, fix_freshness, place_vehicle};
use pid_core::timetable::query::{BoardRow, TripRoute};

use crate::components::{DepTime, RouteBadge};
use crate::state::{AppState, Screen};
use crate::util::{css_color, now, page_hidden};

/// Kolik zastávek před vozem (nebo nástupem) a za nástupem ukázat při sbalené trase.
const CONTEXT_BEFORE: usize = 2;
const CONTEXT_AFTER: usize = 5;
/// Jak často se přepočítá stáří polohy, i když nová data nepřišla (offline).
const TICK_MS: u32 = 15_000;

/// „před 20 s“ / „před 3 min“ podle časové značky polohy.
fn age_label(timestamp: Option<i64>, now_ts: i64) -> Option<String> {
    let age = (now_ts - timestamp?).max(0);
    Some(if age < 60 { format!("před {age} s") } else { format!("před {} min", age / 60) })
}

fn vehicle_text(fix: &VehicleFix, now_ts: i64) -> String {
    let mut s = match &fix.label {
        Some(l) => format!("🚋 vůz {l}"),
        None => "🚋 vůz".to_string(),
    };
    if let Some(age) = age_label(fix.timestamp, now_ts) {
        s.push_str(" · ");
        if fix_freshness(fix.timestamp, now_ts) == FixFreshness::Stale {
            s.push_str("poloha ");
        }
        s.push_str(&age);
    }
    s
}

/// „dalších 18 zastávek“ se správným skloňováním.
fn more_text(count: usize) -> String {
    match count {
        1 => "další 1 zastávka".to_string(),
        2..=4 => format!("další {count} zastávky"),
        _ => format!("dalších {count} zastávek"),
    }
}

/// Detail spoje. `alert` je aktivní mimořádnost linky (⚠ v hlavičce).
#[component]
pub fn TripDetail(
    row: BoardRow,
    delays: RwSignal<Delays>,
    on_close: Callback<()>,
    #[prop(optional)] alert: Option<ServiceAlert>,
) -> impl IntoView {
    let state = expect_context::<AppState>();
    let route: TripRoute =
        state.tt.get_untracked().index().map(|ix| ix.trip_route(&row.dep)).unwrap_or(TripRoute { stops: Vec::new(), board: None });
    let coords: Vec<(f64, f64)> = route.stops.iter().map(|s| (s.lat, s.lon)).collect();
    let board = route.board;
    let n = route.stops.len();
    let stops = Arc::new(route.stops);

    let at = RwSignal::new(now());
    let head_open = RwSignal::new(false);
    let tail_open = RwSignal::new(false);

    // Poloha vozu ze sdíleného úložiště (#14): obnova po 20 s, dokud je detail otevřený.
    crate::positions::subscribe(state);
    let trip_id = row.dep.trip_gtfs_id.clone();
    // Poloha starší než 10 min se na trase nezobrazuje (#12).
    let vehicle = {
        let trip_id = trip_id.clone();
        Memo::new(move |_| {
            let now_ts = at.get().timestamp();
            state.positions.with(|p| {
                p.as_ref()
                    .and_then(|p| p.fixes.get(&trip_id).cloned())
                    .filter(|f| fix_freshness(f.timestamp, now_ts) != FixFreshness::Expired)
            })
        })
    };
    let loaded = Memo::new(move |_| state.positions.with(Option::is_some));
    // Nová data → přepočítat stáří polohy a časy.
    Effect::new(move |_| {
        state.positions.track();
        at.set(now());
    });
    // Stáří polohy roste i bez nových dat (offline), proto hodiny tikají samy.
    {
        let alive = Arc::new(AtomicBool::new(true));
        let flag = alive.clone();
        on_cleanup(move || flag.store(false, Ordering::Relaxed));
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

    let place = Memo::new(move |_| vehicle.with(|v| v.as_ref().and_then(|f| place_vehicle(&coords, f))));

    let body = {
        let stops = stops.clone();
        let trip_id = trip_id.clone();
        let dep_stop = row.dep.stop;
        move || {
            let t = at.get();
            let place_now = place.get();
            let veh = place_now.map(|p| match p {
                VehiclePlace::AtStop(i) | VehiclePlace::Between(i) => i,
            });
            // Rozsah: od (vůz nebo nástup) − 2 do (nástup + 5); zbytek je sbalený.
            let b = board.unwrap_or(0);
            let lo = veh.unwrap_or(b).min(b).saturating_sub(CONTEXT_BEFORE);
            let hi = (b + CONTEXT_AFTER).max(veh.map_or(0, |v| v + 2)).min(n.saturating_sub(1));
            let from = if head_open.get() { 0 } else { lo };
            let to = if tail_open.get() { n.saturating_sub(1) } else { hi };
            let head_hidden = from;
            let tail_hidden = n.saturating_sub(to + 1);
            let fix = vehicle.get();
            let now_ts = t.timestamp();
            let label = |fix: &Option<VehicleFix>| {
                fix.as_ref().map(|f| {
                    let stale = fix_freshness(f.timestamp, now_ts) == FixFreshness::Stale;
                    view! { <div class="trip-vehicle" class:stale=stale>{vehicle_text(f, now_ts)}</div> }.into_any()
                })
            };
            let mut out = Vec::new();
            if head_hidden > 0 {
                out.push(
                    view! { <button class="trip-more" on:click=move |_| head_open.set(true)>{format!("⋮ {}", more_text(head_hidden))}</button> }
                        .into_any(),
                );
            }
            if n > 0 {
                for i in from..=to {
                    let s = &stops[i];
                    if place_now == Some(VehiclePlace::AtStop(i)) {
                        out.extend(label(&fix));
                    }
                    let passed = match place_now {
                        Some(VehiclePlace::AtStop(v)) => i < v,
                        Some(VehiclePlace::Between(v)) => i <= v,
                        None => false,
                    };
                    let is_board = board == Some(i) || (board.is_none() && s.stop == dep_stop);
                    let delay = delays.with(|d| d.at(&trip_id, &s.gtfs_id));
                    out.push(
                        view! {
                            <div class="trip-stop" class:board=is_board class:passed=passed>
                                <span class="stop-name">
                                    {s.name.clone()}
                                    {is_board.then(|| view! { <span class="here">"tady stojíš"</span> })}
                                </span>
                                <DepTime scheduled=s.time delay_s=delay at=t />
                            </div>
                        }
                        .into_any(),
                    );
                    if place_now == Some(VehiclePlace::Between(i)) {
                        out.extend(label(&fix));
                    }
                }
            }
            if tail_hidden > 0 {
                out.push(
                    view! { <button class="trip-more" on:click=move |_| tail_open.set(true)>{format!("⋮ {}", more_text(tail_hidden))}</button> }
                        .into_any(),
                );
            }
            out
        }
    };

    view! {
        <section class="trip-panel" style=format!("--line:{}", css_color(row.dep.color))>
            <div class="trip-head">
                <RouteBadge name=row.dep.route_short_name.clone() color=row.dep.color />
                <span class="trip-title">"→ " {row.dep.headsign.clone()}</span>
                {alert.map(|a| {
                    let id = a.id;
                    view! {
                        <button class="trip-warn" aria-label="Mimořádnost na lince" on:click=move |_| state.screen.set(Screen::Alerts { open: Some(id.clone()) })>"⚠"</button>
                    }
                })}
                <button class="link" on:click=move |_| on_close.run(())>"Zpět"</button>
            </div>
            <div class="trip-line">{body}</div>
            <Show when=move || loaded.get() && place.get().is_none()>
                <p class="muted small">"Poloha vozu není k dispozici."</p>
            </Show>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age() {
        assert_eq!(age_label(Some(100), 120).as_deref(), Some("před 20 s"));
        assert_eq!(age_label(Some(100), 400).as_deref(), Some("před 5 min"));
        assert_eq!(age_label(Some(100), 90).as_deref(), Some("před 0 s"));
        assert_eq!(age_label(None, 90), None);
    }

    fn fix(timestamp: i64) -> VehicleFix {
        VehicleFix { label: Some("8758".into()), lat: 50.0, lon: 14.0, stop_sequence: None, timestamp: Some(timestamp) }
    }

    #[test]
    fn vehicle_text_fresh_and_stale() {
        assert_eq!(vehicle_text(&fix(1_000), 1_030), "🚋 vůz 8758 · před 30 s");
        assert_eq!(vehicle_text(&fix(1_000), 1_090), "🚋 vůz 8758 · před 1 min");
        assert_eq!(vehicle_text(&fix(1_000), 1_180), "🚋 vůz 8758 · poloha před 3 min");
    }

    #[test]
    fn more() {
        assert_eq!(more_text(1), "další 1 zastávka");
        assert_eq!(more_text(3), "další 3 zastávky");
        assert_eq!(more_text(18), "dalších 18 zastávek");
    }
}
