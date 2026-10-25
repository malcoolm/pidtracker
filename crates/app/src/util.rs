//! Drobné pomůcky: čas, localStorage, formátování.

use chrono::{DateTime, FixedOffset};
use chrono_tz::Tz;
use pid_core::time::{TZ, now_prague};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub fn now() -> DateTime<Tz> {
    now_prague(js_sys::Date::now())
}

pub fn now_fixed() -> DateTime<FixedOffset> {
    now().fixed_offset()
}

pub fn local(t: &DateTime<FixedOffset>) -> DateTime<Tz> {
    t.with_timezone(&TZ)
}

pub fn hhmm(t: &DateTime<FixedOffset>) -> String {
    local(t).format("%H:%M").to_string()
}

/// „25 min“, „1 h 05 min“
pub fn duration_label(minutes: i64) -> String {
    if minutes < 60 {
        format!("{minutes} min")
    } else {
        format!("{} h {:02} min", minutes / 60, minutes % 60)
    }
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

pub fn ls_get_str(key: &str) -> Option<String> {
    storage()?.get_item(key).ok()?
}

pub fn ls_set_str(key: &str, value: &str) {
    if let Some(s) = storage() {
        let _ = s.set_item(key, value);
    }
}

pub fn ls_remove(key: &str) {
    if let Some(s) = storage() {
        let _ = s.remove_item(key);
    }
}

pub fn ls_get<T: DeserializeOwned>(key: &str) -> Option<T> {
    serde_json::from_str(&ls_get_str(key)?).ok()
}

pub fn ls_set<T: Serialize>(key: &str, value: &T) {
    if let Ok(s) = serde_json::to_string(value) {
        ls_set_str(key, &s);
    }
}

/// Klíče v localStorage.
pub mod keys {
    pub const TOKEN: &str = "pt.token";
    pub const ME: &str = "pt.me";
    pub const NICKNAMES: &str = "pt.nicknames";
    pub const ACTIVE_RIDE: &str = "pt.active_ride_id";
    pub const LAST_SEEN_BUILD: &str = "pt.last_seen_build";
    pub const LAST_SYNC: &str = "pt.last_sync";
}

pub fn reload() {
    if let Some(w) = web_sys::window() {
        let _ = w.location().reload();
    }
}

/// Barva linky jako CSS hex.
pub fn css_color(color: u32) -> String {
    format!("#{color:06x}")
}

/// Standardní barvy PID podle typu dopravy (pro jízdy bez barvy z jízdního řádu).
pub fn kind_color(route_type: Option<u16>) -> u32 {
    match route_type {
        Some(0) => 0x7a0603,
        Some(1) => 0x00a562,
        Some(2) => 0x0a4a8f,
        Some(4) => 0x00b2e3,
        Some(7) => 0xc5aa00,
        Some(11) => 0x80166f,
        _ => 0x007da8,
    }
}
