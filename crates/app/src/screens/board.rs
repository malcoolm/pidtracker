//! Nástup: zastávka z polohy (nebo vyhledání), linka z jízdního řádu (nebo ručně), Start.

use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::ride::{Ride, RideStatus, RouteKind};
use pid_core::timetable::query::{Departure, DepartureSource, NearbyGroup, next_per_direction};

use crate::components::{DataBanner, RouteBadge};
use crate::config::BUILD_ID;
use crate::interop::{Fix, current_position};
use crate::state::{AppState, Screen};
use crate::util::{css_color, kind_color, now, now_fixed};

const BEFORE_MIN: i64 = 5;
const AFTER_MIN: i64 = 30;

/// Výběr zastávky: kandidáti z okolí, nebo výsledky vyhledávání.
#[component]
pub fn GroupPicker(
    candidates: Signal<Vec<NearbyGroup>>,
    selected: RwSignal<Option<u32>>,
    #[prop(optional)] on_pick: Option<Callback<u32>>,
) -> impl IntoView {
    let state = expect_context::<AppState>();
    let query = RwSignal::new(String::new());
    let shown = move || {
        let q = query.get();
        if q.trim().is_empty() {
            candidates.get()
        } else {
            state.tt.get().index().map(|ix| ix.search_groups(&q, 8)).unwrap_or_default()
        }
    };
    view! {
        <div class="picker">
            <input
                type="search"
                placeholder="Hledat zastávku…"
                prop:value=move || query.get()
                on:input=move |ev| query.set(event_target_value(&ev))
            />
            <div class="chips">
                <For each=shown key=|g| g.group let:g>
                    {
                        let id = g.group;
                        view! {
                            <button
                                class="chip"
                                class:selected=move || selected.get() == Some(id)
                                on:click=move |_| {
                                    selected.set(Some(id));
                                    query.set(String::new());
                                    if let Some(cb) = on_pick { cb.run(id); }
                                }
                            >
                                {g.name.clone()}
                                {(g.distance_m > 0.0).then(|| view! { <small>{format!(" · {:.0} m", g.distance_m)}</small> })}
                            </button>
                        }
                    }
                </For>
            </div>
        </div>
    }
}

#[derive(Clone, PartialEq)]
enum LineChoice {
    Departure(usize),
    Manual,
}

