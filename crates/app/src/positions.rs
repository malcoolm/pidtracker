//! Sdílená poloha vozů (#14): jeden feed `vehicle_positions.pb` pro celou aplikaci.
//!
//! Obrazovka, která polohu potřebuje, zavolá [`subscribe`]; dokud je přihlášená aspoň jedna
//! a stránka je vidět, stahuje se každých 20 s. Bez přihlášených se nestahuje nic
//! (feed má ~500 kB). Při chybě zůstávají poslední data.

use std::collections::HashMap;
use std::sync::Arc;

use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::realtime::{VEHICLE_POSITIONS_URL, VehicleFix, parse_vehicle_fixes, should_fetch_positions};

use crate::state::AppState;
use crate::util::page_hidden;

/// Jak často se kontroluje, zda je čas stáhnout.
const CHECK_MS: u32 = 5_000;

/// Polohy vozů z posledního úspěšného stažení.
#[derive(Clone)]
pub struct Positions {
    /// `trip_id` → poloha vozu.
    pub fixes: Arc<HashMap<String, VehicleFix>>,
    /// Kdy se stáhly (`Date.now()`, ms).
    pub fetched_ms: f64,
}

/// Stáří dat v ms (`None` = zatím nic).
pub fn age_ms(state: AppState) -> Option<f64> {
    state.positions.with_untracked(|p| p.as_ref().map(|p| js_sys::Date::now() - p.fetched_ms))
}

/// Stáhne feed, pokud to pravidlo dovolí.
fn try_fetch(state: AppState) {
    let go = should_fetch_positions(
        state.positions_subs.get_untracked(),
        page_hidden(),
        state.positions_in_flight.get_untracked().map(|since| js_sys::Date::now() - since),
        age_ms(state),
    );
    if !go {
        return;
    }
    let started = js_sys::Date::now();
    state.positions_in_flight.set(Some(started));
    spawn_local(async move {
        if let Some(fixes) = fetch().await {
            state.positions.set(Some(Positions { fixes: Arc::new(fixes), fetched_ms: js_sys::Date::now() }));
        }
        // Zaseknuté stahování, které doběhne pozdě, nesmí uvolnit novější běžící stahování.
        if state.positions_in_flight.get_untracked() == Some(started) {
            state.positions_in_flight.set(None);
        }
    });
}

async fn fetch() -> Option<HashMap<String, VehicleFix>> {
    let resp = Request::get(VEHICLE_POSITIONS_URL).send().await.ok()?;
    if !resp.ok() {
        return None;
    }
    let bytes = resp.binary().await.ok()?;
    Some(parse_vehicle_fixes(&bytes))
}

/// Přihlásí volající komponentu k odběru polohy do jejího zániku. Hned stáhne, jsou-li data
/// starší než 20 s.
pub fn subscribe(state: AppState) {
    state.positions_subs.update(|n| *n += 1);
    on_cleanup(move || state.positions_subs.update(|n| *n = n.saturating_sub(1)));
    try_fetch(state);
}

/// Kontrola každých 5 s a hned po návratu do popředí.
pub fn install(state: AppState) {
    spawn_local(async move {
        loop {
            TimeoutFuture::new(CHECK_MS).await;
            try_fetch(state);
        }
    });
    Effect::new(move |prev: Option<u32>| {
        let f = state.foreground.get();
        if prev.is_some_and(|p| p != f) {
            try_fetch(state);
        }
        f
    });
}
