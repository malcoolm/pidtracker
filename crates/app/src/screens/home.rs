//! Úvodní obrazovka: vyhledání spojení a nová jízda, nebo probíhající jízda s tlačítkem Výstup.

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::ride::{Ride, on_vehicle_changed};

use crate::components::{DataBanner, RouteBadge, ride_color};
use crate::screens::Search;
use crate::season::XmasScene;
use crate::state::{AppState, Screen};
use crate::util::{hhmm, now_fixed};

#[component]
pub fn Home() -> impl IntoView {
    let state = expect_context::<AppState>();
    view! {
        <DataBanner />
        {move || match state.current_ride() {
            Some(ride) => view! { <Running ride=ride /> }.into_any(),
            None => view! { <Idle /> }.into_any(),
        }}
    }
}

#[component]
fn Idle() -> impl IntoView {
    view! {
        <section class="home">
            <Search />
            <XmasScene />
            <NewRideFab />
        </section>
    }
}

/// Obrazovka Spojení z menu: během probíhající jízdy bez tlačítka Nová jízda.
#[component]
pub fn SearchScreen() -> impl IntoView {
    let state = expect_context::<AppState>();
    view! {
        <DataBanner />
        <section class="home">
            <Search />
            <XmasScene />
            <Show when=move || state.current_ride().is_none()>
                <NewRideFab />
            </Show>
        </section>
    }
}

#[component]
fn NewRideFab() -> impl IntoView {
    let state = expect_context::<AppState>();
    view! {
        <button class="fab" on:click=move |_| state.screen.set(Screen::Board { group: None })>"＋ Nová jízda"</button>
    }
}

#[component]
fn Running(ride: Ride) -> impl IntoView {
    let state = expect_context::<AppState>();
    let elapsed = RwSignal::new(String::new());
    let alive = RwSignal::new(true);
    let board_time = ride.board_time;
    let tick = move || {
        let secs = (now_fixed() - board_time).num_seconds().max(0);
        elapsed.set(if secs >= 3600 {
            format!("{}:{:02}:{:02}", secs / 3600, secs % 3600 / 60, secs % 60)
        } else {
            format!("{:02}:{:02}", secs / 60, secs % 60)
        });
    };
    tick();
    spawn_local(async move {
        while alive.try_get_untracked().unwrap_or(false) {
            TimeoutFuture::new(1_000).await;
            if alive.try_get_untracked().unwrap_or(false) {
                tick();
            }
        }
    });
    on_cleanup(move || {
        let _ = alive.try_set(false);
    });

    let confirm_cancel = RwSignal::new(false);
    let cancel_ride = {
        let ride = ride.clone();
        move |_| {
            if !confirm_cancel.get() {
                confirm_cancel.set(true);
                return;
            }
            let mut r = ride.clone();
            r.deleted = true;
            r.updated_at = now_fixed();
            state.save_ride(r);
            state.set_active_ride(None);
            state.show_toast("Jízda zrušena");
        }
    };
    let color = ride_color(&state, &ride);

    view! {
        <section class="running">
            <div class="running-head">
                <RouteBadge name=ride.route_short_name.clone() color=color />
                <span class="headsign">{(!ride.headsign.is_empty()).then(|| format!("→ {}", ride.headsign))}</span>
            </div>
            <p class="from">"Z " <strong>{ride.board_stop_name.clone()}</strong> " v " {hhmm(&ride.board_time)}</p>
            <p class="elapsed">{move || elapsed.get()}</p>
            <label class="vehicle">"Číslo vozu"
                <input type="text" inputmode="numeric" placeholder="např. 9355"
                    prop:value=ride.vehicle.clone()
                    on:change={
                        let id = ride.id.clone();
                        move |ev| {
                            if let Some(mut r) = state.ride(&id) {
                                let vehicle = event_target_value(&ev).trim().to_string();
                                if vehicle == r.vehicle {
                                    return;
                                }
                                r.vehicle = vehicle;
                                state.rides.with_untracked(|all| on_vehicle_changed(&mut r, all));
                                r.updated_at = now_fixed();
                                state.save_ride(r);
                            }
                        }
                    } />
            </label>
            <label class="vehicle">"SPZ"
                <input type="text" autocapitalize="characters" placeholder="např. 1AB 2345"
                    prop:value={
                        let id = ride.id.clone();
                        move || state.ride(&id).map(|r| r.license_plate).unwrap_or_default()
                    }
                    on:change={
                        let id = ride.id.clone();
                        move |ev| {
                            if let Some(mut r) = state.ride(&id) {
                                r.license_plate = event_target_value(&ev).trim().to_string();
                                r.updated_at = now_fixed();
                                state.save_ride(r);
                            }
                        }
                    } />
            </label>
            <div class="actions">
                <button class="primary huge" on:click=move |_| state.screen.set(Screen::Alight)>"Výstup"</button>
                <button class="link danger" on:click=cancel_ride>
                    {move || if confirm_cancel.get() { "Opravdu zrušit? Klepněte znovu" } else { "Zrušit jízdu" }}
                </button>
            </div>
            <XmasScene />
        </section>
    }
}
