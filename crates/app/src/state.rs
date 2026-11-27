//! Globální stav aplikace sdílený přes Leptos context.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::alerts::ServiceAlert;
use pid_core::ride::{Ride, RideStatus, RouteKind};
use pid_core::timetable::query::TimetableIndex;

use crate::interop::Fix;
use crate::util::{keys, ls_get, ls_get_str, ls_remove, ls_set, ls_set_str};
use crate::{storage, sync};

#[derive(Clone)]
pub enum TtState {
    Loading,
    Ready(Arc<TimetableIndex>),
    Missing(String),
}

impl TtState {
    pub fn index(&self) -> Option<Arc<TimetableIndex>> {
        match self {
            TtState::Ready(ix) => Some(ix.clone()),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Screen {
    Home,
    /// Vyhledání spojení (bez probíhající jízdy je i obsahem úvodní obrazovky).
    Search,
    /// Výběr zastávky a linky. `group` je předvybraná skupina (přestup).
    Board { group: Option<u32> },
    Alight,
    History,
    Edit(String),
    Stats,
    Settings,
    /// Mimořádnosti; `open` je předvybraná mimořádnost (Task 10).
    Alerts { open: Option<String> },
}

#[derive(Clone, Debug, PartialEq)]
pub enum SyncStatus {
    Idle { last: Option<String> },
    Running,
    Pending(u32),
    NeedsSignIn(u32),
    Error(String),
}

#[derive(Clone, Copy)]
pub struct AppState {
    pub tt: RwSignal<TtState>,
    pub rides: RwSignal<Vec<Ride>>,
    pub me: RwSignal<Option<String>>,
    pub sheet_id: RwSignal<Option<String>>,
    pub nicknames: RwSignal<HashMap<String, String>>,
    pub sync: RwSignal<SyncStatus>,
    pub screen: RwSignal<Screen>,
    pub active_ride: RwSignal<Option<String>>,
    pub toast: RwSignal<Option<String>>,
    /// Poslední úspěšně zjištěná poloha (i z dřívějšího spuštění).
    pub last_fix: RwSignal<Option<Fix>>,
    /// Zvýší se při každém návratu aplikace do popředí.
    pub foreground: RwSignal<u32>,
    /// Menu (hamburger) je otevřené.
    pub menu_open: RwSignal<bool>,
    /// Mimořádnosti z posledního feedu (nebo uložené z minula).
    pub alerts: RwSignal<Vec<ServiceAlert>>,
    /// Feed už byl někdy získán (jinak „offline“ místo „žádné mimořádnosti“).
    pub alerts_known: RwSignal<bool>,
    /// Id nových mimořádností pro pruh pod proužkem.
    pub alert_banner: RwSignal<Vec<String>>,
    /// Linky, na kterých jezdím (30 dní).
    pub watched: Memo<HashSet<String>>,
    /// Počet aktivních relevantních mimořádností.
    pub alert_count: Memo<usize>,
    /// Rezerva před prvním nástupem při vyhledávání spojení (0–5 min).
    pub planner_buffer_min: RwSignal<u8>,
    /// Druhy dopravy vyloučené z vyhledávání spojení.
    pub planner_excluded: RwSignal<Vec<RouteKind>>,
    /// Sdílená poloha vozů (viz `positions`).
    pub positions: RwSignal<Option<crate::positions::Positions>>,
    /// Kolik obrazovek polohu vozů právě potřebuje.
    pub positions_subs: RwSignal<u32>,
    /// Feed poloh se právě stahuje.
    pub positions_in_flight: RwSignal<bool>,
}

impl AppState {
    pub fn new() -> Self {
        let mut s = AppState {
            tt: RwSignal::new(TtState::Loading),
            rides: RwSignal::new(Vec::new()),
            me: RwSignal::new(ls_get_str(keys::ME)),
            sheet_id: RwSignal::new(Some(crate::config::SHEET_ID.to_string())),
            nicknames: RwSignal::new(ls_get(keys::NICKNAMES).unwrap_or_default()),
            sync: RwSignal::new(SyncStatus::Idle { last: ls_get_str(keys::LAST_SYNC) }),
            screen: RwSignal::new(Screen::Home),
            active_ride: RwSignal::new(ls_get_str(keys::ACTIVE_RIDE)),
            toast: RwSignal::new(None),
            last_fix: RwSignal::new(ls_get(keys::LAST_FIX)),
            foreground: RwSignal::new(0),
            menu_open: RwSignal::new(false),
            alerts: RwSignal::new(crate::alerts::dedupe(ls_get(keys::ALERTS).unwrap_or_default())),
            alerts_known: RwSignal::new(ls_get_str(keys::ALERTS).is_some()),
            alert_banner: RwSignal::new(Vec::new()),
            watched: Memo::new(|_| HashSet::new()),
            alert_count: Memo::new(|_| 0usize),
            planner_buffer_min: RwSignal::new(ls_get::<u8>(keys::PLANNER_BUFFER_MIN).unwrap_or(2).min(5)),
            planner_excluded: RwSignal::new(ls_get(keys::PLANNER_EXCLUDED).unwrap_or_default()),
            positions: RwSignal::new(None),
            positions_subs: RwSignal::new(0),
            positions_in_flight: RwSignal::new(false),
        };
        // Odvozené hodnoty potřebují hotový stav.
        s.watched = crate::alerts::make_watched(s);
        s.alert_count = crate::alerts::make_count(s);
        s
    }

    pub fn ride(&self, id: &str) -> Option<Ride> {
        self.rides.with(|rs| rs.iter().find(|r| r.id == id).cloned())
    }

    /// Probíhající jízda přihlášeného uživatele.
    pub fn current_ride(&self) -> Option<Ride> {
        let id = self.active_ride.get()?;
        self.ride(&id).filter(|r| r.status == RideStatus::InProgress && !r.deleted)
    }

    pub fn set_active_ride(&self, id: Option<String>) {
        match &id {
            Some(id) => ls_set_str(keys::ACTIVE_RIDE, id),
            None => ls_remove(keys::ACTIVE_RIDE),
        }
        self.active_ride.set(id);
    }

    pub fn nickname(&self, email: &str) -> String {
        self.nicknames
            .with(|n| n.get(email).filter(|s| !s.is_empty()).cloned())
            .unwrap_or_else(|| email.split('@').next().unwrap_or(email).to_string())
    }

    /// Zapíše jízdu lokálně (signál + IndexedDB + outbox) a požádá o synchronizaci.
    /// Nikdy nečeká na síť ani přihlášení.
    pub fn save_ride(&self, ride: Ride) {
        self.rides.update(|rs| match rs.iter_mut().find(|r| r.id == ride.id) {
            Some(existing) => *existing = ride.clone(),
            None => rs.push(ride.clone()),
        });
        let state = *self;
        spawn_local(async move {
            match storage::put_ride(&ride, true).await {
                Ok(()) => sync::request_sync(state),
                Err(e) => state.toast.set(Some(format!("Uložení selhalo: {}", e.0))),
            }
        });
    }

    pub fn remember_fix(&self, f: Fix) {
        ls_set(keys::LAST_FIX, &f);
        self.last_fix.set(Some(f));
    }

    /// Poslední známá poloha jako (lat, lon).
    pub fn last_pos(&self) -> Option<(f64, f64)> {
        self.last_fix.get().map(|f| (f.lat, f.lon))
    }

    /// Nastaví a uloží rezervu před prvním nástupem (ořezáno na 0–5 min).
    pub fn set_planner_buffer(&self, min: u8) {
        let min = min.min(5);
        ls_set(keys::PLANNER_BUFFER_MIN, &min);
        self.planner_buffer_min.set(min);
    }

    /// Nastaví a uloží vyloučené druhy dopravy pro vyhledávání spojení.
    pub fn set_planner_excluded(&self, kinds: Vec<RouteKind>) {
        ls_set(keys::PLANNER_EXCLUDED, &kinds);
        self.planner_excluded.set(kinds);
    }

    pub fn show_toast(&self, msg: impl Into<String>) {
        self.toast.set(Some(msg.into()));
    }
}
