//! Globální stav aplikace sdílený přes Leptos context.

use std::collections::HashMap;
use std::sync::Arc;

use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::ride::{Ride, RideStatus};
use pid_core::timetable::query::TimetableIndex;

use crate::util::{keys, ls_get, ls_get_str, ls_remove, ls_set_str};
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
    /// Výběr zastávky a linky. `group` je předvybraná skupina (přestup).
    Board { group: Option<u32> },
    Alight,
    History,
    Edit(String),
    Stats,
    Settings,
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
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            tt: RwSignal::new(TtState::Loading),
            rides: RwSignal::new(Vec::new()),
            me: RwSignal::new(ls_get_str(keys::ME)),
            sheet_id: RwSignal::new(ls_get_str(keys::SHEET_ID)),
            nicknames: RwSignal::new(ls_get(keys::NICKNAMES).unwrap_or_default()),
            sync: RwSignal::new(SyncStatus::Idle { last: ls_get_str(keys::LAST_SYNC) }),
            screen: RwSignal::new(Screen::Home),
            active_ride: RwSignal::new(ls_get_str(keys::ACTIVE_RIDE)),
            toast: RwSignal::new(None),
        }
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

    pub fn show_toast(&self, msg: impl Into<String>) {
        self.toast.set(Some(msg.into()));
    }
}
