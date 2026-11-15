use std::path::PathBuf;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use gtfs_prep::{build_timetable, write_output};
use pid_core::timetable::codec::{decode, encode};
use pid_core::timetable::Timetable;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini")
}

fn gen_at() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap()
}

fn build() -> Timetable {
    build_timetable(&fixture(), gen_at()).unwrap()
}

fn service_on(t: &Timetable, trip_gtfs_id: &str, d: NaiveDate) -> bool {
    let trip = t.trips.iter().find(|x| x.gtfs_id == trip_gtfs_id).unwrap();
    t.service_runs(trip.service, d)
}

#[test]
fn builds_mini_feed() {
    let t = build();
    assert_eq!(t.stops.len(), 7);
    assert_eq!(t.groups.len(), 5);
    assert_eq!(t.trips.len(), 5);
    assert_eq!(t.patterns.len(), 2);
    assert_eq!(t.profiles.len(), 2);
    assert_eq!(t.feed_start, NaiveDate::from_ymd_opt(2026, 10, 25).unwrap());
    assert_eq!(t.feed_end, NaiveDate::from_ymd_opt(2026, 11, 7).unwrap());
    assert_eq!(t.version, "20261025-202610251200");
}

#[test]
fn stop_times_sorted_by_sequence() {
    let t = build();
    let trip = t.trips.iter().find(|x| x.gtfs_id == "T22b").unwrap();
    let pattern = &t.patterns[t.profiles[trip.profile as usize].pattern as usize];
    let names: Vec<_> = pattern.stops.iter().map(|&s| t.stop_name(s)).collect();
    assert_eq!(names, vec!["Náměstí Míru", "Jana Masaryka", "Ruská"]);
    assert_eq!(trip.start, 8 * 3600);
    assert_eq!(t.profiles[trip.profile as usize].offsets, vec![0, 120, 300]);
}

#[test]
fn pickup_dropoff_flags() {
    let t = build();
    let trip = t.trips.iter().find(|x| x.gtfs_id == "TA1").unwrap();
    let pattern = &t.patterns[t.profiles[trip.profile as usize].pattern as usize];
    assert_eq!(pattern.flags, vec![pid_core::timetable::NO_DROP_OFF, pid_core::timetable::NO_PICKUP]);
}

#[test]
fn holiday_flips_services() {
    let t = build();
    let d = |m, dd| NaiveDate::from_ymd_opt(2026, m, dd).unwrap();
    assert!(!service_on(&t, "T22b", d(10, 28)));
    assert!(service_on(&t, "T22n", d(10, 28)));
    assert!(service_on(&t, "T22b", d(10, 26)));
    assert!(!service_on(&t, "T22n", d(10, 26)));
    assert!(!service_on(&t, "T22b", d(11, 8)));
}

#[test]
fn codec_roundtrip() {
    let t = build();
    assert_eq!(decode(&encode(&t)).unwrap(), t);
}

#[test]
fn decode_rejects_garbage() {
    assert!(decode(b"nonsense").is_err());
}

#[test]
fn write_output_manifest() {
    let t = build();
    let dir = tempfile::tempdir().unwrap();
    let info = write_output(&t, dir.path()).unwrap();
    assert_eq!(info.file, "timetable-20261025-202610251200.pidt");
    assert!(dir.path().join(&info.file).exists());
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["file"], info.file);
    assert_eq!(manifest["version"], "20261025-202610251200");
    assert_eq!(manifest["feed_end"], "2026-11-07");
}

#[test]
fn trip_operator_from_sub_agency() {
    let t = build();
    let op = |id: &str| {
        let trip = t.trips.iter().find(|x| x.gtfs_id == id).unwrap();
        trip.operator.map(|o| t.string(o).to_string())
    };
    assert_eq!(op("T22b").as_deref(), Some("Dopravní podnik hl. m. Prahy, akciová společnost"));
    assert_eq!(op("T22c").as_deref(), Some("Testovací dopravce s.r.o."));
}

#[test]
fn footpaths_between_nearby_nodes() {
    let t = build();
    let idx = |id: &str| t.stops.iter().position(|s| s.gtfs_id == id).unwrap() as u32;
    let (mir_b, masaryka, ruska) = (idx("U1Z2P"), idx("U2Z1P"), idx("U3Z1P"));
    let fp = |from: u32, to: u32| t.footpaths[from as usize].iter().find(|f| f.to == to).copied();

    let there = fp(mir_b, masaryka).expect("Náměstí Míru B -> Jana Masaryka");
    assert!((340..=380).contains(&there.walk_s), "walk_s = {}", there.walk_s);
    let back = fp(masaryka, mir_b).expect("Jana Masaryka -> Náměstí Míru B");
    assert!((340..=380).contains(&back.walk_s), "walk_s = {}", back.walk_s);

    assert!(fp(mir_b, ruska).is_none());
    assert!(fp(ruska, mir_b).is_none());
    // uvnitř uzlu Náměstí Míru žádné footpaths
    let group = t.stops[mir_b as usize].group;
    for &s in &t.groups[group as usize].stops {
        for f in &t.footpaths[s as usize] {
            assert_ne!(t.stops[f.to as usize].group, group);
        }
    }
}

#[test]
fn decode_rejects_old_format() {
    let err = decode(b"PIDT2\x00\x00").unwrap_err();
    assert!(matches!(err, pid_core::timetable::codec::CodecError::BadMagic));
}
