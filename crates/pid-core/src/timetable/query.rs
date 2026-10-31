//! Dotazy do jízdního řádu: zastávky v okolí, odjezdy, zastávky po trase.

use std::cmp::Ordering;

use chrono::{DateTime, Duration, NaiveDate};
use chrono_tz::Tz;

use super::{NO_DROP_OFF, NO_PICKUP, Timetable};
use crate::geo::haversine_m;
use crate::realtime::Delays;
use crate::text::fold;
use crate::time::gtfs_instant;

pub const MIN_RADIUS_M: f64 = 100.0;
pub const MAX_RADIUS_M: f64 = 500.0;
/// Při horší přesnosti polohy (typicky pod zemí) jdou stanice metra v nabídce dopředu.
pub const METRO_FIRST_ACCURACY_M: f64 = 100.0;
const FALLBACK_COUNT: usize = 5;
const FALLBACK_RADIUS_M: f64 = 2000.0;
const EXPIRING_DAYS: i64 = 2;
/// Spoj se považuje za odjetý až tolik sekund po očekávaném odjezdu.
pub const DEPARTED_GRACE_S: i64 = 30;

#[derive(Clone, Debug, PartialEq)]
pub struct NearbyGroup {
    pub group: u32,
    pub name: String,
    pub distance_m: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Departure {
    /// Index do `Timetable::trips`.
    pub trip: u32,
    /// Stanoviště, ze kterého spoj odjíždí.
    pub stop: u32,
    /// GTFS stop_id stanoviště (pro párování s realtime daty).
    pub stop_gtfs_id: String,
    pub route_short_name: String,
    pub route_id: String,
    pub route_type: u16,
    pub color: u32,
    pub headsign: String,
    pub time: DateTime<Tz>,
    pub trip_gtfs_id: String,
    /// Dopravce spoje, prázdné = neznámý.
    pub operator: String,
    pub pattern: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StopRef {
    pub stop: u32,
    pub gtfs_id: String,
    pub name: String,
    pub distance_m: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataStatus {
    Valid,
    ExpiringSoon { days_left: i64 },
    Expired,
}

/// Zdroj odjezdů. Dnes statický GTFS, později třeba Golemio API.
pub trait DepartureSource {
    fn nearby_groups(&self, lat: f64, lon: f64, accuracy_m: f64) -> Vec<NearbyGroup>;
    fn search_groups(&self, query: &str, limit: usize) -> Vec<NearbyGroup> {
        self.search_groups_near(query, limit, None)
    }
    /// Hledání bez diakritiky; při stejné kvalitě shody blíž k `near` (lat, lon) dřív.
    /// Prázdný dotaz s polohou vrátí nejbližší zastávky.
    fn search_groups_near(&self, query: &str, limit: usize, near: Option<(f64, f64)>) -> Vec<NearbyGroup>;
    fn departures(&self, group: u32, at: DateTime<Tz>, before_min: i64, after_min: i64) -> Vec<Departure>;
    fn downstream_stops(&self, trip: u32, board_stop: u32, lat: Option<f64>, lon: Option<f64>) -> Vec<StopRef>;
    /// (route_id, short_name, route_type)
    fn search_routes(&self, query: &str, limit: usize) -> Vec<(String, String, u16)>;
    fn status(&self, today: NaiveDate) -> DataStatus;
    fn version(&self) -> &str;
}

pub struct TimetableIndex {
    pub tt: Timetable,
    /// stop → [(pattern, pozice v patternu)]
    stop_patterns: Vec<Vec<(u32, u32)>>,
    /// pattern → [profile]
    pattern_profiles: Vec<Vec<u32>>,
    /// profile → rozsah v `tt.trips` (trips jsou seřazené podle (profile, start))
    profile_trips: Vec<(usize, usize)>,
    group_names_folded: Vec<String>,
    group_has_metro: Vec<bool>,
}

impl TimetableIndex {
    pub fn new(mut tt: Timetable) -> Self {
        tt.trips.sort_by_key(|t| (t.profile, t.start));
        let mut stop_patterns = vec![Vec::new(); tt.stops.len()];
        for (p, pattern) in tt.patterns.iter().enumerate() {
            for (pos, &stop) in pattern.stops.iter().enumerate() {
                stop_patterns[stop as usize].push((p as u32, pos as u32));
            }
        }
        let mut pattern_profiles = vec![Vec::new(); tt.patterns.len()];
        for (i, profile) in tt.profiles.iter().enumerate() {
            pattern_profiles[profile.pattern as usize].push(i as u32);
        }
        let mut profile_trips = vec![(0, 0); tt.profiles.len()];
        let mut i = 0;
        while i < tt.trips.len() {
            let profile = tt.trips[i].profile;
            let start = i;
            while i < tt.trips.len() && tt.trips[i].profile == profile {
                i += 1;
            }
            profile_trips[profile as usize] = (start, i);
        }
        let group_names_folded = tt.groups.iter().map(|g| fold(tt.string(g.name))).collect();
        let mut group_has_metro = vec![false; tt.groups.len()];
        for pattern in tt.patterns.iter().filter(|p| tt.routes[p.route as usize].route_type == 1) {
            for &stop in &pattern.stops {
                group_has_metro[tt.stops[stop as usize].group as usize] = true;
            }
        }
        TimetableIndex { tt, stop_patterns, pattern_profiles, profile_trips, group_names_folded, group_has_metro }
    }

    fn stop_coords(&self, stop: u32) -> (f64, f64) {
        let s = &self.tt.stops[stop as usize];
        (s.lat_e6 as f64 / 1e6, s.lon_e6 as f64 / 1e6)
    }

    fn stop_distance(&self, stop: u32, lat: f64, lon: f64) -> f64 {
        let (slat, slon) = self.stop_coords(stop);
        haversine_m(lat, lon, slat, slon)
    }

    fn group_ref(&self, group: u32, distance_m: f64) -> NearbyGroup {
        NearbyGroup { group, name: self.tt.group_name(group).to_string(), distance_m }
    }

    /// Souřadnice prvního stanoviště skupiny (lat, lon).
    pub fn group_coords(&self, group: u32) -> (f64, f64) {
        self.stop_coords(self.group_first_stop(group))
    }

    /// Staví ve skupině metro?
    pub fn group_has_metro(&self, group: u32) -> bool {
        self.group_has_metro[group as usize]
    }

    /// Vzdálenost každé skupiny (nejbližší stanoviště) od bodu.
    fn group_distances(&self, lat: f64, lon: f64) -> Vec<f64> {
        let mut best = vec![f64::INFINITY; self.tt.groups.len()];
        for (i, stop) in self.tt.stops.iter().enumerate() {
            let d = self.stop_distance(i as u32, lat, lon);
            let slot = &mut best[stop.group as usize];
            if d < *slot {
                *slot = d;
            }
        }
        best
    }

    /// Skupina, do které patří stanoviště.
    pub fn group_of_stop(&self, stop: u32) -> u32 {
        self.tt.stops[stop as usize].group
    }

    /// První stanoviště skupiny (pro ručně zadanou linku bez konkrétního spoje).
    pub fn group_first_stop(&self, group: u32) -> u32 {
        self.tt.groups[group as usize].stops[0]
    }

    pub fn trip_by_gtfs_id(&self, id: &str) -> Option<u32> {
        self.tt.trips.iter().position(|t| t.gtfs_id == id).map(|i| i as u32)
    }

    pub fn stop_by_gtfs_id(&self, id: &str) -> Option<u32> {
        self.tt.stops.iter().position(|s| s.gtfs_id == id).map(|i| i as u32)
    }

    /// Označení stanoviště (např. „A“, „1“), pokud existuje.
    pub fn platform(&self, stop: u32) -> Option<&str> {
        self.tt.stops[stop as usize].platform.map(|p| self.tt.string(p))
    }
}

/// Z odjezdů seřazených podle času ponechá jen nejbližší pro každou dvojici linka + směr.
pub fn next_per_direction(departures: Vec<Departure>) -> Vec<Departure> {
    let mut seen = std::collections::HashSet::new();
    departures
        .into_iter()
        .filter(|d| seen.insert((d.route_id.clone(), d.headsign.clone())))
        .collect()
}

/// Odjezd na tabuli Nástupu: jízdní řád + zpoždění z realtime dat.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardRow {
    pub dep: Departure,
    /// Zpoždění v sekundách (záporné = náskok), `None` = neznámé.
    pub delay_s: Option<i32>,
    /// Očekávaný odjezd (jízdní řád + zpoždění).
    pub expected: DateTime<Tz>,
}

/// Rozdělí odjezdy na nadcházející (nejbližší neodjetý spoj pro každou linku a směr,
/// podle očekávaného času) a právě odjeté (posledních `recent_min` minut, nejnovější první).
pub fn board_rows(deps: Vec<Departure>, delays: &Delays, now: DateTime<Tz>, recent_min: i64) -> (Vec<BoardRow>, Vec<BoardRow>) {
    let mut rows: Vec<BoardRow> = deps
        .into_iter()
        .map(|dep| {
            let delay_s = delays.at(&dep.trip_gtfs_id, &dep.stop_gtfs_id);
            let expected = dep.time + Duration::seconds(delay_s.unwrap_or(0) as i64);
            BoardRow { dep, delay_s, expected }
        })
        .collect();
    rows.sort_by(|a, b| a.expected.cmp(&b.expected).then_with(|| a.dep.route_short_name.cmp(&b.dep.route_short_name)));
    let departed = |r: &BoardRow| r.expected + Duration::seconds(DEPARTED_GRACE_S) < now;
    let (gone, coming): (Vec<_>, Vec<_>) = rows.into_iter().partition(departed);
    let upcoming = next_rows_per_direction(coming);
    let since = now - Duration::minutes(recent_min);
    let recent = next_rows_per_direction(gone.into_iter().rev().filter(|r| r.expected >= since).collect());
    (upcoming, recent)
}

/// Vybraný řádek tabule: zvolený spoj (aktuální verze z tabule, jinak jeho uložená podoba,
/// i když už ze seznamu zmizel), bez volby nejbližší nadcházející.
pub fn pick_row(up: &[BoardRow], recent: &[BoardRow], chosen: Option<&BoardRow>) -> Option<BoardRow> {
    match chosen {
        Some(c) => Some(
            up.iter()
                .chain(recent)
                .find(|r| (r.dep.trip, r.dep.stop) == (c.dep.trip, c.dep.stop))
                .unwrap_or(c)
                .clone(),
        ),
        None => up.first().cloned(),
    }
}

/// „za 3 min“, „teď“, „před 3 min“ (celé minuty, okolí ±1 min je „teď“).
pub fn when_label(expected: DateTime<Tz>, now: DateTime<Tz>) -> String {
    let m = (expected - now).num_minutes();
    match m {
        1.. => format!("za {m} min"),
        0 => "teď".to_string(),
        _ => format!("před {} min", -m),
    }
}

fn next_rows_per_direction(rows: Vec<BoardRow>) -> Vec<BoardRow> {
    let mut seen = std::collections::HashSet::new();
    rows.into_iter().filter(|r| seen.insert((r.dep.route_id.clone(), r.dep.headsign.clone()))).collect()
}

fn by_distance(a: &NearbyGroup, b: &NearbyGroup) -> Ordering {
    a.distance_m.partial_cmp(&b.distance_m).unwrap_or(Ordering::Equal)
}

fn match_rank(haystack: &str, needle: &str) -> Option<u8> {
    if haystack.starts_with(needle) {
        Some(0)
    } else if haystack.split([' ', '-', ',', '/', '.']).any(|w| w.starts_with(needle)) {
        Some(1)
    } else if haystack.contains(needle) {
        Some(2)
    } else {
        None
    }
}

impl DepartureSource for TimetableIndex {
    fn nearby_groups(&self, lat: f64, lon: f64, accuracy_m: f64) -> Vec<NearbyGroup> {
        let radius = accuracy_m.clamp(MIN_RADIUS_M, MAX_RADIUS_M);
        let mut all: Vec<NearbyGroup> =
            self.group_distances(lat, lon).iter().enumerate().map(|(g, &d)| self.group_ref(g as u32, d)).collect();
        all.sort_by(by_distance);
        let within: Vec<_> = all.iter().filter(|g| g.distance_m <= radius).cloned().collect();
        let mut out = if within.is_empty() {
            all.into_iter().filter(|g| g.distance_m <= FALLBACK_RADIUS_M).take(FALLBACK_COUNT).collect()
        } else {
            within
        };
        if accuracy_m > METRO_FIRST_ACCURACY_M {
            // Stabilní řazení: metro napřed, jinak podle vzdálenosti.
            out.sort_by_key(|g| !self.group_has_metro(g.group));
        }
        out
    }

    fn search_groups_near(&self, query: &str, limit: usize, near: Option<(f64, f64)>) -> Vec<NearbyGroup> {
        let q = fold(query.trim());
        let dist = near.map(|(lat, lon)| self.group_distances(lat, lon));
        let d = |g: usize| dist.as_ref().map_or(0.0, |v| v[g]);
        if q.is_empty() {
            let Some(dist) = &dist else { return Vec::new() };
            let mut all: Vec<NearbyGroup> = dist.iter().enumerate().map(|(g, &d)| self.group_ref(g as u32, d)).collect();
            all.sort_by(by_distance);
            all.truncate(limit);
            return all;
        }
        let mut hits: Vec<(u8, f64, &str, u32)> = self
            .group_names_folded
            .iter()
            .enumerate()
            .filter_map(|(g, name)| match_rank(name, &q).map(|r| (r, d(g), name.as_str(), g as u32)))
            .collect();
        hits.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(b.2)));
        hits.into_iter().take(limit).map(|(_, dm, _, g)| self.group_ref(g, dm)).collect()
    }

