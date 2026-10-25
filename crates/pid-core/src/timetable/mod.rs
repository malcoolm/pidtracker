//! Kompaktní jízdní řád předpočítaný z PID GTFS.

pub mod codec;
pub mod query;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Na zastávce nelze nastoupit (GTFS pickup_type = 1).
pub const NO_PICKUP: u8 = 1;
/// Na zastávce nelze vystoupit (GTFS drop_off_type = 1).
pub const NO_DROP_OFF: u8 = 2;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Timetable {
    /// `<feed_start YYYYMMDD>-<vygenerováno YYYYMMDDHHMM>`
    pub version: String,
    pub feed_start: NaiveDate,
    pub feed_end: NaiveDate,
    /// Internované řetězce (názvy zastávek, headsigny, označení nástupišť).
    pub strings: Vec<String>,
    pub stops: Vec<Stop>,
    pub groups: Vec<StopGroup>,
    pub routes: Vec<Route>,
    pub patterns: Vec<Pattern>,
    pub profiles: Vec<Profile>,
    pub trips: Vec<Trip>,
    pub services: Vec<Service>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stop {
    pub gtfs_id: String,
    pub name: u32,
    pub platform: Option<u32>,
    pub lat_e6: i32,
    pub lon_e6: i32,
    pub group: u32,
}

/// Skupina stanovišť stejného uzlu (asw_node_id, jinak stejný název).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StopGroup {
    pub name: u32,
    pub stops: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Route {
    pub gtfs_id: String,
    pub short_name: String,
    pub route_type: u16,
    pub color: u32,
}

/// Unikátní sekvence zastávek linky.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    pub route: u32,
    pub stops: Vec<u32>,
    /// Bitové příznaky NO_PICKUP / NO_DROP_OFF pro každou zastávku.
    pub flags: Vec<u8>,
}

/// Pattern + časové offsety odjezdů od první zastávky (sekundy).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub pattern: u32,
    pub offsets: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trip {
    pub gtfs_id: String,
    pub profile: u32,
    pub service: u32,
    /// Odjezd z první zastávky v sekundách od „poledne − 12 h“ (může být ≥ 24 h).
    pub start: u32,
    pub headsign: u32,
}

/// Bitmapa dnů od `feed_start` (bit i = feed_start + i dní), kdy služba jede.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Service {
    pub days: Vec<u8>,
}

impl Timetable {
    pub fn string(&self, idx: u32) -> &str {
        &self.strings[idx as usize]
    }

    pub fn stop_name(&self, stop: u32) -> &str {
        self.string(self.stops[stop as usize].name)
    }

    pub fn group_name(&self, group: u32) -> &str {
        self.string(self.groups[group as usize].name)
    }

    pub fn service_runs(&self, service: u32, date: NaiveDate) -> bool {
        let day = (date - self.feed_start).num_days();
        if day < 0 {
            return false;
        }
        let day = day as usize;
        self.services[service as usize]
            .days
            .get(day / 8)
            .is_some_and(|b| b & (1 << (day % 8)) != 0)
    }
}
