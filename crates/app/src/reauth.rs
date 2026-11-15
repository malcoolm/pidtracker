//! Tiché obnovení Google tokenu při klepnutí uživatele (popup musí vzniknout v rámci gesta).

use std::cell::Cell;

use leptos::prelude::GetUntracked;
use pid_core::reauth::{RefreshInput, should_refresh};
use wasm_bindgen_futures::spawn_local;

use crate::google::auth::{sign_in_with, token_expires_at_ms};
use crate::state::AppState;

thread_local! {
    static IN_FLIGHT: Cell<bool> = const { Cell::new(false) };
    static LAST_ATTEMPT: Cell<Option<f64>> = const { Cell::new(None) };
}

pub fn install(state: AppState) {
    crate::interop::on_user_tap(move || {
        let me = state.me.get_untracked();
        let now_ms = js_sys::Date::now();
        let online = web_sys::window().map(|w| w.navigator().on_line()).unwrap_or(true);
        let input = RefreshInput {
            now_ms,
            expires_at_ms: token_expires_at_ms(),
            signed_in_before: me.is_some(),
            online,
            in_flight: IN_FLIGHT.with(Cell::get),
            last_attempt_ms: LAST_ATTEMPT.with(Cell::get),
        };
        if !should_refresh(&input) {
            return;
        }
        LAST_ATTEMPT.with(|c| c.set(Some(now_ms)));
        IN_FLIGHT.with(|c| c.set(true));
        // Popup se otevře synchronně tady, v obsluze klepnutí; čeká až future.
        let fut = sign_in_with(me.as_deref(), true);
        spawn_local(async move {
            // Selhání je tiché: zůstává ruční tlačítko „Přihlásit pro synchronizaci“.
            if fut.await.is_ok() {
                crate::sync::request_sync(state);
            }
            IN_FLIGHT.with(|c| c.set(false));
        });
    });
}
