//! Číslo vozu z realtime feedu PID (GTFS-Realtime, vehicle positions).
//!
//! Feed je veřejný (bez API klíče, s CORS). Dekódujeme jen pole, která potřebujeme;
//! ostatní pole protobuf při čtení přeskočí.

use std::collections::HashMap;

use prost::Message;

pub const VEHICLE_POSITIONS_URL: &str = "https://api.golemio.cz/v2/vehiclepositions/gtfsrt/vehicle_positions.pb";
/// Zpoždění jedoucích spojů (~1,3 MB, bez komprese) – stahovat střídmě.
pub const TRIP_UPDATES_URL: &str = "https://api.golemio.cz/v2/vehiclepositions/gtfsrt/trip_updates.pb";

#[derive(Clone, PartialEq, Message)]
pub struct FeedMessage {
    #[prost(message, repeated, tag = "2")]
    pub entity: Vec<FeedEntity>,
}

#[derive(Clone, PartialEq, Message)]
pub struct FeedEntity {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(message, optional, tag = "3")]
    pub trip_update: Option<TripUpdate>,
    #[prost(message, optional, tag = "4")]
    pub vehicle: Option<VehiclePosition>,
}

#[derive(Clone, PartialEq, Message)]
pub struct TripUpdate {
    #[prost(message, optional, tag = "1")]
    pub trip: Option<TripDescriptor>,
    #[prost(message, repeated, tag = "2")]
    pub stop_time_update: Vec<StopTimeUpdate>,
    #[prost(int32, optional, tag = "5")]
    pub delay: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
pub struct StopTimeUpdate {
    #[prost(uint32, optional, tag = "1")]
    pub stop_sequence: Option<u32>,
    #[prost(message, optional, tag = "2")]
    pub arrival: Option<StopTimeEvent>,
    #[prost(message, optional, tag = "3")]
    pub departure: Option<StopTimeEvent>,
    #[prost(string, optional, tag = "4")]
    pub stop_id: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct StopTimeEvent {
    #[prost(int32, optional, tag = "1")]
    pub delay: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
pub struct VehiclePosition {
    #[prost(message, optional, tag = "1")]
    pub trip: Option<TripDescriptor>,
    #[prost(message, optional, tag = "2")]
    pub position: Option<Position>,
    #[prost(uint32, optional, tag = "3")]
    pub current_stop_sequence: Option<u32>,
    #[prost(uint64, optional, tag = "5")]
    pub timestamp: Option<u64>,
    #[prost(message, optional, tag = "8")]
    pub vehicle: Option<VehicleDescriptor>,
}

#[derive(Clone, PartialEq, Message)]
pub struct Position {
    #[prost(float, tag = "1")]
    pub latitude: f32,
    #[prost(float, tag = "2")]
    pub longitude: f32,
}

#[derive(Clone, PartialEq, Message)]
pub struct TripDescriptor {
    #[prost(string, optional, tag = "1")]
    pub trip_id: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct VehicleDescriptor {
    #[prost(string, optional, tag = "1")]
    pub id: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub label: Option<String>,
}

/// Evidenční číslo vozu z popisu vozidla: `label`, jinak přípona `service-<x>-<číslo>`.
fn vehicle_label(v: VehicleDescriptor) -> Option<String> {
    v.label
        .filter(|l| !l.trim().is_empty())
        // Jen `service-<x>-<číslo vozu>`; `metro-…` (pořadí soupravy) a `train-…` (číslo vlaku) nejsou čísla vozů.
        .or_else(|| {
            v.id.filter(|id| id.starts_with("service-"))
                .and_then(|id| id.rsplit('-').next().map(str::to_string))
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Evidenční číslo vozu, který právě jede spojem `trip_id` (GTFS trip_id ze statických dat).
pub fn vehicle_for_trip(feed: &[u8], trip_id: &str) -> Option<String> {
    if trip_id.is_empty() {
        return None;
    }
    let msg = FeedMessage::decode(feed).ok()?;
    msg.entity.into_iter().find_map(|e| {
        let vp = e.vehicle?;
        if vp.trip?.trip_id.as_deref() != Some(trip_id) {
            return None;
        }
        vehicle_label(vp.vehicle?)
    })
}

/// Poloha vozu spoje z feedu `vehicle_positions.pb`.
#[derive(Clone, Debug, PartialEq)]
pub struct VehicleFix {
    pub label: Option<String>,
    pub lat: f64,
    pub lon: f64,
    /// Pořadí zastávky, ke které vůz míří (GTFS `stop_sequence`, u PID 1..n).
    pub stop_sequence: Option<u32>,
    pub timestamp: Option<i64>,
}

/// Poloha vozu jedoucího spojem `trip_id`; `None` = spoj ve feedu není nebo nemá polohu.
pub fn vehicle_fix(feed: &[u8], trip_id: &str) -> Option<VehicleFix> {
    if trip_id.is_empty() {
        return None;
    }
    let msg = FeedMessage::decode(feed).ok()?;
    msg.entity.into_iter().find_map(|e| {
        let vp = e.vehicle?;
        if vp.trip?.trip_id.as_deref() != Some(trip_id) {
            return None;
        }
        let pos = vp.position?;
        Some(VehicleFix {
            label: vp.vehicle.and_then(vehicle_label),
            lat: f64::from(pos.latitude),
            lon: f64::from(pos.longitude),
            stop_sequence: vp.current_stop_sequence,
            timestamp: vp.timestamp.and_then(|t| i64::try_from(t).ok()),
        })
    })
}

/// Kde na trase vůz je.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VehiclePlace {
    AtStop(usize),
    /// Mezi zastávkami `i` a `i+1`.
    Between(usize),
}

/// Do této vzdálenosti od zastávky je vůz „na zastávce“.
const AT_STOP_M: f64 = 50.0;
/// Dál než tohle od trasy vůz na trase neumisťujeme.
const MAX_OFF_ROUTE_M: f64 = 300.0;

/// Umístí vůz na trasu zadanou souřadnicemi zastávek (lat, lon) v pořadí jízdy.
pub fn place_vehicle(stops: &[(f64, f64)], fix: &VehicleFix) -> Option<VehiclePlace> {
    let dist = |i: usize| crate::geo::haversine_m(fix.lat, fix.lon, stops[i].0, stops[i].1);
    if let Some(s) = fix.stop_sequence.map(|s| s as usize).filter(|s| (1..=stops.len()).contains(s)) {
        let next = s - 1;
        return Some(if next == 0 || dist(next) <= AT_STOP_M {
            VehiclePlace::AtStop(next)
        } else {
            VehiclePlace::Between(next - 1)
        });
    }
    // Nejbližší úsek lomené čáry; ekvirektangulární projekce kolem polohy vozu (metry).
    let k = 111_320.0;
    let cos = fix.lat.to_radians().cos();
    let xy = |(lat, lon): (f64, f64)| ((lon - fix.lon) * k * cos, (lat - fix.lat) * k);
    let mut best: Option<(usize, f64)> = None;
    for i in 0..stops.len().saturating_sub(1) {
        let (a, b) = (xy(stops[i]), xy(stops[i + 1]));
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len2 = dx * dx + dy * dy;
        let t = if len2 > 0.0 { (-(a.0 * dx + a.1 * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
        let d = (a.0 + t * dx).hypot(a.1 + t * dy);
        if best.is_none_or(|(_, bd)| d < bd) {
            best = Some((i, d));
        }
    }
    let (i, d) = best?;
    if d > MAX_OFF_ROUTE_M {
        return None;
    }
    // U zastávky (konec i začátek úseku; vůz může být kousek za ní) = na zastávce.
    Some(if dist(i + 1) <= AT_STOP_M {
        VehiclePlace::AtStop(i + 1)
    } else if dist(i) <= AT_STOP_M {
        VehiclePlace::AtStop(i)
    } else {
        VehiclePlace::Between(i)
    })
}

#[derive(Clone, Debug, Default, PartialEq)]
struct TripDelay {
    /// stop_id → zpoždění v sekundách (odjezd, jinak příjezd)
    by_stop: HashMap<String, i32>,
    trip: Option<i32>,
}

/// Zpoždění jedoucích spojů podle GTFS `trip_id`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Delays(HashMap<String, TripDelay>);

impl Delays {
    /// Zpoždění spoje na stanovišti v sekundách (záporné = náskok), `None` = neznámé.
    pub fn at(&self, trip_id: &str, stop_id: &str) -> Option<i32> {
        let t = self.0.get(trip_id)?;
        t.by_stop.get(stop_id).copied().or(t.trip)
    }

    /// Ruční sestavení (testy): (trip_id, stop_id, zpoždění s).
    pub fn from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str, i32)>) -> Self {
        let mut out = Delays::default();
        for (trip, stop, delay) in pairs {
            out.0.entry(trip.to_string()).or_default().by_stop.insert(stop.to_string(), delay);
        }
        out
    }
}

/// Dekóduje feed `trip_updates.pb`. Neplatná data → prázdné zpoždění.
pub fn parse_trip_delays(feed: &[u8]) -> Delays {
    let Ok(msg) = FeedMessage::decode(feed) else { return Delays::default() };
    let mut out = Delays::default();
    for tu in msg.entity.into_iter().filter_map(|e| e.trip_update) {
        let Some(trip_id) = tu.trip.and_then(|t| t.trip_id) else { continue };
        let mut td = TripDelay { trip: tu.delay, ..Default::default() };
        for u in tu.stop_time_update {
            let delay = u.departure.and_then(|e| e.delay).or(u.arrival.and_then(|e| e.delay));
            if let (Some(stop), Some(delay)) = (u.stop_id, delay) {
                td.by_stop.insert(stop, delay);
            }
        }
        out.0.insert(trip_id, td);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    fn entity(trip: &str, label: Option<&str>, vehicle_id: &str) -> FeedEntity {
        FeedEntity {
            id: format!("{trip}_x"),
            trip_update: None,
            vehicle: Some(VehiclePosition {
                trip: Some(TripDescriptor { trip_id: Some(trip.into()) }),
                vehicle: Some(VehicleDescriptor { id: Some(vehicle_id.into()), label: label.map(Into::into) }),
                position: None,
                current_stop_sequence: None,
                timestamp: None,
            }),
        }
    }

    fn feed(entities: Vec<FeedEntity>) -> Vec<u8> {
        FeedMessage { entity: entities }.encode_to_vec()
    }

    #[test]
    fn finds_vehicle_label_for_trip() {
        let bytes = feed(vec![
            entity("244_3216_261024", Some("4189"), "service-3-4189"),
            entity("22_41333_261017", Some("8758"), "service-0-8758"),
        ]);
        assert_eq!(vehicle_for_trip(&bytes, "22_41333_261017").as_deref(), Some("8758"));
    }

    #[test]
    fn falls_back_to_vehicle_id_suffix() {
        let bytes = feed(vec![entity("12_1_261025", None, "service-0-9355")]);
        assert_eq!(vehicle_for_trip(&bytes, "12_1_261025").as_deref(), Some("9355"));
    }

    #[test]
    fn unknown_trip_or_garbage_is_none() {
        let bytes = feed(vec![entity("22_1_1", Some("8758"), "x")]);
        assert_eq!(vehicle_for_trip(&bytes, "9_9_9"), None);
        assert_eq!(vehicle_for_trip(b"\xff\xff garbage", "9_9_9"), None);
        assert_eq!(vehicle_for_trip(&bytes, ""), None);
    }
}

#[cfg(test)]
mod mode_tests {
    use super::*;
    use prost::Message;

    fn one(trip: &str, vehicle_id: &str) -> Vec<u8> {
        FeedMessage {
            entity: vec![FeedEntity {
                id: "x".into(),
                trip_update: None,
                vehicle: Some(VehiclePosition {
                    trip: Some(TripDescriptor { trip_id: Some(trip.into()) }),
                    vehicle: Some(VehicleDescriptor { id: Some(vehicle_id.into()), label: None }),
                    position: None,
                    current_stop_sequence: None,
                    timestamp: None,
                }),
            }],
        }
        .encode_to_vec()
    }

    #[test]
    fn metro_and_train_ids_are_not_vehicle_numbers() {
        // Metro: pořadí soupravy, vlak: číslo vlaku – ani jedno není evidenční číslo vozu.
        assert_eq!(vehicle_for_trip(&one("991_1_1", "metro-A-9-9"), "991_1_1"), None);
        assert_eq!(vehicle_for_trip(&one("1240_9713_1", "train-9713"), "1240_9713_1"), None);
    }
}

#[cfg(test)]
mod delay_tests {
    use super::*;
    use prost::Message;

    fn stu(stop: &str, arrival: Option<i32>, departure: Option<i32>) -> StopTimeUpdate {
        StopTimeUpdate {
            stop_sequence: None,
            arrival: arrival.map(|d| StopTimeEvent { delay: Some(d) }),
            departure: departure.map(|d| StopTimeEvent { delay: Some(d) }),
            stop_id: Some(stop.into()),
        }
    }

    fn feed(trip: &str, updates: Vec<StopTimeUpdate>, trip_delay: Option<i32>) -> Vec<u8> {
        FeedMessage {
            entity: vec![FeedEntity {
                id: "x".into(),
                vehicle: None,
                trip_update: Some(TripUpdate {
                    trip: Some(TripDescriptor { trip_id: Some(trip.into()) }),
                    stop_time_update: updates,
                    delay: trip_delay,
                }),
            }],
        }
        .encode_to_vec()
    }

    #[test]
    fn delay_from_matching_stop() {
        let d = parse_trip_delays(&feed("T1", vec![stu("U1Z1P", Some(10), Some(20)), stu("U1Z2P", Some(80), Some(90))], None));
        assert_eq!(d.at("T1", "U1Z2P"), Some(90));
    }

    #[test]
    fn arrival_delay_when_no_departure() {
        let d = parse_trip_delays(&feed("T1", vec![stu("U1Z2P", Some(30), None)], None));
        assert_eq!(d.at("T1", "U1Z2P"), Some(30));
    }

    #[test]
    fn trip_level_delay_fallback() {
        let d = parse_trip_delays(&feed("T1", vec![stu("U1Z2P", Some(30), None)], Some(60)));
        assert_eq!(d.at("T1", "XYZ"), Some(60));
    }

    #[test]
    fn unknown_trip_or_garbage() {
        let d = parse_trip_delays(&feed("T1", vec![stu("U1Z2P", Some(30), None)], None));
        assert_eq!(d.at("nope", "U1Z2P"), None);
        assert_eq!(d.at("T1", "XYZ"), None);
        assert_eq!(parse_trip_delays(b"\xff\xff"), Delays::default());
    }
}

#[cfg(test)]
mod place_tests {
    use super::*;
    use prost::Message;

    // Tři zastávky na přímce (poledník), po ~500 m.
    const STOPS: [(f64, f64); 3] = [(50.0, 14.4), (50.0045, 14.4), (50.0090, 14.4)];

    fn fix(lat: f64, lon: f64, seq: Option<u32>) -> VehicleFix {
        VehicleFix { label: None, lat, lon, stop_sequence: seq, timestamp: None }
    }

    #[test]
    fn place_by_stop_sequence() {
        assert_eq!(place_vehicle(&STOPS, &fix(50.00675, 14.4, Some(3))), Some(VehiclePlace::Between(1)));
        // Míří na první zastávku, nebo je u cílové zastávky do 50 m.
        assert_eq!(place_vehicle(&STOPS, &fix(50.0, 14.4, Some(1))), Some(VehiclePlace::AtStop(0)));
        assert_eq!(place_vehicle(&STOPS, &fix(50.0090, 14.4, Some(3))), Some(VehiclePlace::AtStop(2)));
    }

    #[test]
    fn place_by_gps_projection() {
        assert_eq!(place_vehicle(&STOPS, &fix(50.00225, 14.4003, None)), Some(VehiclePlace::Between(0)));
        // ~20 m od 2. zastávky.
        assert_eq!(place_vehicle(&STOPS, &fix(50.00468, 14.4, None)), Some(VehiclePlace::AtStop(1)));
    }

    #[test]
    fn place_vehicle_far_from_route_is_none() {
        assert_eq!(place_vehicle(&STOPS, &fix(50.00225, 14.4142, None)), None);
        assert_eq!(place_vehicle(&[], &fix(50.0, 14.4, None)), None);
    }

    #[test]
    fn vehicle_fix_reads_position() {
        let mk = |trip: &str| FeedEntity {
            id: "x".into(),
            trip_update: None,
            vehicle: Some(VehiclePosition {
                trip: Some(TripDescriptor { trip_id: Some(trip.into()) }),
                vehicle: Some(VehicleDescriptor { id: Some("service-0-8758".into()), label: Some("8758".into()) }),
                position: Some(Position { latitude: 50.5, longitude: 14.25 }),
                current_stop_sequence: Some(5),
                timestamp: Some(1_700_000_000),
            }),
        };
        let bytes = FeedMessage { entity: vec![mk("A"), mk("B")] }.encode_to_vec();
        assert_eq!(
            vehicle_fix(&bytes, "B"),
            Some(VehicleFix {
                label: Some("8758".into()),
                lat: 50.5,
                lon: 14.25,
                stop_sequence: Some(5),
                timestamp: Some(1_700_000_000)
            })
        );
        assert_eq!(vehicle_fix(&bytes, "nope"), None);
    }
}
