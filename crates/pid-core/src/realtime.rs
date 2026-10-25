//! Číslo vozu z realtime feedu PID (GTFS-Realtime, vehicle positions).
//!
//! Feed je veřejný (bez API klíče, s CORS). Dekódujeme jen pole, která potřebujeme;
//! ostatní pole protobuf při čtení přeskočí.

use prost::Message;

pub const VEHICLE_POSITIONS_URL: &str = "https://api.golemio.cz/v2/vehiclepositions/gtfsrt/vehicle_positions.pb";

#[derive(Clone, PartialEq, Message)]
pub struct FeedMessage {
    #[prost(message, repeated, tag = "2")]
    pub entity: Vec<FeedEntity>,
}

#[derive(Clone, PartialEq, Message)]
pub struct FeedEntity {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(message, optional, tag = "4")]
    pub vehicle: Option<VehiclePosition>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    fn entity(trip: &str, label: Option<&str>, vehicle_id: &str) -> FeedEntity {
        FeedEntity {
            id: format!("{trip}_x"),
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