    fn departures(&self, group: u32, at: DateTime<Tz>, before_min: i64, after_min: i64) -> Vec<Departure> {
        let from = at - Duration::minutes(before_min);
        let to = at + Duration::minutes(after_min);
        let today = at.date_naive();
        let mut out = Vec::new();
        for &stop in &self.tt.groups[group as usize].stops {
            for &(p, pos) in &self.stop_patterns[stop as usize] {
                let pattern = &self.tt.patterns[p as usize];
                let pos = pos as usize;
                if pattern.flags[pos] & NO_PICKUP != 0 || pos + 1 == pattern.stops.len() {
                    continue;
                }
                let route = &self.tt.routes[pattern.route as usize];
                for &profile_idx in &self.pattern_profiles[p as usize] {
                    let offset = self.tt.profiles[profile_idx as usize].offsets[pos] as i64;
                    let (lo, hi) = self.profile_trips[profile_idx as usize];
                    let trips = &self.tt.trips[lo..hi];
                    // Včerejší služba (časy ≥ 24 h), dnešní a zítřejší (okno přes půlnoc).
                    for service_date in [today - Duration::days(1), today, today + Duration::days(1)] {
                        let base = gtfs_instant(service_date, 0);
                        let start_min = (from - base).num_seconds() - offset;
                        let start_max = (to - base).num_seconds() - offset;
                        if start_max < 0 {
                            continue;
                        }
                        let first = trips.partition_point(|t| (t.start as i64) < start_min);
                        for (k, trip) in trips[first..].iter().enumerate() {
                            if trip.start as i64 > start_max {
                                break;
                            }
                            if !self.tt.service_runs(trip.service, service_date) {
                                continue;
                            }
                            out.push(Departure {
                                trip: (lo + first + k) as u32,
                                stop,
                                stop_gtfs_id: self.tt.stops[stop as usize].gtfs_id.clone(),
                                route_short_name: route.short_name.clone(),
                                route_id: route.gtfs_id.clone(),
                                route_type: route.route_type,
                                color: route.color,
                                headsign: self.tt.string(trip.headsign).to_string(),
                                time: gtfs_instant(service_date, trip.start + offset as u32),
                                trip_gtfs_id: trip.gtfs_id.clone(),
                                operator: trip.operator.map(|o| self.tt.string(o).to_string()).unwrap_or_default(),
                                pattern: p,
                            });
                        }
                    }
                }
            }
        }
        out.sort_by(|a, b| a.time.cmp(&b.time).then_with(|| a.route_short_name.cmp(&b.route_short_name)));
        out
    }

