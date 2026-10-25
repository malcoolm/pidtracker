//! Automatická aktualizace na novou verzi z GitHub Pages.
//!
//! `version.json` se kontroluje při startu, při návratu do popředí a každých 15 minut.
//! Nová verze se načte (reload) jen na bezpečné obrazovce, nikdy uprostřed vyplňování.

use std::cell::Cell;

use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Deserialize;

use crate::config::{APP_VERSION, BUILD_ID};
use crate::state::{AppState, Screen};
use crate::util::{keys, ls_get_str, ls_set_str, reload};

const CHECK_INTERVAL_MS: u32 = 15 * 60 * 1000;

thread_local! {
    static PENDING_UPDATE: Cell<bool> = const { Cell::new(false) };
}

#[derive(Deserialize)]
struct VersionInfo {
    build_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateCheck {
    UpToDate,
    Available(String),
    Offline,
}

/// Obrazovky, na kterých lze aplikaci bez rizika přenačíst.
pub fn is_safe(state: &AppState) -> bool {
    match state.screen.get_untracked() {
        Screen::Home => state.current_ride().is_none(),
        Screen::History | Screen::Stats | Screen::Settings => true,
        Screen::Board { .. } | Screen::Alight | Screen::Edit(_) => false,
    }
}

pub async fn check_now() -> UpdateCheck {
    if BUILD_ID == "dev" {
        return UpdateCheck::UpToDate;
    }
    let url = format!("version.json?t={}", js_sys::Date::now() as u64);
    let Ok(resp) = Request::get(&url).send().await else { return UpdateCheck::Offline };
    let Ok(info) = resp.json::<VersionInfo>().await else { return UpdateCheck::Offline };
    if info.build_id == BUILD_ID {
        UpdateCheck::UpToDate
    } else {
        crate::interop::sw_update().await;
        PENDING_UPDATE.with(|p| p.set(true));
        UpdateCheck::Available(info.build_id)
    }
}

/// Přenačte aplikaci, pokud čeká aktualizace a obrazovka je bezpečná.
pub fn apply_if_safe(state: &AppState) {
    if PENDING_UPDATE.with(|p| p.get()) && is_safe(state) {
        reload();
    }
}

pub fn install(state: AppState) {
    // Hláška po aktualizaci.
    let previous = ls_get_str(keys::LAST_SEEN_BUILD);
    if previous.as_deref().is_some_and(|p| p != BUILD_ID) {
        state.show_toast(format!("Aktualizováno na verzi {APP_VERSION} ({BUILD_ID})"));
    }
    ls_set_str(keys::LAST_SEEN_BUILD, BUILD_ID);

    let check = move || {
        spawn_local(async move {
            check_now().await;
            apply_if_safe(&state);
        })
    };
    crate::interop::on_visible(check);
    spawn_local(async move {
        loop {
            check_now().await;
            apply_if_safe(&state);
            TimeoutFuture::new(CHECK_INTERVAL_MS).await;
        }
    });
    // Při každé změně obrazovky zkusit čekající aktualizaci.
    Effect::new(move |_| {
        state.screen.track();
        state.active_ride.track();
        apply_if_safe(&state);
    });
}
