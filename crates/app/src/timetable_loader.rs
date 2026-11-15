//! Načtení jízdního řádu: `data/manifest.json` → `data/<file>` (cache service workeru).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gloo_net::http::Request;
use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::timetable::codec::{CodecError, decode};
use pid_core::timetable::query::TimetableIndex;
use serde::Deserialize;

use crate::interop::purge_data_cache_except;
use crate::state::{AppState, TtState};

/// Prefix všech verzí formátu (`PIDT2`, `PIDT3`, …).
const MAGIC_FAMILY: &[u8] = b"PIDT";

/// Právě běží načítání (jeden pokus naráz).
static IN_FLIGHT: AtomicBool = AtomicBool::new(false);

#[derive(Deserialize)]
struct Manifest {
    file: String,
}

#[derive(Debug, Clone)]
pub enum LoadError {
    /// Jízdní řád není k dispozici (offline bez cache, build bez dat).
    NoData(String),
    /// Jízdní řád jiné verze formátu (např. starý soubor z cache po aktualizaci).
    OldFormat,
    Corrupt(String),
}

impl LoadError {
    pub fn message(&self) -> String {
        match self {
            LoadError::NoData(m) => format!("Jízdní řád není k dispozici ({m}). Zastávky a linky zadejte ručně."),
            LoadError::OldFormat => "Jízdní řád je ve starém formátu, stáhne se po připojení.".into(),
            LoadError::Corrupt(m) => format!("Jízdní řád je poškozený ({m}). Zastávky a linky zadejte ručně."),
        }
    }
}

pub async fn load() -> Result<Arc<TimetableIndex>, LoadError> {
    let manifest: Manifest = Request::get("data/manifest.json")
        .send()
        .await
        .map_err(|e| LoadError::NoData(e.to_string()))?
        .json()
        .await
        .map_err(|e| LoadError::NoData(e.to_string()))?;
    let resp = Request::get(&format!("data/{}", manifest.file))
        .send()
        .await
        .map_err(|e| LoadError::NoData(e.to_string()))?;
    if !resp.ok() {
        return Err(LoadError::NoData(format!("HTTP {}", resp.status())));
    }
    let bytes = resp.binary().await.map_err(|e| LoadError::NoData(e.to_string()))?;
    let tt = decode(&bytes).map_err(|e| match e {
        CodecError::BadMagic if bytes.starts_with(MAGIC_FAMILY) => LoadError::OldFormat,
        e => LoadError::Corrupt(e.to_string()),
    })?;
    purge_data_cache_except(&manifest.file).await;
    Ok(Arc::new(TimetableIndex::new(tt)))
}

/// Jeden pokus o načtení do `state.tt` (nic, pokud už jeden běží).
fn attempt(state: AppState) {
    if IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return;
    }
    spawn_local(async move {
        let tt = match load().await {
            Ok(ix) => TtState::Ready(ix),
            Err(e) => TtState::Missing(e.message()),
        };
        state.tt.set(tt);
        IN_FLIGHT.store(false, Ordering::Release);
    });
}

/// Načte jízdní řád při startu; chybí-li, zkusí to znovu při návratu do popředí
/// a při připojení k síti (jeden pokus na událost, žádné opakování ve smyčce).
pub fn install(state: AppState) {
    attempt(state);
    let retry = move || {
        if matches!(state.tt.get_untracked(), TtState::Missing(_)) {
            attempt(state);
        }
    };
    crate::interop::on_visible(retry);
    crate::interop::on_online(retry);
}
