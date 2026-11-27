//! Detail spoje: všechny zastávky z jízdního řádu, časy se zpožděním a poloha vozu.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::alerts::ServiceAlert;
use pid_core::realtime::{
    ArrivalState, Delays, FixFreshness, VehicleFix, VehiclePlace, arrival_state, estimate_place, fix_freshness,
    is_estimate, place_vehicle, segment_progress,
};
use pid_core::ride::RouteKind;
use pid_core::timetable::query::{BoardRow, TripRoute, stops_text};

use crate::components::{DepTime, RouteBadge, VehicleIcon};
use crate::state::{AppState, Screen};
use crate::util::{css_color, now, page_hidden};

/// Kolik zastávek před vozem (nebo nástupem) a za nástupem ukázat při sbalené trase.
const CONTEXT_BEFORE: usize = 2;
const CONTEXT_AFTER: usize = 5;
/// Jak často se přepočítá stáří polohy, i když nová data nepřišla (offline).
const TICK_MS: u32 = 15_000;
/// Jak často se posouvá odhadovaná poloha značky (#15).
const MOVE_TICK_MS: u32 = 1_000;

/// „před 20 s“ / „před 3 min“ podle časové značky polohy.
fn age_label(timestamp: Option<i64>, now_ts: i64) -> Option<String> {
    let age = (now_ts - timestamp?).max(0);
    Some(if age < 60 { format!("před {age} s") } else { format!("před {} min", age / 60) })
}

/// Druh vozu malým písmem do souhrnu („autobus 1956 …“).
fn kind_word(kind: RouteKind) -> &'static str {
    match kind {
        RouteKind::Bus => "autobus",
        RouteKind::Tram => "tramvaj",
        RouteKind::Trolleybus => "trolejbus",
        RouteKind::Metro => "metro",
        RouteKind::Train => "vlak",
        _ => "vůz",
    }
}

/// Druhý řádek souhrnu: „autobus 1956 · za zastávkou Pod Královkou · před 20 s“.
fn vehicle_text(kind: RouteKind, fix: &VehicleFix, place: Option<&str>, now_ts: i64, estimate: bool) -> String {
    let mut s = kind_word(kind).to_string();
    if let Some(l) = &fix.label {
        s.push(' ');
        s.push_str(l);
    }
    if let Some(p) = place {
        s.push_str(" · ");
        s.push_str(p);
    }
    if let Some(age) = age_label(fix.timestamp, now_ts) {
        s.push_str(" · ");
        match fix_freshness(fix.timestamp, now_ts) {
            FixFreshness::Stale => s.push_str("poloha "),
            _ if estimate => s.push_str("odhad, GPS "),
            _ => {}
        }
        s.push_str(&age);
    }
    s
}

