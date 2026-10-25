//! Doplnění čísla vozu z realtime feedu PID (best effort, nikdy neblokuje záznam jízdy).

use gloo_net::http::Request;
use pid_core::realtime::{VEHICLE_POSITIONS_URL, vehicle_for_trip};

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
        current.updated_at = now_fixed();
        state.save_ride(current);
    }
}
