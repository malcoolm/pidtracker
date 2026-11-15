//! Výkon hledání spojení nad skutečnými daty. Spuštění:
//! `PIDT_FILE=dist/data/<soubor>.pidt cargo test -p gtfs-prep --release --test perf -- --ignored --nocapture`

use std::time::Instant;

use chrono::{Datelike, Duration, TimeZone, Weekday};
use pid_core::time::TZ;
use pid_core::timetable::codec::decode;
use pid_core::timetable::query::{DepartureSource, TimetableIndex};
use pid_core::timetable::router::{JourneyQuery, TimeMode};

const RUNS: usize = 20;

#[test]
#[ignore]
fn plan_perf_real_data() {
    let path = std::env::var("PIDT_FILE").expect("nastavte PIDT_FILE na soubor .pidt");
    let ix = TimetableIndex::new(decode(&std::fs::read(&path).unwrap()).unwrap());
    // První pracovní den po začátku platnosti dat (bez kontroly svátků).
    let mut day = ix.tt.feed_start + Duration::days(1);
    while matches!(day.weekday(), Weekday::Sat | Weekday::Sun) {
        day += Duration::days(1);
    }
    let time = TZ.from_local_datetime(&day.and_hms_opt(7, 40, 0).unwrap()).unwrap();
    // U Anděla → Letňany.
    let q = JourneyQuery {
        origins: ix.access_from(50.0710, 14.4036),
        target_group: ix.search_groups("Letňany", 1)[0].group,
        mode: TimeMode::DepartAt,
        time,
        excluded: Vec::new(),
        buffer_s: 120,
    };
    let found = measure("plan (odjezd 7:40)", &ix, &q);
    assert!(found.len() >= 3);
    let back = JourneyQuery { mode: TimeMode::ArriveBy, time: time + Duration::minutes(50), ..q };
    let found = measure("plan (příjezd do 8:30)", &ix, &back);
    assert!(found.len() >= 3);
}

/// `RUNS`× změří `plan` s `min_count` 3, vypíše spojení a medián.
fn measure(label: &str, ix: &TimetableIndex, q: &JourneyQuery) -> Vec<pid_core::timetable::router::Journey> {
    let mut times = Vec::with_capacity(RUNS);
    let mut found = Vec::new();
    for _ in 0..RUNS {
        let t0 = Instant::now();
        found = ix.plan(q, 3);
        times.push(t0.elapsed());
    }
    times.sort();
    println!("{label}: origins {}, journeys {}", q.origins.len(), found.len());
    for j in &found {
        println!("  {} → {} ({} přestupů)", j.leave.format("%H:%M"), j.arrive.format("%H:%M"), j.transfers);
    }
    println!("{label} median: {:.1} ms (min {:.1}, max {:.1})", ms(times[RUNS / 2]), ms(times[0]), ms(times[RUNS - 1]));
    found
}

fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}
