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

#[test]
fn next_per_direction_keeps_earliest() {
    use pid_core::timetable::query::next_per_direction;
    let ix = index();
    let d = ix.departures(group(&ix, "Náměstí Míru"), prague(2026, 10, 26, 7, 58), 5, 30);
    let n = next_per_direction(d);
    assert_eq!(n.len(), 1);
    assert_eq!(n[0].trip_gtfs_id, "T22b");
}

#[test]
fn next_day_service_before_midnight() {
    // Neděle 23:55: pondělní spoj kódovaný 00:05:00 (služba WD) musí být v okně +30 min.
    let ix = index();
    let d = ix.departures(group(&ix, "Náměstí Míru"), prague(2026, 10, 25, 23, 55), 5, 30);
    let ids: Vec<_> = d.iter().map(|x| x.trip_gtfs_id.as_str()).collect();
    assert!(ids.contains(&"T22m"), "{ids:?}");
    assert_eq!(d.iter().find(|x| x.trip_gtfs_id == "T22m").unwrap().time.format("%Y-%m-%d %H:%M").to_string(), "2026-10-26 00:05");
}

#[test]
fn departure_carries_operator() {
    let ix = index();
    let d = ix.departures(group(&ix, "Náměstí Míru"), prague(2026, 10, 26, 7, 58), 5, 30);
    assert_eq!(d[0].operator, "Dopravní podnik hl. m. Prahy, akciová společnost");
    assert_eq!(d[1].operator, "Testovací dopravce s.r.o.");
}

#[test]
fn board_rows_skip_departed_trip() {
    use pid_core::realtime::Delays;
    use pid_core::timetable::query::board_rows;
    let ix = index();
    let now = prague(2026, 10, 26, 8, 1);
    let d = ix.departures(group(&ix, "Náměstí Míru"), now, 15, 30);
    let (up, recent) = board_rows(d, &Delays::default(), now, 5);
    assert_eq!(up[0].dep.trip_gtfs_id, "T22c");
    assert_eq!(recent[0].dep.trip_gtfs_id, "T22b");
    assert_eq!(up[0].delay_s, None);
}

#[test]
fn board_rows_delayed_trip_is_upcoming() {
    use pid_core::realtime::Delays;
    use pid_core::timetable::query::board_rows;
    let ix = index();
    let now = prague(2026, 10, 26, 8, 1);
    let d = ix.departures(group(&ix, "Náměstí Míru"), now, 15, 30);
    let delays = Delays::from_pairs([("T22b", "U1Z2P", 120)]);
    let (up, recent) = board_rows(d, &delays, now, 5);
    assert_eq!(up[0].dep.trip_gtfs_id, "T22b");
    assert_eq!(up[0].delay_s, Some(120));
    assert_eq!(up[0].expected, prague(2026, 10, 26, 8, 2));
    assert!(recent.is_empty());
}

#[test]
fn departure_carries_stop_gtfs_id() {
    let ix = index();
    let d = ix.departures(group(&ix, "Náměstí Míru"), prague(2026, 10, 26, 7, 58), 5, 30);
    assert!(d.iter().any(|x| x.trip_gtfs_id == "T22b" && x.stop_gtfs_id == "U1Z2P"));
}

#[test]
fn metro_first_when_inaccurate() {
    let ix = index();
    // U Jana Masaryka (bez metra), Náměstí Míru se stanicí A je ~250 m.
    assert_eq!(ix.nearby_groups(50.0735, 14.4400, 400.0)[0].name, "Náměstí Míru");
    assert_eq!(ix.nearby_groups(50.0735, 14.4400, 20.0)[0].name, "Jana Masaryka");
}

#[test]
fn search_near_orders_equal_matches_by_distance() {
    let ix = index();
    let r = ix.search_groups_near("m", 5, Some((50.0755, 14.4375)));
    assert_eq!(r[0].name, "Muzeum");
    assert_eq!(r[1].name, "Náměstí Míru");
    assert!(r[1].distance_m < 50.0);
}

#[test]
fn search_near_empty_query_lists_nearest() {
    let r = index().search_groups_near("", 3, Some((50.0735, 14.4400)));
    assert_eq!(r[0].name, "Jana Masaryka");
    assert_eq!(r.len(), 3);
}

#[test]
fn search_diacritics_both_ways() {
    let ix = index();
    assert_eq!(ix.search_groups("RUS", 5)[0].name, "Ruská");
    assert_eq!(ix.search_groups("rusk", 5)[0].name, "Ruská");
    assert_eq!(ix.search_groups("Ruská", 5)[0].name, "Ruská");
    assert_eq!(ix.search_groups("náměstí", 5)[0].name, "Náměstí Míru");
}
