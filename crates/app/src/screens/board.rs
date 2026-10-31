//! Nástup: zastávka z polohy (nebo vyhledání), linka z jízdního řádu (nebo ručně), Start.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::DateTime;
use chrono_tz::Tz;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::realtime::Delays;
use pid_core::ride::{Ride, RideStatus, RouteKind};
use pid_core::timetable::query::{BoardRow, DepartureSource, NearbyGroup, board_rows, pick_row};

use crate::components::{DataBanner, RouteBadge};
use crate::config::BUILD_ID;
use crate::interop::{Fix, current_position};
use crate::state::{AppState, Screen};
use crate::stop_input::StopInput;
use crate::util::{css_color, kind_color, now, now_fixed};

/// Okno dotazu do jízdního řádu (zpožděné spoje mohou mít čas v minulosti).
const BEFORE_MIN: i64 = 15;
/// „Právě odjelo“ – jak daleko do minulosti.
const RECENT_MIN: i64 = 5;
const AFTER_MIN: i64 = 30;
const TICK_MS: u32 = 30_000;
const DELAYS_REFRESH_MS: u32 = 60_000;

#[derive(Clone, PartialEq)]
enum LineChoice {
    /// Nejbližší nadcházející spoj (výchozí).
    Auto,
    /// Ručně vybraný spoj; drží se, i když ze seznamu zmizí.
    Trip(BoardRow),
    Manual,
}

/// Okolí přestupní zastávky (stejný uzel může mít víc jmen, např. Můstek / Václavské náměstí).
const TRANSFER_RADIUS_M: f64 = 150.0;

fn row_key(r: &BoardRow) -> (u32, u32) {
    (r.dep.trip, r.dep.stop)
}

/// „+2 min“ / „−1 min“; zpoždění pod minutu se neukazuje.
fn delay_label(delay_s: Option<i32>) -> Option<(String, &'static str)> {
    let d = delay_s?;
    if d.abs() < 60 {
        return None;
    }
    let mins = (d as f64 / 60.0).round() as i64;
    Some(if mins > 0 { (format!("+{mins} min"), "delay late") } else { (format!("−{} min", -mins), "delay early") })
}

#[component]
fn DepRow(row: BoardRow, at: DateTime<Tz>, choice: RwSignal<LineChoice>, selected: Signal<Option<(u32, u32)>>) -> impl IntoView {
    let key = row_key(&row);
    let picked = row.clone();
    let mins = (row.expected - at).num_minutes();
    let when = if mins <= 0 { "teď".to_string() } else { format!("za {mins} min") };
    let delay = delay_label(row.delay_s);
    let d = row.dep;
    view! {
        <button
            class="dep-row"
            class:selected=move || selected.get() == Some(key)
            on:click=move |_| choice.set(LineChoice::Trip(picked.clone()))
        >
            <RouteBadge name=d.route_short_name.clone() color=d.color />
            <span class="dep-head">{d.headsign.clone()}</span>
            <span class="dep-time">
                {d.time.format("%H:%M").to_string()}
                " "
                <small>
                    {when}
                    {delay.map(|(text, class)| view! { " · " <span class=class>{text}</span> })}
                </small>
            </span>
        </button>
    }
}

