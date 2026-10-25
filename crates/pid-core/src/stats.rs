//! Statistiky jízd po osobách a obdobích.

use std::collections::{BTreeMap, HashMap};

use chrono::{Datelike, Duration, NaiveDate, Weekday};

use crate::ride::{Ride, RideStatus, RouteKind};
use crate::time::TZ;

/// Do celkového času se nepočítají jízdy delší než 6 h (zapomenutý Výstup).
pub const MAX_COUNTED_MINUTES: i64 = 360;
const TOP_ROUTES: usize = 5;
pub const TOP_VEHICLES: usize = 10;
const MONTHS_CS: [&str; 12] = [
    "leden", "únor", "březen", "duben", "květen", "červen", "červenec", "srpen", "září", "říjen", "listopad",
    "prosinec",
];
const WEEKDAYS_CS: [&str; 7] = ["Po", "Út", "St", "Čt", "Pá", "So", "Ne"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeriodKind {
    Week,
    Month,
    Year,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Period {
    Week { iso_year: i32, week: u32 },
    Month { year: i32, month: u32 },
    Year(i32),
    All,
}

fn week_start(iso_year: i32, week: u32) -> NaiveDate {
    NaiveDate::from_isoywd_opt(iso_year, week, Weekday::Mon).expect("platný ISO týden")
}

fn week_of(d: NaiveDate) -> Period {
    let w = d.iso_week();
    Period::Week { iso_year: w.year(), week: w.week() }
}

impl Period {
    pub fn containing(kind: PeriodKind, d: NaiveDate) -> Self {
        match kind {
            PeriodKind::Week => week_of(d),
            PeriodKind::Month => Period::Month { year: d.year(), month: d.month() },
            PeriodKind::Year => Period::Year(d.year()),
            PeriodKind::All => Period::All,
        }
    }

    pub fn kind(&self) -> PeriodKind {
        match self {
            Period::Week { .. } => PeriodKind::Week,
            Period::Month { .. } => PeriodKind::Month,
            Period::Year(_) => PeriodKind::Year,
            Period::All => PeriodKind::All,
        }
    }

    fn shift(&self, delta: i32) -> Self {
        match *self {
            Period::Week { iso_year, week } => week_of(week_start(iso_year, week) + Duration::weeks(delta as i64)),
            Period::Month { year, month } => {
                let idx = year * 12 + month as i32 - 1 + delta;
                Period::Month { year: idx.div_euclid(12), month: idx.rem_euclid(12) as u32 + 1 }
            }
            Period::Year(y) => Period::Year(y + delta),
            Period::All => Period::All,
        }
    }

    pub fn prev(&self) -> Self {
        self.shift(-1)
    }

    pub fn next(&self) -> Self {
        self.shift(1)
    }

    pub fn contains(&self, d: NaiveDate) -> bool {
        match *self {
            Period::All => true,
            p => Period::containing(p.kind(), d) == p,
        }
    }

    pub fn label_cs(&self) -> String {
        match *self {
            Period::Week { iso_year, week } => {
                let from = week_start(iso_year, week);
                let to = from + Duration::days(6);
                format!(
                    "týden {week}/{iso_year} ({}. {}. – {}. {}.)",
                    from.day(),
                    from.month(),
                    to.day(),
                    to.month()
                )
            }
            Period::Month { year, month } => format!("{} {year}", MONTHS_CS[month as usize - 1]),
            Period::Year(y) => y.to_string(),
            Period::All => "Celkem".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StatsQuery {
    pub period: Period,
    /// Prázdné = všichni, kdo v období jeli.
    pub persons: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PersonStats {
    pub person: String,
    pub rides: u32,
    pub minutes: i64,
    /// Sestupně podle počtu.
    pub by_kind: Vec<(RouteKind, u32)>,
    /// Max. 5, sestupně podle počtu, při shodě abecedně.
    pub top_routes: Vec<(String, u32)>,
    /// Počty jízd podle dopravce, sestupně (bez neznámého dopravce).
    pub by_operator: Vec<(String, u32)>,
    /// Nejčastější vozy (max. 10), sestupně (bez neznámého vozu).
    pub by_vehicle: Vec<(String, u32)>,
    /// Týden → dny Po..Ne, měsíc → dny 1..N, rok → měsíce 1..12, celkem → roky.
    pub series: Vec<(String, u32)>,
}

fn local_date(r: &Ride) -> NaiveDate {
    r.board_time.with_timezone(&TZ).date_naive()
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let next = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    (NaiveDate::from_ymd_opt(next.0, next.1, 1).unwrap() - Duration::days(1)).day()
}

fn series(period: Period, dates: &[NaiveDate]) -> Vec<(String, u32)> {
    let count = |f: &dyn Fn(&NaiveDate) -> bool| dates.iter().filter(|d| f(d)).count() as u32;
    match period {
        Period::Week { .. } => WEEKDAYS_CS
            .iter()
            .enumerate()
            .map(|(i, name)| (name.to_string(), count(&|d| d.weekday().num_days_from_monday() as usize == i)))
            .collect(),
        Period::Month { year, month } => (1..=days_in_month(year, month))
            .map(|day| (day.to_string(), count(&|d| d.day() == day)))
            .collect(),
        Period::Year(_) => (1..=12).map(|m| (m.to_string(), count(&|d| d.month() == m))).collect(),
        Period::All => {
            let (Some(min), Some(max)) = (dates.iter().map(|d| d.year()).min(), dates.iter().map(|d| d.year()).max())
            else {
                return Vec::new();
            };
            (min..=max).map(|y| (y.to_string(), count(&|d| d.year() == y))).collect()
        }
    }
}

fn sorted_counts<K: Ord + Clone>(map: HashMap<K, u32>) -> Vec<(K, u32)> {
    let mut v: Vec<_> = map.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

pub fn compute(rides: &[Ride], q: &StatsQuery) -> Vec<PersonStats> {
    let wanted: Vec<String> = q.persons.iter().map(|p| p.to_lowercase()).collect();
    let mut per_person: BTreeMap<String, Vec<&Ride>> = wanted.iter().map(|p| (p.clone(), Vec::new())).collect();
    for r in rides {
        if r.deleted || r.status != RideStatus::Done || !q.period.contains(local_date(r)) {
            continue;
        }
        let person = r.person.to_lowercase();
        if wanted.is_empty() || wanted.contains(&person) {
            per_person.entry(person).or_default().push(r);
        }
    }

    per_person
        .into_iter()
        .map(|(person, rides)| {
            let minutes = rides
                .iter()
                .filter_map(|r| r.duration_minutes())
                .filter(|&m| m > 0 && m <= MAX_COUNTED_MINUTES)
                .sum();
            let mut kinds = HashMap::new();
            let mut routes = HashMap::new();
            let mut operators = HashMap::new();
            let mut vehicles = HashMap::new();
            for r in &rides {
                *kinds.entry(r.kind()).or_insert(0) += 1;
                *routes.entry(r.route_short_name.clone()).or_insert(0) += 1;
                if !r.operator.trim().is_empty() {
                    *operators.entry(r.operator.trim().to_string()).or_insert(0) += 1;
                }
                if !r.vehicle.trim().is_empty() {
                    *vehicles.entry(r.vehicle.trim().to_string()).or_insert(0) += 1;
                }
            }
            let mut top_routes = sorted_counts(routes);
            top_routes.truncate(TOP_ROUTES);
            let mut by_vehicle = sorted_counts(vehicles);
            by_vehicle.truncate(TOP_VEHICLES);
            let dates: Vec<NaiveDate> = rides.iter().map(|r| local_date(r)).collect();
            PersonStats {
                person,
                rides: rides.len() as u32,
                minutes,
                by_kind: sorted_counts(kinds),
                by_operator: sorted_counts(operators),
                by_vehicle,
                top_routes,
                series: series(q.period, &dates),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ride::RideStatus;
    use crate::ride::tests::sample_ride;
    use chrono::{DateTime, Duration, NaiveDate};

    const TATA: &str = "tata@gmail.com";
    const SYN: &str = "syn@gmail.com";

    fn d(y: i32, m: u32, dd: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, dd).unwrap()
    }

    fn mk(id: &str, person: &str, start: &str, minutes: i64, route: &str, route_type: u16) -> Ride {
        let mut r = sample_ride();
        r.id = id.into();
        r.person = person.into();
        r.board_time = DateTime::parse_from_rfc3339(start).unwrap();
        r.alight_time = Some(r.board_time + Duration::minutes(minutes));
        r.route_short_name = route.into();
        r.route_type = Some(route_type);
        r
    }

    fn rides() -> Vec<Ride> {
        let mut deleted = mk("del", TATA, "2026-10-12T09:00:00+02:00", 10, "9", 0);
        deleted.deleted = true;
        let mut running = mk("run", TATA, "2026-10-13T09:00:00+02:00", 10, "9", 0);
        running.status = RideStatus::InProgress;
        running.alight_time = None;
        vec![
            mk("t1", TATA, "2026-10-05T08:00:00+02:00", 20, "22", 0),
            mk("t2", TATA, "2026-10-05T08:30:00+02:00", 10, "A", 1),
            mk("t3", TATA, "2026-10-20T07:00:00+02:00", 14 * 60, "135", 3),
            deleted,
            running,
            mk("s1", SYN, "2026-10-21T15:00:00+02:00", 15, "22", 0),
            mk("t4", TATA, "2026-11-02T08:00:00+01:00", 12, "22", 0),
        ]
    }

    fn month(m: u32) -> StatsQuery {
        StatsQuery { period: Period::Month { year: 2026, month: m }, persons: vec![] }
    }

    fn person<'a>(s: &'a [PersonStats], email: &str) -> &'a PersonStats {
        s.iter().find(|p| p.person == email).unwrap()
    }

    #[test]
    fn month_per_person() {
        let s = compute(&rides(), &month(10));
        assert_eq!(s.len(), 2);
        assert_eq!(person(&s, TATA).rides, 3);
        assert_eq!(person(&s, SYN).rides, 1);
        assert_eq!(s[0].person, SYN, "řazeno podle e-mailu");
    }

    #[test]
    fn forgotten_exit_excluded_from_time() {
        assert_eq!(person(&compute(&rides(), &month(10)), TATA).minutes, 30);
    }

    #[test]
    fn person_filter() {
        let q = StatsQuery { period: Period::Month { year: 2026, month: 10 }, persons: vec![SYN.into()] };
        let s = compute(&rides(), &q);
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].rides, s[0].minutes), (1, 15));
    }

    #[test]
    fn by_kind_and_top_routes() {
        let s = compute(&rides(), &month(10));
        let t = person(&s, TATA);
        let mut kinds = t.by_kind.clone();
        kinds.sort();
        assert_eq!(kinds, vec![(RouteKind::Tram, 1), (RouteKind::Metro, 1), (RouteKind::Bus, 1)]);
        assert_eq!(t.top_routes, vec![("135".into(), 1), ("22".into(), 1), ("A".into(), 1)]);
    }

    #[test]
    fn all_period_counts_everything() {
        let s = compute(&rides(), &StatsQuery { period: Period::All, persons: vec![] });
        assert_eq!(person(&s, TATA).rides, 4);
        assert_eq!(person(&s, TATA).series, vec![("2026".into(), 4)]);
    }

    #[test]
    fn series_month_has_all_days() {
        let s = compute(&rides(), &month(10));
        let t = person(&s, TATA);
        assert_eq!(t.series.len(), 31);
        assert_eq!(t.series.iter().map(|x| x.1).sum::<u32>(), 3);
        assert_eq!(t.series[4], ("5".into(), 2));
    }

    #[test]
    fn series_week_and_year() {
        let w = compute(&rides(), &StatsQuery { period: Period::Week { iso_year: 2026, week: 41 }, persons: vec![] });
        assert_eq!(person(&w, TATA).series.len(), 7);
        assert_eq!(person(&w, TATA).series[0], ("Po".into(), 2));
        let y = compute(&rides(), &StatsQuery { period: Period::Year(2026), persons: vec![] });
        assert_eq!(person(&y, TATA).series.len(), 12);
        assert_eq!(person(&y, TATA).series[10], ("11".into(), 1));
    }

    #[test]
    fn period_navigation() {
        assert_eq!(Period::Month { year: 2026, month: 1 }.prev(), Period::Month { year: 2025, month: 12 });
        assert_eq!(Period::Month { year: 2026, month: 12 }.next(), Period::Month { year: 2027, month: 1 });
        assert_eq!(Period::containing(PeriodKind::Week, d(2026, 10, 25)), Period::Week { iso_year: 2026, week: 43 });
        assert_eq!(Period::Week { iso_year: 2026, week: 1 }.prev(), Period::Week { iso_year: 2025, week: 52 });
        assert_eq!(Period::Year(2026).next(), Period::Year(2027));
    }

    #[test]
    fn period_labels() {
        assert_eq!(Period::Month { year: 2026, month: 10 }.label_cs(), "říjen 2026");
        assert_eq!(Period::Week { iso_year: 2026, week: 43 }.label_cs(), "týden 43/2026 (19. 10. – 25. 10.)");
        assert_eq!(Period::Year(2026).label_cs(), "2026");
        assert_eq!(Period::All.label_cs(), "Celkem");
    }
}

#[cfg(test)]
mod operator_vehicle_tests {
    use super::*;
    use crate::ride::tests::sample_ride;
    use chrono::{DateTime, Duration};

    fn mk(id: &str, operator: &str, vehicle: &str, start: &str) -> Ride {
        let mut r = sample_ride();
        r.id = id.into();
        r.operator = operator.into();
        r.vehicle = vehicle.into();
        r.board_time = DateTime::parse_from_rfc3339(start).unwrap();
        r.alight_time = Some(r.board_time + Duration::minutes(10));
        r
    }

    #[test]
    fn aggregates_by_operator_and_vehicle() {
        let rides = vec![
            mk("1", "DPP", "8758", "2026-10-05T08:00:00+02:00"),
            mk("2", "DPP", "8758", "2026-10-06T08:00:00+02:00"),
            mk("3", "DPP", "9355", "2026-10-07T08:00:00+02:00"),
            mk("4", "ARRIVA CITY a.s.", "", "2026-10-08T08:00:00+02:00"),
            mk("5", "", "1234", "2026-10-09T08:00:00+02:00"),
        ];
        let s = compute(&rides, &StatsQuery { period: Period::Month { year: 2026, month: 10 }, persons: vec![] });
        let p = &s[0];
        assert_eq!(p.by_operator, vec![("DPP".into(), 3), ("ARRIVA CITY a.s.".into(), 1)]);
        assert_eq!(p.by_vehicle, vec![("8758".into(), 2), ("1234".into(), 1), ("9355".into(), 1)]);
    }

    #[test]
    fn vehicle_list_is_capped() {
        let rides: Vec<Ride> = (0..15)
            .map(|i| mk(&i.to_string(), "DPP", &format!("{}", 1000 + i), "2026-10-05T08:00:00+02:00"))
            .collect();
        let s = compute(&rides, &StatsQuery { period: Period::All, persons: vec![] });
        assert_eq!(s[0].by_vehicle.len(), TOP_VEHICLES);
    }
}
