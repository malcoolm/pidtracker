//! Synchronizace s Google Sheets: pull → sloučení → push fronty změn.
//!
//! Běží „na pozadí“, kdykoli je aplikace otevřená: při startu, při návratu do popředí,
//! po připojení k síti, po každé lokální změně a periodicky, dokud něco čeká na odeslání.

use std::cell::Cell;
use std::collections::HashMap;

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::ride::Ride;
use pid_core::sheet::{HEADERS, PEOPLE_HEADERS, PEOPLE_SHEET, RIDES_SHEET, parse_rows, people_rows, to_row};
use pid_core::sync::{SheetWrite, merge_remote, plan_push, update_range};

use crate::google::auth::{current_token, drop_token};
use crate::google::sheets::{SheetsClient, SheetsError};
use crate::state::{AppState, SyncStatus};
use crate::storage;
use crate::util::{hhmm, keys, ls_set, ls_set_str, now_fixed};

const DEBOUNCE_MS: u32 = 2_000;
const RETRY_INTERVAL_MS: u32 = 60_000;

thread_local! {
    static RUNNING: Cell<bool> = const { Cell::new(false) };
    static RERUN: Cell<bool> = const { Cell::new(false) };
    static GENERATION: Cell<u32> = const { Cell::new(0) };
    /// Listy a hlavičky stačí ověřit jednou za běh aplikace.
    static ENSURED: Cell<bool> = const { Cell::new(false) };
}

/// Naplánuje synchronizaci (debounce 2 s; souběžně běží nejvýš jedna).
pub fn request_sync(state: AppState) {
    let generation = GENERATION.with(|g| {
        g.set(g.get().wrapping_add(1));
        g.get()
    });
    spawn_local(async move {
        TimeoutFuture::new(DEBOUNCE_MS).await;
        if GENERATION.with(|g| g.get()) == generation {
            sync_now(state).await;
        }
    });
}

/// Nastaví spouštěče synchronizace. Volá se jednou po startu.
pub fn install_triggers(state: AppState) {
    crate::interop::on_visible(move || request_sync(state));
    crate::interop::on_online(move || request_sync(state));
    spawn_local(async move {
        loop {
            TimeoutFuture::new(RETRY_INTERVAL_MS).await;
            if matches!(state.sync.get_untracked(), SyncStatus::Pending(_) | SyncStatus::Error(_)) {
                sync_now(state).await;
            }
        }
    });
    request_sync(state);
}

pub async fn sync_now(state: AppState) {
    if state.sheet_id.get_untracked().is_none() {
        return;
    }
    if RUNNING.with(|r| r.replace(true)) {
        RERUN.with(|r| r.set(true));
        return;
    }
    loop {
        RERUN.with(|r| r.set(false));
        let result = run_once(state).await;
        apply_result(state, result).await;
        if !RERUN.with(|r| r.get()) {
            break;
        }
    }
    RUNNING.with(|r| r.set(false));
}

async fn pending_count() -> u32 {
    storage::pending_ids().await.map(|p| p.len() as u32).unwrap_or(0)
}

async fn apply_result(state: AppState, result: Result<(), SheetsError>) {
    let status = match result {
        Ok(()) => {
            let n = pending_count().await;
            if n > 0 {
                SyncStatus::Pending(n)
            } else {
                let last = hhmm(&now_fixed());
                ls_set_str(keys::LAST_SYNC, &last);
                SyncStatus::Idle { last: Some(last) }
            }
        }
        Err(SheetsError::Unauthorized) => {
            drop_token();
            SyncStatus::NeedsSignIn(pending_count().await)
        }
        Err(SheetsError::Network(_)) => SyncStatus::Pending(pending_count().await),
        Err(e) => SyncStatus::Error(e.message()),
    };
    state.sync.set(status);
}