#[component]
pub fn Board(group: Option<u32>) -> impl IntoView {
    let state = expect_context::<AppState>();
    let fix = RwSignal::new(None::<Fix>);
    let geo_msg = RwSignal::new(None::<String>);
    let locating = RwSignal::new(group.is_none());
    let selected_group = RwSignal::new(group);
    let at = RwSignal::new(now());
    let choice = RwSignal::new(LineChoice::Departure(0));
    let manual_line = RwSignal::new(String::new());
    let manual_kind = RwSignal::new(None::<u16>);
    let manual_stop = RwSignal::new(String::new());
    let manual_stop_open = RwSignal::new(false);

    if group.is_none() {
        spawn_local(async move {
            match current_position().await {
                Ok(f) => fix.set(Some(f)),
                Err(e) => geo_msg.set(Some(e.message().into())),
            }
            locating.set(false);
        });
    }

    let candidates = Signal::derive(move || {
        let Some(ix) = state.tt.get().index() else { return Vec::new() };
        match (fix.get(), group) {
            (Some(f), _) => ix.nearby_groups(f.lat, f.lon, f.accuracy_m),
            // Přestup: ukázat předvybranou zastávku, i když polohu nezjišťujeme.
            (None, Some(g)) => vec![NearbyGroup { group: g, name: ix.tt.group_name(g).to_string(), distance_m: 0.0 }],
            (None, None) => Vec::new(),
        }
    });
    Effect::new(move |_| {
        if selected_group.get_untracked().is_none()
            && let Some(first) = candidates.get().first() {
                selected_group.set(Some(first.group));
            }
    });

    let departures: Memo<Vec<Departure>> = Memo::new(move |_| {
        let (Some(ix), Some(g)) = (state.tt.get().index(), selected_group.get()) else { return Vec::new() };
        next_per_direction(ix.departures(g, at.get(), BEFORE_MIN, AFTER_MIN))
    });
    Effect::new(move |_| {
        departures.track();
        if departures.with_untracked(|d| d.is_empty()) {
            choice.set(LineChoice::Manual);
        } else if choice.get_untracked() == LineChoice::Manual && manual_line.get_untracked().is_empty() {
            choice.set(LineChoice::Departure(0));
        }
    });

    let has_tt = move || state.tt.get().index().is_some();
    let route_suggestions = move || {
        let q = manual_line.get();
        state.tt.get().index().map(|ix| ix.search_routes(&q, 6)).unwrap_or_default()
    };

    let can_start = move || {
        let stop_ok = selected_group.get().is_some() || !manual_stop.get().trim().is_empty();
        let line_ok = match choice.get() {
            LineChoice::Departure(i) => departures.with(|d| i < d.len()),
            LineChoice::Manual => !manual_line.get().trim().is_empty(),
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
        let mut ride = Ride {
            id: uuid::Uuid::new_v4().to_string(),
            person: me,
            board_stop_id: String::new(),
            board_stop_name: manual_stop.get_untracked().trim().to_string(),
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
        };
        let manual = !ride.board_stop_name.is_empty();
        if let (false, Some(ix), Some(g)) = (manual, &ix, selected_group.get_untracked()) {
            ride.board_stop_name = ix.tt.group_name(g).to_string();
            ride.board_stop_id = ix.tt.stops[ix.group_first_stop(g) as usize].gtfs_id.clone();
        }
        match choice.get_untracked() {
            LineChoice::Departure(i) => {
                let d = departures.with_untracked(|d| d[i].clone());
                if let (false, Some(ix)) = (manual, &ix) {
                    ride.board_stop_id = ix.tt.stops[d.stop as usize].gtfs_id.clone();
                }
                ride.route_id = d.route_id;
                ride.route_short_name = d.route_short_name;
                ride.route_type = Some(d.route_type);
                ride.headsign = d.headsign;
                ride.trip_id = d.trip_gtfs_id;
                ride.pattern_id = Some(d.pattern);
            }
            LineChoice::Manual => {
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
        state.set_active_ride(Some(ride.id.clone()));
        state.save_ride(ride);
        state.screen.set(Screen::Home);
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
            <Show
                when=has_tt
                fallback=move || view! {
                    <label>"Zastávka"
                        <input type="text" placeholder="Název zastávky"
                            prop:value=move || manual_stop.get()
                            on:input=move |ev| manual_stop.set(event_target_value(&ev)) />
                    </label>
                }
            >
                <GroupPicker
                    candidates=candidates
                    selected=selected_group
                    on_pick=Callback::new(move |_| {
                        at.set(now());
                        manual_stop.set(String::new());
                        manual_stop_open.set(false);
                        choice.set(LineChoice::Departure(0));
                    })
                />
                <Show
                    when=move || manual_stop_open.get()
                    fallback=move || view! {
                        <button class="link small" on:click=move |_| manual_stop_open.set(true)>"Zastávka není v seznamu? Zadat ručně"</button>
                    }
                >
                    <input type="text" placeholder="Název zastávky"
                        prop:value=move || manual_stop.get()
                        on:input=move |ev| {
                            manual_stop.set(event_target_value(&ev));
                            selected_group.set(None);
                        } />
                </Show>
            </Show>

            <h3>"Linka"</h3>
            <div class="list">
                <For
                    each={move || departures.get().into_iter().enumerate().collect::<Vec<_>>()}
                    key=|(i, d)| (*i, d.trip, d.stop)
                    let:item
                >
                    {
                        let (i, d) = item;
                        let mins = (d.time - now()).num_minutes();
                        let when = if mins <= 0 { "teď".to_string() } else { format!("za {mins} min") };
                        view! {
                            <button
                                class="dep-row"
                                class:selected=move || choice.get() == LineChoice::Departure(i)
                                on:click=move |_| choice.set(LineChoice::Departure(i))
                            >
                                <RouteBadge name=d.route_short_name.clone() color=d.color />
                                <span class="dep-head">{d.headsign.clone()}</span>
                                <span class="dep-time">{d.time.format("%H:%M").to_string()} <small>{when}</small></span>
                            </button>
                        }
                    }
                </For>
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
