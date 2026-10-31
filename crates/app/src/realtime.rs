//! Realtime feed PID: číslo vozu a zpoždění (best effort, nikdy neblokuje záznam jízdy).

use gloo_net::http::Request;
use leptos::prelude::WithUntracked;
use pid_core::ride::apply_known_plate;
use pid_core::realtime::{Delays, TRIP_UPDATES_URL, VEHICLE_POSITIONS_URL, parse_trip_delays, vehicle_for_trip};

use crate::state::AppState;
use crate::util::now_fixed;

/// Pokud jízda ještě nemá číslo vozu, zkusí ho najít podle spoje v realtime feedu.
/// Offline nebo bez shody nic nemění (číslo jde vždy zadat ručně).
pub async fn fill_vehicle(state: AppState, ride_id: String) {
    let Some(ride) = state.ride(&ride_id) else { return };
    if !ride.vehicle.is_empty() || ride.trip_id.is_empty() {
        return;
    }
    let Ok(resp) = Request::get(VEHICLE_POSITIONS_URL).send().await else { return };
    if !resp.ok() {
        return;
    }
    let Ok(bytes) = resp.binary().await else { return };
    let Some(vehicle) = vehicle_for_trip(&bytes, &ride.trip_id) else { return };
    // Mezitím mohl uživatel číslo zadat ručně – to má přednost.
    let Some(mut current) = state.ride(&ride_id) else { return };
    if current.vehicle.is_empty() {
        current.vehicle = vehicle;
        state.rides.with_untracked(|all| apply_known_plate(&mut current, all));
        current.updated_at = now_fixed();
        state.save_ride(current);
    }
}

/// Aktuální zpoždění jedoucích spojů. Offline nebo chyba → `None`.
pub async fn fetch_delays() -> Option<Delays> {
    let resp = Request::get(TRIP_UPDATES_URL).send().await.ok()?;
    if !resp.ok() {
        return None;
    }
    let bytes = resp.binary().await.ok()?;
    Some(parse_trip_delays(&bytes))
}
