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
    #[prost(message, optional, tag = "8")]
    pub vehicle: Option<VehicleDescriptor>,
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
        let v = vp.vehicle?;
        v.label
            .filter(|l| !l.trim().is_empty())
            // Jen `service-<x>-<číslo vozu>`; `metro-…` (pořadí soupravy) a `train-…` (číslo vlaku) nejsou čísla vozů.
            .or_else(|| {
                v.id.filter(|id| id.starts_with("service-"))
                    .and_then(|id| id.rsplit('-').next().map(str::to_string))
            })
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
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
