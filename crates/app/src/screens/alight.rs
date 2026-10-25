//! Výstup: zastávky po trase spoje (podle polohy), okolí, vyhledávání; Hotovo / Přestoupit.

use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::ride::RideStatus;
use pid_core::timetable::query::{DepartureSource, StopRef};

use crate::interop::{Fix, current_position};
use crate::screens::board::GroupPicker;
use crate::state::{AppState, Screen};
use crate::util::now_fixed;

#[derive(Clone, PartialEq)]
enum Choice {
    OnRoute(StopRef),
    Group(u32),
    Manual,
}

#[component]
pub fn Alight() -> impl IntoView {
    let state = expect_context::<AppState>();
    let Some(ride) = state.current_ride() else {
        state.screen.set(Screen::Home);
        return ().into_any();
    };
    let fix = RwSignal::new(None::<Fix>);
    let geo_msg = RwSignal::new(None::<String>);
    let locating = RwSignal::new(true);
    let choice = RwSignal::new(None::<Choice>);
    let picked_group = RwSignal::new(None::<u32>);
    let manual_stop = RwSignal::new(String::new());
    let alight_time = now_fixed();

    spawn_local(async move {
        match current_position().await {
            Ok(f) => fix.set(Some(f)),
            Err(e) => geo_msg.set(Some(e.message().into())),
        }
        locating.set(false);
    });

    let trip_id = ride.trip_id.clone();
    let board_stop_id = ride.board_stop_id.clone();
    let on_route = Memo::new(move |_| {
        let Some(ix) = state.tt.get().index() else { return Vec::new() };
        let (Some(trip), Some(stop)) = (ix.trip_by_gtfs_id(&trip_id), ix.stop_by_gtfs_id(&board_stop_id)) else {
            return Vec::new();
        };
        let f = fix.get();
        ix.downstream_stops(trip, stop, f.map(|f| f.lat), f.map(|f| f.lon))
    });
    let nearby = Signal::derive(move || {
        let (Some(ix), Some(f)) = (state.tt.get().index(), fix.get()) else { return Vec::new() };
        ix.nearby_groups(f.lat, f.lon, f.accuracy_m)
    });
    // Předvýběr: nejbližší zastávka na trase, jinak nejbližší v okolí.
    Effect::new(move |_| {
        let route = on_route.get();
        let near = nearby.get();
        if fix.get().is_none() && locating.get() {
            return;
        }
        if choice.get_untracked().is_none() {
            if let Some(first) = route.first() {
                choice.set(Some(Choice::OnRoute(first.clone())));
            } else if let Some(g) = near.first() {
                picked_group.set(Some(g.group));
                choice.set(Some(Choice::Group(g.group)));
            } else if state.tt.get_untracked().index().is_none() {
                choice.set(Some(Choice::Manual));
            }
        }
    });

    let finish = move |transfer: bool| {
        let Some(mut r) = state.current_ride() else { return };
        let ix = state.tt.get_untracked().index();
        let mut next_group = None;
        match choice.get_untracked() {
            Some(Choice::OnRoute(s)) => {
                r.alight_stop_id = Some(s.gtfs_id.clone());
                r.alight_stop_name = Some(s.name.clone());
                next_group = ix.as_ref().map(|ix| ix.group_of_stop(s.stop));
            }
            Some(Choice::Group(g)) => {
                if let Some(ix) = &ix {
                    r.alight_stop_id = Some(ix.tt.stops[ix.group_first_stop(g) as usize].gtfs_id.clone());
                    r.alight_stop_name = Some(ix.tt.group_name(g).to_string());
                }
                next_group = Some(g);
            }
            Some(Choice::Manual) | None => {
                let name = manual_stop.get_untracked().trim().to_string();
                r.alight_stop_name = (!name.is_empty()).then_some(name);
            }
        }
        let f = fix.get_untracked();
        r.alight_lat = f.map(|f| f.lat);
        r.alight_lon = f.map(|f| f.lon);
        r.alight_time = Some(alight_time);
        r.status = RideStatus::Done;
        r.updated_at = now_fixed();
        state.save_ride(r);
        state.set_active_ride(None);
        state.screen.set(if transfer { Screen::Board { group: next_group } } else { Screen::Home });
    };

    let route_view = move || {
        on_route
            .get()
            .into_iter()
            .take(8)
            .map(|s| {
                let s2 = s.clone();
                let selected = {
                    let s = s.clone();
                    move || choice.get() == Some(Choice::OnRoute(s.clone()))
                };
                view! {
                    <button class="chip" class:selected=selected on:click=move |_| { picked_group.set(None); choice.set(Some(Choice::OnRoute(s2.clone()))); }>
                        {s.name.clone()}
                        {s.distance_m.map(|d| view! { <small>{format!(" · {d:.0} m")}</small> })}
                    </button>
                }
            })
            .collect_view()
    };

    view! {
        <section class="alight">
            <div class="screen-head">
                <h2>"Výstup"</h2>
                <button class="link" on:click=move |_| state.screen.set(Screen::Home)>"Zpět"</button>
            </div>
            <p class="muted">{format!("Linka {} z {}", ride.route_short_name, ride.board_stop_name)}</p>
            <Show when=move || locating.get()>
                <p class="muted">"Zjišťuji polohu…"</p>
            </Show>
            {move || geo_msg.get().map(|m| view! { <p class="warn">{m}</p> })}
            <Show when=move || !on_route.get().is_empty()>
                <h3>"Na trase"</h3>
                <div class="chips">{route_view}</div>
            </Show>
            <Show
                when=move || state.tt.get().index().is_some()
                fallback=move || view! {
                    <label>"Výstupní zastávka"
                        <input type="text" placeholder="Název zastávky"
                            prop:value=move || manual_stop.get()
                            on:input=move |ev| { manual_stop.set(event_target_value(&ev)); choice.set(Some(Choice::Manual)); } />
                    </label>
                }
            >
                <h3>"Jiné zastávky v okolí"</h3>
                <GroupPicker
                    candidates=nearby
                    selected=picked_group
                    on_pick=Callback::new(move |g| choice.set(Some(Choice::Group(g))))
                />
            </Show>
            <div class="actions sticky">
                <button class="primary huge" on:click=move |_| finish(false)>"Hotovo"</button>
                <button class="secondary big" on:click=move |_| finish(true)>"Přestoupit"</button>
            </div>
        </section>
    }
    .into_any()
}