/// Velký řádek souhrnu (#13): „za 3 min · 2 zastávky“, „přijíždí“, „ve stanici“, „vůz už odjel“.
/// `minutes` = za kolik minut je vůz na mé zastávce podle jízdního řádu a zpoždění.
fn summary_text(state: Option<ArrivalState>, minutes: Option<i64>) -> String {
    let when = minutes.map(|m| if m <= 0 { "teď".to_string() } else { format!("za {m} min") });
    match state {
        Some(ArrivalState::Arriving) => "přijíždí".into(),
        Some(ArrivalState::AtStop) => "ve stanici".into(),
        Some(ArrivalState::Departed) => "vůz už odjel".into(),
        Some(ArrivalState::StopsAway(n)) => match when {
            Some(w) => format!("{w} · {}", stops_text(n)),
            None => stops_text(n),
        },
        // Bez polohy vozu po odjezdu podle jízdního řádu: „odjezd před N min“, ne „teď“ donekonečna.
        None => match minutes {
            Some(m) if m < 0 => format!("odjezd před {} min", -m),
            _ => when.unwrap_or_default(),
        },
    }
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
    let coords: Arc<Vec<(f64, f64)>> = Arc::new(route.stops.iter().map(|s| (s.lat, s.lon)).collect());
    let board = route.board;
    let n = route.stops.len();
    let stops = Arc::new(route.stops);
    let kind = RouteKind::from_gtfs(row.dep.route_type);
    let color = row.dep.color;

    let at = RwSignal::new(now());
    let now_s = RwSignal::new(now().timestamp());
    let head_open = RwSignal::new(false);
    let tail_open = RwSignal::new(false);

    // Poloha vozu ze sdíleného úložiště (#14): obnova po 20 s, dokud je detail otevřený.
    crate::positions::subscribe(state);
    let trip_id = row.dep.trip_gtfs_id.clone();
    // Poloha starší než 10 min se na trase nezobrazuje (#12).
    let vehicle = {
        let trip_id = trip_id.clone();
        Memo::new(move |_| {
            let now_ts = now_s.get();
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
    // Hodiny: `now_s` po 1 s pro posun značky a stáří polohy (#15), `at` po 15 s pro časy
    // v seznamu zastávek, který se tak nepřekresluje každou sekundu.
    {
        let alive = Arc::new(AtomicBool::new(true));
        let flag = alive.clone();
        on_cleanup(move || flag.store(false, Ordering::Relaxed));
        spawn_local(async move {
            let mut ticks = 0u32;
            let mut was_hidden = false;
            loop {
                TimeoutFuture::new(MOVE_TICK_MS).await;
                if !alive.load(Ordering::Relaxed) {
                    break;
                }
                if page_hidden() {
                    was_hidden = true;
                    continue;
                }
                now_s.set(now().timestamp());
                ticks += 1;
                // Po návratu z pozadí hned přepočítat i časy a stáří v seznamu.
                if was_hidden || ticks * MOVE_TICK_MS >= TICK_MS {
                    was_hidden = false;
                    ticks = 0;
                    at.set(now());
                }
            }
        });
    }

    let place = {
        let coords = coords.clone();
        Memo::new(move |_| vehicle.with(|v| v.as_ref().and_then(|f| place_vehicle(&coords, f))))
    };
    // Podíl úseku, na kterém vůz je (jen mezi zastávkami).
    let progress = {
        let coords = coords.clone();
        Memo::new(move |_| match (place.get(), vehicle.get()) {
            (Some(VehiclePlace::Between(i)), Some(f)) => segment_progress(&coords, &f, i),
            _ => 0.0,
        })
    };
    // Časy zastávek (jízdní řád + zpoždění) pro odhad jízdních dob úseků.
    let stop_times = {
        let stops = stops.clone();
        let trip_id = trip_id.clone();
        Memo::new(move |_| {
            delays.with(|d| {
                stops.iter().map(|s| s.time.timestamp() + i64::from(d.at(&trip_id, &s.gtfs_id).unwrap_or(0))).collect::<Vec<_>>()
            })
        })
    };
    // Odhad místa vozu „teď“ (#15), jen u čerstvé polohy; zastaralá stojí, kde ji GPS viděla.
    let estimated = Memo::new(move |_| {
        let now_ts = now_s.get();
        let gps = (place.get(), progress.get());
        vehicle.with(|v| match (v, gps.0) {
            // Z mé zastávky odhad vůz nerozjede: odjezd musí potvrdit nová poloha (jinak by
            // řádek „tady stojíš“ vypadal projetý, zatímco vůz ještě stojí).
            (Some(_), Some(VehiclePlace::AtStop(i))) if Some(i) == board => gps,
            (Some(f), Some(pl)) if fix_freshness(f.timestamp, now_ts) == FixFreshness::Fresh => {
                let (pl, p) = stop_times.with(|t| estimate_place(pl, gps.1, t, f.timestamp, now_ts));
                (Some(pl), p)
            }
            _ => gps,
        })
    });
    let est_place = Memo::new(move |_| estimated.get().0);
    let est_progress = Memo::new(move |_| estimated.get().1);
    let est_flag = Memo::new(move |_| {
        let now_ts = now_s.get();
        vehicle.with(|v| {
            v.as_ref().is_some_and(|f| fix_freshness(f.timestamp, now_ts) == FixFreshness::Fresh && is_estimate(f.timestamp, now_ts))
        })
    });
    let stale = Memo::new(move |_| {
        let now_ts = now_s.get();
        vehicle.with(|v| v.as_ref().is_some_and(|f| fix_freshness(f.timestamp, now_ts) == FixFreshness::Stale))
    });

    // Souhrn nahoře (#13): jen když víme, kde stojíš.
    let summary = {
        let stops = stops.clone();
        let coords = coords.clone();
        let trip_id = trip_id.clone();
        move || {
            let b = board?;
            let s = stops.get(b)?;
            let t = at.get();
            let delay = delays.with(|d| d.at(&trip_id, &s.gtfs_id)).unwrap_or(0);
            let minutes = (s.time + chrono::Duration::seconds(i64::from(delay)) - t).num_minutes();
            let fix = vehicle.get();
            let pl = place.get();
            let arrival = fix.as_ref().zip(pl).map(|(f, p)| arrival_state(&coords, f, p, b));
            let where_text = pl.and_then(|p| match p {
                VehiclePlace::AtStop(i) => stops.get(i).map(|s| format!("na zastávce {}", s.name)),
                VehiclePlace::Between(i) => stops.get(i).map(|s| format!("za zastávkou {}", s.name)),
            });
            let now_ts = now_s.get();
            let small = fix.as_ref().map(|f| vehicle_text(kind, f, where_text.as_deref(), now_ts, est_flag.get()));
            Some(view! {
                <div class="trip-sum" class:stale=move || stale.get()>
                    <div class="trip-sum-big">{summary_text(arrival, Some(minutes))}</div>
                    {small.map(|s| view! { <div class="trip-sum-small">{s}</div> })}
                </div>
            })
        }
    };

    // Značka vozu: piktogram v barvě linky a evidenční číslo.
    let marker = move |top: Box<dyn Fn() -> String + Send + Sync>| {
        let label = vehicle.with_untracked(|v| v.as_ref().and_then(|f| f.label.clone()));
        view! {
            <span class="veh-mark" class:stale=move || stale.get() class:est=move || est_flag.get() style=move || format!("top:{}", top())>
                <VehicleIcon kind=kind color=color />
                {label.map(|l| view! { <span class="veh-num">{l}</span> })}
            </span>
        }
        .into_any()
    };

    let body = {
        let stops = stops.clone();
        let trip_id = trip_id.clone();
        let dep_stop = row.dep.stop;
        move || {
            let t = at.get();
            let place_now = est_place.get();
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
                    // Projetá část čáry: horní polovina řádku patří předchozímu úseku, dolní dalšímu.
                    let (passed, fade_top, fade_bottom) = match place_now {
                        Some(VehiclePlace::AtStop(v)) => (i < v, i <= v && i > 0, i < v),
                        Some(VehiclePlace::Between(v)) => (i <= v, i <= v && i > 0, i <= v),
                        None => (false, false, false),
                    };
                    let is_board = board == Some(i) || (board.is_none() && s.stop == dep_stop);
                    let delay = delays.with(|d| d.at(&trip_id, &s.gtfs_id));
                    let at_stop = place_now == Some(VehiclePlace::AtStop(i));
                    out.push(
                        view! {
                            <div class="trip-stop" class:board=is_board class:passed=passed class:veh-here=at_stop>
                                {fade_top.then(|| view! { <span class="rail-fade top"></span> })}
                                {fade_bottom.then(|| view! { <span class="rail-fade bottom"></span> })}
                                {at_stop.then(|| marker(Box::new(|| "50%".to_string())))}
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
                        // Mezera mezi zastávkami, ve které vůz jede; projetá část čáry podle podílu úseku.
                        // 15–85 %, aby štítek s číslem nezasahoval do názvů zastávek nad a pod mezerou.
                        let pct = move || (15.0 + est_progress.get().clamp(0.0, 1.0) * 70.0).round();
                        out.push(
                            view! {
                                <div class="trip-gap">
                                    <span class="rail-fade" style=move || format!("top:0;height:{}%", pct())></span>
                                    {marker(Box::new(move || format!("{}%", pct())))}
                                </div>
                            }
                            .into_any(),
                        );
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
            {summary}
            <Show when=move || loaded.get() && place.get().is_none()>
                <p class="muted small">"Poloha vozu není k dispozici."</p>
            </Show>
            <div class="trip-line">{body}</div>
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
        assert_eq!(vehicle_text(RouteKind::Bus, &fix(1_000), None, 1_030, false), "autobus 8758 · před 30 s");
        assert_eq!(
            vehicle_text(RouteKind::Tram, &fix(1_000), Some("za zastávkou Pohořelec"), 1_090, false),
            "tramvaj 8758 · za zastávkou Pohořelec · před 1 min"
        );
        assert_eq!(vehicle_text(RouteKind::Bus, &fix(1_000), None, 1_180, false), "autobus 8758 · poloha před 3 min");
    }

    #[test]
    fn vehicle_text_estimate() {
        assert_eq!(vehicle_text(RouteKind::Tram, &fix(1_000), None, 1_025, true), "tramvaj 8758 · odhad, GPS před 25 s");
        assert_eq!(vehicle_text(RouteKind::Tram, &fix(1_000), None, 1_025, false), "tramvaj 8758 · před 25 s");
        // U zastaralé polohy se odhad neukazuje (#12 má přednost).
        assert_eq!(vehicle_text(RouteKind::Tram, &fix(1_000), None, 1_180, true), "tramvaj 8758 · poloha před 3 min");
    }

    #[test]
    fn summary_text_states() {
        assert_eq!(summary_text(Some(ArrivalState::StopsAway(2)), Some(3)), "za 3 min · 2 zastávky");
        assert_eq!(summary_text(Some(ArrivalState::StopsAway(1)), Some(0)), "teď · 1 zastávka");
        assert_eq!(summary_text(Some(ArrivalState::StopsAway(5)), Some(-1)), "teď · 5 zastávek");
        assert_eq!(summary_text(Some(ArrivalState::Arriving), Some(1)), "přijíždí");
        assert_eq!(summary_text(Some(ArrivalState::AtStop), Some(0)), "ve stanici");
        assert_eq!(summary_text(Some(ArrivalState::Departed), Some(-2)), "vůz už odjel");
        assert_eq!(summary_text(None, Some(5)), "za 5 min");
        assert_eq!(summary_text(None, Some(0)), "teď");
        // Bez polohy vozu a po odjezdu: ne „teď“ donekonečna (jako „před N min“ u zastávky).
        assert_eq!(summary_text(None, Some(-8)), "odjezd před 8 min");
        assert_eq!(summary_text(Some(ArrivalState::StopsAway(2)), None), "2 zastávky");
    }

    #[test]
    fn kind_word_cs() {
        assert_eq!(kind_word(RouteKind::Bus), "autobus");
        assert_eq!(kind_word(RouteKind::Tram), "tramvaj");
        assert_eq!(kind_word(RouteKind::Trolleybus), "trolejbus");
        assert_eq!(kind_word(RouteKind::Metro), "metro");
        assert_eq!(kind_word(RouteKind::Train), "vlak");
        assert_eq!(kind_word(RouteKind::Ferry), "vůz");
    }

    #[test]
    fn more() {
        assert_eq!(more_text(1), "další 1 zastávka");
        assert_eq!(more_text(3), "další 3 zastávky");
        assert_eq!(more_text(18), "dalších 18 zastávek");
    }
}