#[component]
pub fn Board(group: Option<u32>) -> impl IntoView {
    let state = expect_context::<AppState>();
    let fix = RwSignal::new(None::<Fix>);
    let geo_msg = RwSignal::new(None::<String>);
    let locating = RwSignal::new(group.is_none());
    let selected_group = RwSignal::new(group);
    let stop_text = RwSignal::new(String::new());
    let at = RwSignal::new(now());
    let delays = RwSignal::new(Delays::default());
    let choice = RwSignal::new(LineChoice::Auto);
    let show_recent = RwSignal::new(false);
    let manual_line = RwSignal::new(String::new());
    let manual_kind = RwSignal::new(None::<u16>);

    if group.is_none() {
        spawn_local(async move {
            match current_position().await {
                Ok(f) => {
                    fix.set(Some(f));
                    state.remember_fix(f);
                }
                Err(e) => geo_msg.set(Some(e.message().into())),
            }
            locating.set(false);
        });
    }

    // Čas tabule se posouvá a zpoždění se obnovují, dokud je obrazovka otevřená.
    let alive = Arc::new(AtomicBool::new(true));
    {
        let alive = alive.clone();
        on_cleanup(move || alive.store(false, Ordering::Relaxed));
    }
    {
        let alive = alive.clone();
        spawn_local(async move {
            loop {
                TimeoutFuture::new(TICK_MS).await;
                if !alive.load(Ordering::Relaxed) {
                    break;
                }
                at.set(now());
            }
        });
    }
    spawn_local(async move {
        loop {
            let fetched = crate::realtime::fetch_delays().await;
            if !alive.load(Ordering::Relaxed) {
                break;
            }
            if let Some(d) = fetched {
                delays.set(d);
            }
            TimeoutFuture::new(DELAYS_REFRESH_MS).await;
            if !alive.load(Ordering::Relaxed) {
                break;
            }
        }
    });

    let candidates = Signal::derive(move || {
        let Some(ix) = state.tt.get().index() else { return Vec::new() };
        match (fix.get(), group) {
            (Some(f), _) => ix.nearby_groups(f.lat, f.lon, f.accuracy_m),
            // Přestup: předvybraná zastávka a vedle ní okolí (bez zjišťování polohy).
            (None, Some(g)) => {
                let (lat, lon) = ix.group_coords(g);
                let mut out = vec![NearbyGroup { group: g, name: ix.tt.group_name(g).to_string(), distance_m: 0.0 }];
                out.extend(ix.nearby_groups(lat, lon, TRANSFER_RADIUS_M).into_iter().filter(|n| n.group != g));
                out
            }
            (None, None) => Vec::new(),
        }
    });
    let near = Signal::derive(move || fix.get().map(|f| (f.lat, f.lon)).or_else(|| state.last_pos()));
    Effect::new(move |_| {
        if selected_group.get_untracked().is_none()
            && stop_text.get_untracked().trim().is_empty()
            && let Some(first) = candidates.get().first() {
                selected_group.set(Some(first.group));
            }
    });

    let rows: Memo<(Vec<BoardRow>, Vec<BoardRow>)> = Memo::new(move |_| {
        let (Some(ix), Some(g)) = (state.tt.get().index(), selected_group.get()) else { return Default::default() };
        let t = at.get();
        delays.with(|d| board_rows(ix.departures(g, t, BEFORE_MIN, AFTER_MIN), d, t, RECENT_MIN))
    });
    // Vybraný řádek: ručně zvolený spoj (i když už ze seznamu zmizel), jinak nejbližší nadcházející.
    let selected_row = Memo::new(move |_| {
        rows.with(|(up, recent)| match choice.get() {
            LineChoice::Manual => None,
            LineChoice::Trip(r) => pick_row(up, recent, Some(&r)),
            LineChoice::Auto => pick_row(up, recent, None),
        })
    });
    let selected_key = Signal::derive(move || selected_row.with(|r| r.as_ref().map(row_key)));
    // Zvolený spoj, který už v seznamech není – ukáže se nahoře, ať je výběr vidět.
    let vanished = Memo::new(move |_| {
        let sel = selected_row.get()?;
        rows.with(|(up, recent)| up.iter().chain(recent).all(|r| row_key(r) != row_key(&sel)).then_some(sel))
    });
    // Ručně jen při změně „žádné odjezdy“ ↔ „jsou odjezdy“, ne při každém obnovení tabule.
    Effect::new(move |was_empty: Option<bool>| {
        let empty = rows.with(|(up, recent)| up.is_empty() && recent.is_empty());
        if was_empty != Some(empty) {
            if empty {
                choice.set(LineChoice::Manual);
            } else if was_empty == Some(true)
                && choice.get_untracked() == LineChoice::Manual
                && manual_line.get_untracked().is_empty()
            {
                choice.set(LineChoice::Auto);
            }
        }
        empty
    });

    let has_tt = move || state.tt.get().index().is_some();
    let route_suggestions = move || {
        let q = manual_line.get();
        state.tt.get().index().map(|ix| ix.search_routes(&q, 6)).unwrap_or_default()
    };

    let can_start = move || {
        let stop_ok = selected_group.get().is_some() || !stop_text.get().trim().is_empty();
        let line_ok = match choice.get() {
            LineChoice::Manual => !manual_line.get().trim().is_empty(),
            _ => selected_row.with(|r| r.is_some()),
        };
        stop_ok && line_ok
    };

    let started = RwSignal::new(false);
    let start = move |_| {
        // Ochrana proti dvojitému klepnutí (dvě probíhající jízdy).
        if started.get_untracked() {
            return;
        }
        let Some(me) = state.me.get_untracked() else { return };
        started.set(true);
        let ix = state.tt.get_untracked().index();
        let t = now_fixed();
        let f = fix.get_untracked();
        let group = selected_group.get_untracked();
        let mut ride = Ride {
            id: uuid::Uuid::new_v4().to_string(),
            person: me,
            board_stop_id: String::new(),
            board_stop_name: stop_text.get_untracked().trim().to_string(),
            route_id: String::new(),
            route_short_name: String::new(),
            route_type: None,
            headsign: String::new(),
            trip_id: String::new(),
            pattern_id: None,
            board_time: t,
            alight_stop_id: None,
            alight_stop_name: None,
            alight_time: None,
            board_lat: f.map(|f| f.lat),
            board_lon: f.map(|f| f.lon),
            alight_lat: None,
            alight_lon: None,
            status: RideStatus::InProgress,
            deleted: false,
            created_at: t,
            updated_at: t,
            app_version: BUILD_ID.to_string(),
            data_version: ix.as_ref().map(|ix| ix.version().to_string()).unwrap_or_default(),
            vehicle: String::new(),
            operator: String::new(),
            license_plate: String::new(),
        };
        if let (Some(ix), Some(g)) = (&ix, group) {
            ride.board_stop_name = ix.tt.group_name(g).to_string();
            ride.board_stop_id = ix.tt.stops[ix.group_first_stop(g) as usize].gtfs_id.clone();
        }
        let row = match choice.get_untracked() {
            LineChoice::Manual => None,
            _ => selected_row.get_untracked(),
        };
        match row {
            Some(r) => {
                let d = r.dep;
                if group.is_some() {
                    ride.board_stop_id = d.stop_gtfs_id.clone();
                }
                ride.route_id = d.route_id;
                ride.route_short_name = d.route_short_name;
                ride.route_type = Some(d.route_type);
                ride.headsign = d.headsign;
                ride.trip_id = d.trip_gtfs_id;
                ride.operator = d.operator;
                ride.pattern_id = Some(d.pattern);
            }
            None => {
                let name = manual_line.get_untracked().trim().to_string();
                let found = ix.as_ref().and_then(|ix| ix.search_routes(&name, 1).into_iter().find(|r| r.1 == name));
                match found {
                    Some((id, short, rtype)) => {
                        ride.route_id = id;
                        ride.route_short_name = short;
                        ride.route_type = Some(rtype);
                    }
                    None => {
                        ride.route_short_name = name;
                        ride.route_type = manual_kind.get_untracked();
                    }
                }
            }
        }
        let id = ride.id.clone();
        state.set_active_ride(Some(id.clone()));
        state.save_ride(ride);
        state.screen.set(Screen::Home);
        spawn_local(crate::realtime::fill_vehicle(state, id));
    };

    view! {
        <section class="board">
            <DataBanner />
            <div class="screen-head">
                <h2>"Nástup"</h2>
                <button class="link" on:click=move |_| state.screen.set(Screen::Home)>"Zpět"</button>
            </div>
            <Show when=move || locating.get()>
                <p class="muted">"Zjišťuji polohu…"</p>
            </Show>
            {move || fix.get().map(|f| view! { <p class="muted small">{format!("Přesnost polohy ±{:.0} m", f.accuracy_m)}</p> })}
            {move || geo_msg.get().map(|m| view! { <p class="warn">{m}</p> })}
            <StopInput
                text=stop_text
                selected=selected_group
                near=near
                candidates=candidates
                placeholder="Hledat zastávku nebo napsat název…"
                on_pick=Callback::new(move |_| {
                    at.set(now());
                    choice.set(LineChoice::Auto);
                })
            />

            <h3>"Linka"</h3>
            <div class="list">
                {move || {
                    let t = at.get();
                    vanished
                        .get()
                        .into_iter()
                        .chain(rows.get().0)
                        .map(|r| view! { <DepRow row=r at=t choice=choice selected=selected_key /> })
                        .collect_view()
                }}
                <Show when=move || rows.with(|(_, recent)| !recent.is_empty())>
                    <button class="link small" on:click=move |_| show_recent.update(|v| *v = !*v)>
                        {move || format!("{} Právě odjelo ({})", if show_recent.get() { "▾" } else { "▸" }, rows.with(|(_, r)| r.len()))}
                    </button>
                    <Show when=move || show_recent.get()>
                        {move || {
                            let t = at.get();
                            rows.get().1.into_iter().map(|r| view! { <DepRow row=r at=t choice=choice selected=selected_key /> }).collect_view()
                        }}
                    </Show>
                </Show>
                <button
                    class="dep-row"
                    class:selected=move || choice.get() == LineChoice::Manual
                    on:click=move |_| choice.set(LineChoice::Manual)
                >
                    "Jiná linka…"
                </button>
            </div>
            <Show when=move || choice.get() == LineChoice::Manual>
                <div class="manual">
                    <input type="text" placeholder="Číslo linky (např. 22, A, S9)" list="route-suggestions"
                        prop:value=move || manual_line.get()
                        on:input=move |ev| manual_line.set(event_target_value(&ev)) />
                    <datalist id="route-suggestions">
                        <For each=route_suggestions key=|r| r.0.clone() let:r>
                            <option value=r.1.clone()>{RouteKind::from_gtfs(r.2).label_cs()}</option>
                        </For>
                    </datalist>
                    <Show when=move || !has_tt() || route_suggestions().iter().all(|r| r.1 != manual_line.get().trim())>
                        <div class="kinds">
                            {RouteKind::ALL.into_iter().filter_map(|k| k.to_gtfs().map(|t| (k, t))).map(|(k, t)| view! {
                                <button class="chip" class:selected=move || manual_kind.get() == Some(t)
                                    style=format!("--kind:{}", css_color(kind_color(Some(t))))
                                    on:click=move |_| manual_kind.set(Some(t))>{k.label_cs()}</button>
                            }).collect_view()}
                        </div>
                    </Show>
                </div>
            </Show>

            <div class="actions sticky">
                <button class="primary huge" disabled=move || !can_start() on:click=start>"Start"</button>
            </div>
        </section>
    }
}
