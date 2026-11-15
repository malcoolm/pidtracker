//! Mimořádnosti z GTFS-RT feedu Alerts: parsování, fáze, řazení a výběr nových.

use std::collections::HashSet;

use chrono::{DateTime, FixedOffset};
use prost::Message;
use serde::{Deserialize, Serialize};

use crate::realtime::{FeedMessage, TranslatedString};
use crate::ride::{Ride, RideStatus};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceAlert {
    pub id: String,
    pub route_ids: Vec<String>,
    pub header: String,
    pub description: String,
    pub url: Option<String>,
    pub start: Option<i64>,
    pub end: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertPhase {
    Active,
    Planned,
    Over,
}

impl ServiceAlert {
    /// Fáze mimořádnosti vůči času `now_s` (unix sekundy).
    pub fn phase(&self, now_s: i64) -> AlertPhase {
        if self.end.is_some_and(|e| e < now_s) {
            AlertPhase::Over
        } else if self.start.is_some_and(|s| s > now_s) {
            AlertPhase::Planned
        } else {
            AlertPhase::Active
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListedAlert {
    pub alert: ServiceAlert,
    pub relevant: bool,
    pub phase: AlertPhase,
}

fn text(t: Option<TranslatedString>) -> String {
    let Some(t) = t else { return String::new() };
    let pick = t.translation.iter().position(|x| x.language.as_deref() == Some("cs")).unwrap_or(0);
    t.translation.into_iter().nth(pick).map(|x| x.text).unwrap_or_default()
}

fn is_relevant(a: &ServiceAlert, watched: &HashSet<String>) -> bool {
    a.route_ids.iter().any(|r| watched.contains(r))
}

/// Přečte feed mimořádností; chybný feed dá prázdný seznam.
pub fn parse_alerts(feed: &[u8]) -> Vec<ServiceAlert> {
    let Ok(msg) = FeedMessage::decode(feed) else { return Vec::new() };
    msg.entity
        .into_iter()
        .filter_map(|e| {
            let a = e.alert?;
            let mut route_ids: Vec<String> = Vec::new();
            for r in a.informed_entity.into_iter().filter_map(|s| s.route_id).filter(|r| !r.is_empty()) {
                if !route_ids.contains(&r) {
                    route_ids.push(r);
                }
            }
            let start = a.active_period.iter().filter_map(|p| p.start).min().map(|v| v as i64);
            // Chybějící konec v kterékoli periodě = bez konce.
            let end = if a.active_period.is_empty() || a.active_period.iter().any(|p| p.end.is_none()) {
                None
            } else {
                a.active_period.iter().filter_map(|p| p.end).max().map(|v| v as i64)
            };
            Some(ServiceAlert {
                id: e.id,
                route_ids,
                header: text(a.header_text),
                description: text(a.description_text),
                url: Some(text(a.url)).filter(|u| !u.is_empty()),
                start,
                end,
            })
        })
        .collect()
}

/// Linky, na kterých jsem jel v posledních `days` dnech (nebo právě jedu).
pub fn watched_routes(
    rides: &[Ride],
    now: DateTime<FixedOffset>,
    days: i64,
    resolve_short: impl Fn(&str) -> Option<String>,
) -> HashSet<String> {
    let from = now - chrono::Duration::days(days);
    rides
        .iter()
        .filter(|r| !r.deleted && (r.board_time >= from || r.status == RideStatus::InProgress))
        .filter_map(|r| {
            if r.route_id.is_empty() {
                resolve_short(&r.route_short_name)
            } else {
                Some(r.route_id.clone())
            }
        })
        .collect()
}

/// Seznam bez skončených; aktivní před plánovanými, uvnitř relevantní dřív.
pub fn list_alerts(alerts: &[ServiceAlert], watched: &HashSet<String>, now_s: i64) -> Vec<ListedAlert> {
    let mut out: Vec<ListedAlert> = alerts
        .iter()
        .map(|a| ListedAlert { alert: a.clone(), relevant: is_relevant(a, watched), phase: a.phase(now_s) })
        .filter(|l| l.phase != AlertPhase::Over)
        .collect();
    // Stabilní řazení: zachová pořadí feedu uvnitř skupin.
    out.sort_by_key(|l| (l.phase != AlertPhase::Active, !l.relevant));
    out
}

/// Nové (nevidět dosud), neskončené mimořádnosti na sledovaných linkách.
pub fn fresh_relevant<'a>(
    alerts: &'a [ServiceAlert],
    watched: &HashSet<String>,
    seen: &HashSet<String>,
    now_s: i64,
) -> Vec<&'a ServiceAlert> {
    alerts
        .iter()
        .filter(|a| !seen.contains(&a.id) && a.phase(now_s) != AlertPhase::Over && is_relevant(a, watched))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::realtime::{Alert, EntitySelector, FeedEntity, TimeRange, Translation};
    use crate::ride::tests::sample_ride;

    fn ts(pairs: &[(&str, &str)]) -> Option<TranslatedString> {
        Some(TranslatedString {
            translation: pairs
                .iter()
                .map(|(lang, text)| Translation { text: (*text).into(), language: Some((*lang).into()) })
                .collect(),
        })
    }

    fn feed(alerts: Vec<(&str, Alert)>) -> Vec<u8> {
        FeedMessage {
            entity: alerts
                .into_iter()
                .map(|(id, a)| FeedEntity { id: id.into(), trip_update: None, vehicle: None, alert: Some(a) })
                .collect(),
        }
        .encode_to_vec()
    }

    fn sa(id: &str, routes: &[&str], start: Option<i64>, end: Option<i64>) -> ServiceAlert {
        ServiceAlert {
            id: id.into(),
            route_ids: routes.iter().map(|s| s.to_string()).collect(),
            header: id.into(),
            description: String::new(),
            url: None,
            start,
            end,
        }
    }

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_czech_texts_and_routes() {
        let alert = Alert {
            active_period: vec![TimeRange { start: Some(200), end: None }, TimeRange { start: Some(100), end: Some(900) }],
            informed_entity: ["L22", "L22", "L9"]
                .iter()
                .map(|r| EntitySelector { route_id: Some((*r).into()) })
                .collect(),
            url: ts(&[("cs", "https://x.cz")]),
            header_text: ts(&[("en", "Detour"), ("cs", "Objížďka")]),
            description_text: ts(&[("en", "Long"), ("cs", "Dlouhý popis")]),
        };
        let out = parse_alerts(&feed(vec![("a1", alert)]));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "a1");
        assert_eq!(out[0].header, "Objížďka");
        assert_eq!(out[0].description, "Dlouhý popis");
        assert_eq!(out[0].route_ids, ["L22", "L9"]);
        assert_eq!(out[0].url.as_deref(), Some("https://x.cz"));
        assert_eq!(out[0].start, Some(100));
        assert_eq!(out[0].end, None);
    }

    #[test]
    fn garbage_feed_is_empty() {
        assert!(parse_alerts(b"xyz").is_empty());
    }

    #[test]
    fn phase_active_planned_over() {
        assert_eq!(sa("a", &[], Some(2000), None).phase(1000), AlertPhase::Planned);
        assert_eq!(sa("a", &[], Some(100), Some(500)).phase(1000), AlertPhase::Over);
        assert_eq!(sa("a", &[], None, None).phase(1000), AlertPhase::Active);
        assert_eq!(sa("a", &[], Some(100), Some(5000)).phase(1000), AlertPhase::Active);
    }

    #[test]
    fn alert_without_route_is_listed_not_relevant() {
        let alerts = vec![sa("a", &[], None, None)];
        let w = set(&["L22"]);
        let listed = list_alerts(&alerts, &w, 1000);
        assert_eq!(listed.len(), 1);
        assert!(!listed[0].relevant);
        assert!(fresh_relevant(&alerts, &w, &HashSet::new(), 1000).is_empty());
    }

    fn ride_at(route_id: &str, short: &str, board: &str, deleted: bool, status: RideStatus) -> Ride {
        let mut r = sample_ride();
        r.route_id = route_id.into();
        r.route_short_name = short.into();
        r.board_time = DateTime::parse_from_rfc3339(board).unwrap();
        r.deleted = deleted;
        r.status = status;
        r
    }

    #[test]
    fn watched_routes_last_30_days() {
        let now = DateTime::parse_from_rfc3339("2026-11-15T12:00:00+01:00").unwrap();
        let rides = vec![
            ride_at("L22", "22", "2026-11-05T08:00:00+01:00", false, RideStatus::Done),
            ride_at("L9", "9", "2026-10-06T08:00:00+01:00", false, RideStatus::Done),
            ride_at("L5", "5", "2026-11-14T08:00:00+01:00", true, RideStatus::Done),
            ride_at("", "136", "2026-11-10T08:00:00+01:00", false, RideStatus::Done),
        ];
        let got = watched_routes(&rides, now, 30, |s| (s == "136").then(|| "L136".to_string()));
        assert_eq!(got, set(&["L22", "L136"]));
    }

    #[test]
    fn watched_routes_includes_in_progress_old_ride() {
        let now = DateTime::parse_from_rfc3339("2026-11-15T12:00:00+01:00").unwrap();
        let rides = vec![ride_at("L9", "9", "2026-09-01T08:00:00+01:00", false, RideStatus::InProgress)];
        assert_eq!(watched_routes(&rides, now, 30, |_| None), set(&["L9"]));
    }

    #[test]
    fn fresh_relevant_skips_seen_and_over() {
        let alerts = vec![
            sa("new", &["L22"], None, None),
            sa("seen", &["L22"], None, None),
            sa("over", &["L22"], Some(1), Some(10)),
            sa("other", &["L9"], None, None),
        ];
        let got = fresh_relevant(&alerts, &set(&["L22"]), &set(&["seen"]), 1000);
        assert_eq!(got.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["new"]);
    }

    #[test]
    fn list_order_active_relevant_first() {
        let alerts = vec![
            sa("planned_rel", &["L22"], Some(5000), None),
            sa("active_other", &["L9"], None, None),
            sa("active_rel", &["L22"], None, None),
            sa("over", &["L22"], None, Some(10)),
            sa("active_other2", &[], None, None),
        ];
        let listed = list_alerts(&alerts, &set(&["L22"]), 1000);
        let ids: Vec<_> = listed.iter().map(|l| l.alert.id.as_str()).collect();
        assert_eq!(ids, ["active_rel", "active_other", "active_other2", "planned_rel"]);
        assert_eq!(listed[0].phase, AlertPhase::Active);
        assert!(listed[0].relevant);
    }
}