    fn downstream_stops(&self, trip: u32, board_stop: u32, lat: Option<f64>, lon: Option<f64>) -> Vec<StopRef> {
        let Some(trip) = self.tt.trips.get(trip as usize) else { return Vec::new() };
        let pattern = &self.tt.patterns[self.tt.profiles[trip.profile as usize].pattern as usize];
        let Some(pos) = pattern.stops.iter().position(|&s| s == board_stop) else { return Vec::new() };
        let mut out: Vec<StopRef> = pattern
            .stops
            .iter()
            .zip(&pattern.flags)
            .skip(pos + 1)
            .filter(|&(_, &flags)| flags & NO_DROP_OFF == 0)
            .map(|(&stop, _)| StopRef {
                stop,
                gtfs_id: self.tt.stops[stop as usize].gtfs_id.clone(),
                name: self.tt.stop_name(stop).to_string(),
                distance_m: lat.zip(lon).map(|(la, lo)| self.stop_distance(stop, la, lo)),
            })
            .collect();
        if lat.is_some() && lon.is_some() {
            out.sort_by(|a, b| a.distance_m.partial_cmp(&b.distance_m).unwrap_or(Ordering::Equal));
        }
        out
    }

    fn search_routes(&self, query: &str, limit: usize) -> Vec<(String, String, u16)> {
        let q = fold(query.trim());
        if q.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<(u8, usize, &str, usize)> = self
            .tt
            .routes
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                let name = fold(&r.short_name);
                let rank = if name == q { Some(0) } else { match_rank(&name, &q).map(|r| r + 1) };
                rank.map(|rank| (rank, r.short_name.len(), r.short_name.as_str(), i))
            })
            .collect();
        hits.sort();
        hits.into_iter()
            .take(limit)
            .map(|(_, _, _, i)| {
                let r = &self.tt.routes[i];
                (r.gtfs_id.clone(), r.short_name.clone(), r.route_type)
            })
            .collect()
    }

    fn status(&self, today: NaiveDate) -> DataStatus {
        let days_left = (self.tt.feed_end - today).num_days();
        if days_left < 0 {
            DataStatus::Expired
        } else if days_left <= EXPIRING_DAYS {
            DataStatus::ExpiringSoon { days_left }
        } else {
            DataStatus::Valid
        }
    }

    fn version(&self) -> &str {
        &self.tt.version
    }
}

#[cfg(test)]
mod when_tests {
    use super::*;
    use crate::time::TZ;
    use chrono::TimeZone;

    #[test]
    fn when_label_future_now_past() {
        let t = |h, m, s| TZ.with_ymd_and_hms(2026, 10, 31, h, m, s).unwrap();
        let now = t(21, 0, 0);
        assert_eq!(when_label(t(21, 3, 0), now), "za 3 min");
        assert_eq!(when_label(t(21, 0, 40), now), "teď");
        assert_eq!(when_label(t(20, 59, 30), now), "teď");
        assert_eq!(when_label(t(20, 57, 0), now), "před 3 min");
    }
}