async fn run_once(state: AppState) -> Result<(), SheetsError> {
    let Some(sheet_id) = state.sheet_id.get_untracked() else {
        return Err(SheetsError::Other("Není nastavená tabulka.".into()));
    };
    let Some(token) = current_token() else {
        state.sync.set(SyncStatus::NeedsSignIn(pending_count().await));
        return Err(SheetsError::Unauthorized);
    };
    if state.me.get_untracked().is_none() {
        state.me.set(Some(token.email.clone()));
    }
    state.sync.set(SyncStatus::Running);
    let client = SheetsClient::new(&token.access_token, &sheet_id);
    if !ENSURED.with(|e| e.get()) {
        client.ensure_sheets(&[(RIDES_SHEET, &HEADERS), (PEOPLE_SHEET, &PEOPLE_HEADERS)]).await?;
        ENSURED.with(|e| e.set(true));
    }

    // Pull a sloučení (na čerstvém snímku lokálních jízd, bez await mezi čtením a zápisem).
    let values = client.get_values(&format!("{RIDES_SHEET}!A:ZZ")).await?;
    let parsed = parse_rows(&values);
    let pending = storage::pending_ids().await.map_err(|e| SheetsError::Other(e.0))?;
    let mut local: HashMap<String, Ride> =
        state.rides.get_untracked().into_iter().map(|r| (r.id.clone(), r)).collect();
    let before = local.clone();
    merge_remote(&mut local, &parsed.rides, &pending);
    let changed: Vec<Ride> = local.values().filter(|r| before.get(&r.id) != Some(*r)).cloned().collect();
    if !changed.is_empty() {
        state.rides.set(local.values().cloned().collect());
    }
    storage::put_rides_bulk(&changed).await.map_err(|e| SheetsError::Other(e.0))?;

    // Push fronty.
    let to_push: Vec<Ride> = pending.iter().filter_map(|id| local.get(id).cloned()).collect();
    let writes = plan_push(&parsed.rides, &to_push);
    let mut updates = Vec::new();
    let mut appends = Vec::new();
    for w in &writes {
        match w {
            SheetWrite::Update { row, ride } => updates.push((update_range(*row), vec![to_row(ride)])),
            SheetWrite::Append { ride } => appends.push(to_row(ride)),
        }
    }
    client.batch_update_values(updates).await?;
    client.append(RIDES_SHEET, appends).await?;
    for ride in &to_push {
        let _ = storage::clear_pending_if_unchanged(&ride.id, &ride.updated_at).await;
    }

    // Lidé a přezdívky.
    let people = people_rows(&client.get_values(&format!("{PEOPLE_SHEET}!A:B")).await?);
    let me = token.email.to_lowercase();
    if !people.iter().any(|(_, email, _)| email.eq_ignore_ascii_case(&me)) {
        client.append(PEOPLE_SHEET, vec![vec![me.clone(), String::new()]]).await?;
    }
    let nicknames: HashMap<String, String> =
        people.into_iter().map(|(_, email, nick)| (email.to_lowercase(), nick)).collect();
    ls_set(keys::NICKNAMES, &nicknames);
    state.nicknames.set(nicknames);
    Ok(())
}

/// Uloží přezdívku přihlášeného uživatele do listu `people`.
pub async fn save_nickname(state: AppState, nickname: String) -> Result<(), SheetsError> {
    let (Some(sheet_id), Some(token)) = (state.sheet_id.get_untracked(), current_token()) else {
        return Err(SheetsError::Unauthorized);
    };
    let client = SheetsClient::new(&token.access_token, &sheet_id);
    let me = token.email.to_lowercase();
    let people = people_rows(&client.get_values(&format!("{PEOPLE_SHEET}!A:B")).await?);
    match people.iter().find(|(_, email, _)| email.eq_ignore_ascii_case(&me)) {
        Some((row, _, _)) => {
            client
                .batch_update_values(vec![(format!("{PEOPLE_SHEET}!A{row}:B{row}"), vec![vec![me.clone(), nickname.clone()]])])
                .await?
        }
        None => client.append(PEOPLE_SHEET, vec![vec![me.clone(), nickname.clone()]]).await?,
    }
    state.nicknames.update(|n| {
        n.insert(me, nickname);
    });
    ls_set(keys::NICKNAMES, &state.nicknames.get_untracked());
    Ok(())
}
