use std::path::PathBuf;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use gtfs_prep::build_timetable;
use pid_core::time::TZ;
use pid_core::timetable::query::{DataStatus, DepartureSource, TimetableIndex};

fn index() -> TimetableIndex {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini");
    TimetableIndex::new(build_timetable(&dir, Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap()).unwrap())
}

fn prague(y: i32, m: u32, d: u32, h: u32, mi: u32) -> DateTime<Tz> {
    TZ.with_ymd_and_hms(y, m, d, h, mi, 0).unwrap()
}

fn group(ix: &TimetableIndex, name: &str) -> u32 {
    ix.search_groups(name, 1)[0].group
}

#[test]
fn nearby_finds_namesti_miru_not_ruska() {
    let ix = index();
    let g = ix.nearby_groups(50.07555, 14.43760, 20.0);
    assert_eq!(g[0].name, "Náměstí Míru");
    assert!(g.iter().all(|x| x.name != "Ruská"));
}

#[test]
fn nearby_falls_back_to_nearest_when_empty() {
    let g = index().nearby_groups(50.0790, 14.4200, 10.0);
    assert!(!g.is_empty() && g.len() <= 5);
    assert_eq!(g[0].name, "Muzeum");
}

#[test]
fn weekday_departures_in_window() {
    let ix = index();
    let g = group(&ix, "Náměstí Míru");
    let d = ix.departures(g, prague(2026, 10, 26, 7, 58), 5, 30);
    let lines: Vec<_> =
        d.iter().map(|x| (x.route_short_name.as_str(), x.time.format("%H:%M").to_string())).collect();
    assert_eq!(lines, vec![("22", "08:00".to_string()), ("22", "08:10".to_string())]);
    assert_eq!(d[0].headsign, "Nádraží Hostivař");
    assert_eq!(d[0].trip_gtfs_id, "T22b");
    assert_eq!(d[0].route_type, 0);
}

#[test]
fn holiday_uses_weekend_service() {
    let ix = index();
    assert!(ix.departures(group(&ix, "Náměstí Míru"), prague(2026, 10, 28, 7, 58), 5, 30).is_empty());
}

#[test]
fn night_trip_after_midnight() {
    let ix = index();
    let d = ix.departures(group(&ix, "Náměstí Míru"), prague(2026, 11, 1, 0, 5), 5, 30);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].time.format("%Y-%m-%d %H:%M").to_string(), "2026-11-01 00:10");
}

#[test]
fn downstream_after_board() {
    let ix = index();
    let dep = &ix.departures(group(&ix, "Náměstí Míru"), prague(2026, 10, 26, 7, 58), 5, 30)[0];
    let s: Vec<_> = ix.downstream_stops(dep.trip, dep.stop, None, None).into_iter().map(|x| x.name).collect();
    assert_eq!(s, vec!["Jana Masaryka", "Ruská"]);
}

#[test]
fn downstream_sorted_by_distance_when_located() {
    let ix = index();
    let dep = &ix.departures(group(&ix, "Náměstí Míru"), prague(2026, 10, 26, 7, 58), 5, 30)[0];
    let s = ix.downstream_stops(dep.trip, dep.stop, Some(50.0701), Some(14.4499));
    assert_eq!(s[0].name, "Ruská");
    assert!(s[0].distance_m.unwrap() < 30.0);
}

#[test]
fn search_ignores_diacritics() {
    assert_eq!(index().search_groups("namesti mi", 5)[0].name, "Náměstí Míru");
    assert_eq!(index().search_groups("RUSK", 5)[0].name, "Ruská");
}

#[test]
fn search_routes_by_short_name() {
    let r = index().search_routes("22", 5);
    assert_eq!(r[0].1, "22");
    assert_eq!(r[0].2, 0);
}

#[test]
fn timetable_status_expiry() {
    let ix = index();
    let d = |m, dd| NaiveDate::from_ymd_opt(2026, m, dd).unwrap();
    assert_eq!(ix.status(d(10, 26)), DataStatus::Valid);
    assert_eq!(ix.status(d(11, 6)), DataStatus::ExpiringSoon { days_left: 1 });
    assert_eq!(ix.status(d(11, 8)), DataStatus::Expired);
}

#[test]
fn lookup_by_gtfs_ids() {
    let ix = index();
    let trip = ix.trip_by_gtfs_id("T22c").unwrap();
    assert_eq!(ix.tt.trips[trip as usize].gtfs_id, "T22c");
    let stop = ix.stop_by_gtfs_id("U1Z2P").unwrap();
    assert_eq!(ix.tt.stops[stop as usize].gtfs_id, "U1Z2P");
    assert_eq!(ix.tt.group_name(ix.group_of_stop(stop)), "Náměstí Míru");
    assert!(ix.trip_by_gtfs_id("nope").is_none());
    assert_eq!(ix.tt.stops[ix.group_first_stop(ix.group_of_stop(stop)) as usize].gtfs_id, "U1Z1P");
}
