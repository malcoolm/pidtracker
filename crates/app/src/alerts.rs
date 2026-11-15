//! Mimořádnosti: stahování, úložiště a odvozené signály (pruh, počet, ⚠ u linek).

use std::collections::HashSet;

use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::alerts::{AlertPhase, ServiceAlert, fresh_relevant, list_alerts, parse_alerts, watched_routes};
use pid_core::realtime::ALERTS_URL;
use pid_core::time::TZ;
use pid_core::timetable::query::DepartureSource;

use crate::state::AppState;
use crate::util::{keys, ls_get, ls_set, now_fixed, page_hidden};

const REFRESH_MS: u32 = 300_000;
const WATCH_DAYS: i64 = 30;

/// Zahodí opakované id (nechá první výskyt); `parse_alerts` duplicity neřeší.
pub fn dedupe(alerts: Vec<ServiceAlert>) -> Vec<ServiceAlert> {
    let mut seen = HashSet::new();
    alerts.into_iter().filter(|a| seen.insert(a.id.clone())).collect()
}

/// Aktuální čas v unix sekundách.
pub fn now_s() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}

/// Linky, na kterých uživatel jezdí (posledních 30 dní nebo právě jede).
pub fn make_watched(state: AppState) -> Memo<HashSet<String>> {
    Memo::new(move |_| {
        let ix = state.tt.get().index();
        state.rides.with(|rs| {
            watched_routes(rs, now_fixed(), WATCH_DAYS, |name| {
                let ix = ix.as_ref()?;
                let mut exact = ix.search_routes(name, 5).into_iter().filter(|h| h.1 == name);
                let first = exact.next()?;
                exact.next().is_none().then_some(first.0)
            })
        })
    })
}

/// Počet aktivních mimořádností na sledovaných linkách.
pub fn make_count(state: AppState) -> Memo<usize> {
    Memo::new(move |_| {
        let watched = state.watched.get();
        state
            .alerts
            .with(|a| list_alerts(a, &watched, now_s()).into_iter().filter(|l| l.relevant && l.phase == AlertPhase::Active).count())
    })
}

/// První **aktivní** mimořádnost dané linky.
pub fn route_alert(state: AppState, route_id: &str) -> Option<ServiceAlert> {
    let now = now_s();
    state
        .alerts
        .with(|a| a.iter().find(|a| a.phase(now) == AlertPhase::Active && a.route_ids.iter().any(|r| r == route_id)).cloned())
}

/// Název a barva linky z jízdního řádu; bez shody neutrální odznak s id linky.
pub fn route_badge_info(state: AppState, route_id: &str) -> (String, u32) {
    let found = state
        .tt
        .with_untracked(|t| t.index())
        .and_then(|ix| ix.tt.routes.iter().find(|r| r.gtfs_id == route_id).map(|r| (r.short_name.clone(), r.color)));
    found.unwrap_or_else(|| {
        let short = route_id.strip_prefix('L').filter(|r| r.starts_with(|c: char| c.is_ascii_digit())).unwrap_or(route_id);
        (short.to_string(), 0x6b6b6b)
    })
}

/// „18. 11. 10:00“ z unix sekund.
pub fn when_label(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|t| t.with_timezone(&TZ).format("%-d. %-m. %H:%M").to_string())
        .unwrap_or_default()
}

/// Stažení feedu; offline, chyba nebo prázdná odpověď → `None`.
async fn fetch_alerts() -> Option<Vec<ServiceAlert>> {
    let resp = Request::get(ALERTS_URL).send().await.ok()?;
    if !resp.ok() {
        return None;
    }
    let bytes = resp.binary().await.ok()?;
    if bytes.is_empty() {
        return None;
    }
    Some(dedupe(parse_alerts(&bytes)))
}

/// Uloží nový stav feedu a spočte nové mimořádnosti pro pruh.
fn apply(state: AppState, fetched: Vec<ServiceAlert>) {
    let ids: HashSet<String> = fetched.iter().map(|a| a.id.clone()).collect();
    match ls_get::<Vec<String>>(keys::ALERTS_SEEN) {
        // První spuštění této verze: vše dosavadní je „viděné“, bez hlášky.
        None => {
            let mut all: Vec<String> = ids.iter().cloned().collect();
            all.sort();
            ls_set(keys::ALERTS_SEEN, &all);
        }
        Some(seen) => {
            let seen: HashSet<String> = seen.into_iter().collect();
            let watched = state.watched.get_untracked();
            let fresh: Vec<String> =
                fresh_relevant(&fetched, &watched, &seen, now_s()).into_iter().map(|a| a.id.clone()).collect();
            if !fresh.is_empty() {
                state.alert_banner.update(|b| {
                    for id in &fresh {
                        if !b.contains(id) {
                            b.push(id.clone());
                        }
                    }
                });
            }
            // Prořezání na id přítomná ve feedu.
            let mut next: Vec<String> = seen.into_iter().filter(|id| ids.contains(id)).collect();
            for id in fresh {
                if !next.contains(&id) {
                    next.push(id);
                }
            }
            next.sort();
            ls_set(keys::ALERTS_SEEN, &next);
        }
    }
    state.alert_banner.update(|b| b.retain(|id| ids.contains(id)));
    ls_set(keys::ALERTS, &fetched);
    state.alerts.set(fetched);
    state.alerts_known.set(true);
}

async fn refresh(state: AppState) {
    if let Some(fetched) = fetch_alerts().await {
        apply(state, fetched);
    }
}

/// Stahování při startu, při návratu do popředí a každých 300 s (jen při viditelné stránce).
pub fn install(state: AppState) {
    spawn_local(async move {
        loop {
            if !page_hidden() {
                refresh(state).await;
            }
            TimeoutFuture::new(REFRESH_MS).await;
        }
    });
    Effect::new(move |prev: Option<u32>| {
        let f = state.foreground.get();
        if prev.is_some_and(|p| p != f) && !page_hidden() {
            spawn_local(refresh(state));
        }
        f
    });
}
