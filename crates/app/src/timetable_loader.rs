//! Načtení jízdního řádu: `data/manifest.json` → `data/<file>` (cache service workeru).

use std::sync::Arc;

use gloo_net::http::Request;
use pid_core::timetable::codec::decode;
use pid_core::timetable::query::TimetableIndex;
use serde::Deserialize;

use crate::interop::purge_data_cache_except;

#[derive(Deserialize)]
struct Manifest {
    file: String,
}

#[derive(Debug, Clone)]
pub enum LoadError {
    /// Jízdní řád není k dispozici (offline bez cache, build bez dat).
    NoData(String),
    Corrupt(String),
}

impl LoadError {
    pub fn message(&self) -> String {
        match self {
            LoadError::NoData(m) => format!("Jízdní řád není k dispozici ({m}). Zastávky a linky zadejte ručně."),
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
    let tt = decode(&bytes).map_err(|e| LoadError::Corrupt(e.to_string()))?;
    purge_data_cache_except(&manifest.file).await;
    Ok(Arc::new(TimetableIndex::new(tt)))
}
