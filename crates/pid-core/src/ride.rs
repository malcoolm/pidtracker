//! Model jízdy.

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RideStatus {
    InProgress,
    Done,
}

impl RideStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RideStatus::InProgress => "in_progress",
            RideStatus::Done => "done",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "in_progress" => Some(RideStatus::InProgress),
            "done" => Some(RideStatus::Done),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RouteKind {
    Tram,
    Metro,
    Train,
    Bus,
    Ferry,
    Funicular,
    Trolleybus,
    Other,
}

impl RouteKind {
    pub const ALL: [RouteKind; 8] = [
        RouteKind::Metro,
        RouteKind::Tram,
        RouteKind::Bus,
        RouteKind::Trolleybus,
        RouteKind::Train,
        RouteKind::Funicular,
        RouteKind::Ferry,
        RouteKind::Other,
    ];

    pub fn from_gtfs(t: u16) -> Self {
        match t {
            0 => RouteKind::Tram,
            1 => RouteKind::Metro,
            2 => RouteKind::Train,
            3 => RouteKind::Bus,
            4 => RouteKind::Ferry,
            7 => RouteKind::Funicular,
            11 => RouteKind::Trolleybus,
            _ => RouteKind::Other,
        }
    }

    pub fn to_gtfs(self) -> Option<u16> {
        match self {
            RouteKind::Tram => Some(0),
            RouteKind::Metro => Some(1),
            RouteKind::Train => Some(2),
            RouteKind::Bus => Some(3),
            RouteKind::Ferry => Some(4),
            RouteKind::Funicular => Some(7),
            RouteKind::Trolleybus => Some(11),
            RouteKind::Other => None,
        }
    }

    pub fn label_cs(&self) -> &'static str {
        match self {
            RouteKind::Tram => "Tramvaj",
            RouteKind::Metro => "Metro",
            RouteKind::Train => "Vlak",
            RouteKind::Bus => "Autobus",
            RouteKind::Ferry => "Přívoz",
            RouteKind::Funicular => "Lanovka",
            RouteKind::Trolleybus => "Trolejbus",
            RouteKind::Other => "Jiné",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ride {
    pub id: String,
    pub person: String,
    pub board_stop_id: String,
    pub board_stop_name: String,
    pub route_id: String,
    pub route_short_name: String,
    pub route_type: Option<u16>,
    pub headsign: String,
    pub trip_id: String,
    pub pattern_id: Option<u32>,
    pub board_time: DateTime<FixedOffset>,
    pub alight_stop_id: Option<String>,
    pub alight_stop_name: Option<String>,
    pub alight_time: Option<DateTime<FixedOffset>>,
    pub board_lat: Option<f64>,
    pub board_lon: Option<f64>,
    pub alight_lat: Option<f64>,
    pub alight_lon: Option<f64>,
    pub status: RideStatus,
    pub deleted: bool,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub app_version: String,
    pub data_version: String,
    /// Evidenční číslo vozu (z realtime feedu nebo ručně); prázdné = neznámé.
    #[serde(default)]
    pub vehicle: String,
    /// Dopravce spoje (z jízdního řádu); prázdné = neznámý.
    #[serde(default)]
    pub operator: String,
}

impl Ride {
    pub fn can_edit(&self, email: &str) -> bool {
        self.person.eq_ignore_ascii_case(email.trim())
    }

    pub fn duration_minutes(&self) -> Option<i64> {
        self.alight_time.map(|a| (a - self.board_time).num_minutes())
    }

    pub fn kind(&self) -> RouteKind {
        self.route_type.map(RouteKind::from_gtfs).unwrap_or(RouteKind::Other)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use chrono::DateTime;

    pub(crate) fn sample_ride() -> Ride {
        let t = |s: &str| DateTime::parse_from_rfc3339(s).unwrap();
        Ride {
            id: "0b6f3c1e-4d0a-4c39-9a51-6a1f1c0e2b11".into(),
            person: "tata@gmail.com".into(),
            board_stop_id: "U1Z2P".into(),
            board_stop_name: "Náměstí Míru".into(),
            route_id: "L22".into(),
            route_short_name: "22".into(),
            route_type: Some(0),
            headsign: "Nádraží Hostivař".into(),
            trip_id: "22_1_261026".into(),
            pattern_id: Some(3),
            board_time: t("2026-10-26T08:00:00+01:00"),
            alight_stop_id: Some("U3Z1P".into()),
            alight_stop_name: Some("Ruská".into()),
            alight_time: Some(t("2026-10-26T08:05:00+01:00")),
            board_lat: Some(50.0756),
            board_lon: Some(14.4379),
            alight_lat: Some(50.07),
            alight_lon: Some(14.45),
            status: RideStatus::Done,
            deleted: false,
            created_at: t("2026-10-26T08:00:00+01:00"),
            updated_at: t("2026-10-26T08:05:00+01:00"),
            app_version: "0.1.0+a1b2c3d.202610251200".into(),
            data_version: "20261025-202610251200".into(),
            vehicle: "8758".into(),
            operator: "Dopravní podnik hl. m. Prahy, akciová společnost".into(),
        }
    }

    #[test]
    fn can_edit_only_own() {
        let r = sample_ride();
        assert!(r.can_edit("tata@gmail.com"));
        assert!(r.can_edit("Tata@Gmail.com"));
        assert!(!r.can_edit("syn@gmail.com"));
    }

    #[test]
    fn duration_minutes() {
        let mut r = sample_ride();
        assert_eq!(r.duration_minutes(), Some(5));
        r.alight_time = None;
        assert_eq!(r.duration_minutes(), None);
    }

    #[test]
    fn route_kind_mapping() {
        assert_eq!(RouteKind::from_gtfs(0), RouteKind::Tram);
        assert_eq!(RouteKind::from_gtfs(11), RouteKind::Trolleybus);
        assert_eq!(RouteKind::from_gtfs(99), RouteKind::Other);
        assert_eq!(RouteKind::Train.label_cs(), "Vlak");
    }
}
