//! IndexedDB: jízdy a fronta změn k odeslání (outbox).

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use chrono::{DateTime, FixedOffset};
use pid_core::ride::Ride;
use rexie::{ObjectStore, Rexie, TransactionMode};
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;

const DB_NAME: &str = "pidtracker";
const RIDES: &str = "rides";
const OUTBOX: &str = "outbox";

#[derive(Debug, Clone)]
pub struct StorageError(pub String);

impl From<rexie::Error> for StorageError {
    fn from(e: rexie::Error) -> Self {
        StorageError(e.to_string())
    }
}

impl From<serde_wasm_bindgen::Error> for StorageError {
    fn from(e: serde_wasm_bindgen::Error) -> Self {
        StorageError(e.to_string())
    }
}

#[derive(Serialize, Deserialize)]
struct OutboxEntry {
    id: String,
    updated_at: String,
}

thread_local! {
    static DB: RefCell<Option<Rc<Rexie>>> = const { RefCell::new(None) };
}

async fn db() -> Result<Rc<Rexie>, StorageError> {
    if let Some(db) = DB.with(|d| d.borrow().clone()) {
        return Ok(db);
    }
    let db = Rexie::builder(DB_NAME)
        .version(1)
        .add_object_store(ObjectStore::new(RIDES).key_path("id"))
        .add_object_store(ObjectStore::new(OUTBOX).key_path("id"))
        .build()
        .await?;
    let db = Rc::new(db);
    DB.with(|d| *d.borrow_mut() = Some(db.clone()));
    Ok(db)
}

fn to_js<T: Serialize>(v: &T) -> Result<JsValue, StorageError> {
    Ok(v.serialize(&serde_wasm_bindgen::Serializer::json_compatible())?)
}

pub async fn all_rides() -> Result<Vec<Ride>, StorageError> {
    let db = db().await?;
    let tx = db.transaction(&[RIDES], TransactionMode::ReadOnly)?;
    let values = tx.store(RIDES)?.get_all(None, None).await?;
    tx.done().await?;
    Ok(values.into_iter().filter_map(|v| serde_wasm_bindgen::from_value(v).ok()).collect())
}

/// Uloží jízdu; s `mark_pending` ji zároveň zařadí k odeslání (jedna transakce).
pub async fn put_ride(r: &Ride, mark_pending: bool) -> Result<(), StorageError> {
    let db = db().await?;
    let tx = db.transaction(&[RIDES, OUTBOX], TransactionMode::ReadWrite)?;
    tx.store(RIDES)?.put(&to_js(r)?, None).await?;
    if mark_pending {
        let entry = OutboxEntry { id: r.id.clone(), updated_at: r.updated_at.to_rfc3339() };
        tx.store(OUTBOX)?.put(&to_js(&entry)?, None).await?;
    }
    tx.done().await?;
    Ok(())
}

/// Uloží jízdy stažené z tabulky (bez zařazení k odeslání).
pub async fn put_rides_bulk(rides: &[Ride]) -> Result<(), StorageError> {
    if rides.is_empty() {
        return Ok(());
    }
    let db = db().await?;
    let tx = db.transaction(&[RIDES], TransactionMode::ReadWrite)?;
    let store = tx.store(RIDES)?;
    for r in rides {
        store.put(&to_js(r)?, None).await?;
    }
    tx.done().await?;
    Ok(())
}

pub async fn pending_ids() -> Result<HashSet<String>, StorageError> {
    let db = db().await?;
    let tx = db.transaction(&[OUTBOX], TransactionMode::ReadOnly)?;
    let values = tx.store(OUTBOX)?.get_all(None, None).await?;
    tx.done().await?;
    Ok(values
        .into_iter()
        .filter_map(|v| serde_wasm_bindgen::from_value::<OutboxEntry>(v).ok())
        .map(|e| e.id)
        .collect())
}

/// Odstraní jízdu z fronty, jen pokud se mezitím znovu nezměnila.
pub async fn clear_pending_if_unchanged(id: &str, updated_at: &DateTime<FixedOffset>) -> Result<(), StorageError> {
    let db = db().await?;
    let tx = db.transaction(&[OUTBOX], TransactionMode::ReadWrite)?;
    let store = tx.store(OUTBOX)?;
    if let Some(v) = store.get(JsValue::from_str(id)).await? {
        let entry: OutboxEntry = serde_wasm_bindgen::from_value(v)?;
        if entry.updated_at == updated_at.to_rfc3339() {
            store.delete(JsValue::from_str(id)).await?;
        }
    }
    tx.done().await?;
    Ok(())
}
