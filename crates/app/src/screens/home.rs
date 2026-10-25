//! Úvodní obrazovka: nová jízda, nebo probíhající jízda s tlačítkem Výstup.

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::ride::Ride;

use crate::components::{DataBanner, RideRow, RouteBadge, ride_color};
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
    let state = expect_context::<AppState>();
    let recent = move || {
        let mut rides: Vec<Ride> = state.rides.get().into_iter().filter(|r| !r.deleted).collect();
        rides.sort_by_key(|r| std::cmp::Reverse(r.board_time));
        rides.truncate(3);
        rides
    };
    view! {
        <section class="home">
            <button class="primary huge" on:click=move |_| state.screen.set(Screen::Board { group: None })>
                "Nová jízda"
            </button>
            <Show when=move || !recent().is_empty()>
                <h3>"Poslední jízdy"</h3>
                <div class="list">
                    <For each=recent key=|r| (r.id.clone(), r.updated_at) let:ride>
                        <RideRow ride=ride />
                    </For>
                </div>
            </Show>
        </section>
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
                                r.vehicle = event_target_value(&ev).trim().to_string();
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
        </section>
    }
}
