//! Hledání spojení (RAPTOR) nad jízdním řádem: odjezd v čase i příjezd do času.

use chrono::{DateTime, Duration, NaiveDate};
use chrono_tz::Tz;

use super::query::TimetableIndex;
use super::walk::{MAX_TRANSFERS, METRO_ACCESS_EXTRA_S, transfer_seconds, walk_seconds};
use super::{NO_DROP_OFF, NO_PICKUP};
use crate::geo::haversine_m;
use crate::ride::RouteKind;
use crate::time::gtfs_instant;

/// Výchozí stanoviště hledání s dobou chůze k němu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Access {
    pub stop: u32,
    /// 0 při startu přímo ze zastávky.
    pub walk_s: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeMode {
    DepartAt,
    ArriveBy,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JourneyQuery {
    pub origins: Vec<Access>,
    pub target_group: u32,
    pub mode: TimeMode,
    pub time: DateTime<Tz>,
    pub excluded: Vec<RouteKind>,
    pub buffer_s: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leg {
    /// from = None → z polohy uživatele
    Walk { from: Option<u32>, to: u32, seconds: u32 },
    Ride {
        trip: u32,
        board_pos: u16,
        alight_pos: u16,
        board_stop: u32,
        alight_stop: u32,
        dep: DateTime<Tz>,
        arr: DateTime<Tz>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Journey {
    pub legs: Vec<Leg>,
    pub leave: DateTime<Tz>,
    pub arrive: DateTime<Tz>,
    pub transfers: u8,
    pub walk_s: u32,
}

impl TimetableIndex {
    /// Jedno hledání: Pareto množina spojení (nejlepší čas pro každý počet přestupů
    /// 0..=`MAX_TRANSFERS`, jen zlepšující se), seřazená podle počtu přestupů.
    pub fn plan_once(&self, q: &JourneyQuery) -> Vec<Journey> {
        let origins = self.origin_ends(q);
        if origins.is_empty() {
            return Vec::new();
        }
        let targets = self.target_ends(q.target_group);
        let fwd = q.mode == TimeMode::DepartAt;
        let (sources, sinks) = if fwd { (origins, targets) } else { (targets, origins) };
        let mut r = Raptor::new(self, q, fwd, sources, sinks);
        let start = q.time.timestamp();
        r.run(if fwd { start } else { -start });
        (1..=ROUNDS)
            .filter_map(|k| r.found[k].map(|sink| r.journey(q, &r.steps(k, sink))))
            .collect()
    }

    /// Výchozí stanoviště (mimo cílový uzel); cena navíc = chůze + rezerva (+ metro).
    fn origin_ends(&self, q: &JourneyQuery) -> Vec<Endpoint> {
        q.origins
            .iter()
            .filter(|o| self.group_of_stop(o.stop) != q.target_group)
            .map(|o| {
                let walk = if o.walk_s > 0 { EndWalk::FromUser(o.walk_s) } else { EndWalk::None };
                Endpoint { stop: o.stop, extra: self.access_extra(o.stop, o.walk_s, q.buffer_s) as i64, walk }
            })
            .collect()
    }

    /// Čas od opuštění polohy (nebo příchodu na zastávku) do nejdřívějšího nástupu.
    fn access_extra(&self, stop: u32, walk_s: u32, buffer_s: u32) -> u32 {
        let metro = if walk_s > 0 && self.is_metro_stop(stop) { METRO_ACCESS_EXTRA_S } else { 0 };
        walk_s + buffer_s + metro
    }

    /// Cílová stanoviště: všechna stanoviště uzlu a stanoviště jiných uzlů, ze kterých
    /// do něj vede footpath (cena navíc = čistá chůze). Footpaths jsou symetrické
    /// (gtfs-prep je počítá ze vzdálenosti oběma směry), stačí projít ty z cíle.
    fn target_ends(&self, group: u32) -> Vec<Endpoint> {
        let stops = &self.tt.groups[group as usize].stops;
        let mut out: Vec<Endpoint> =
            stops.iter().map(|&stop| Endpoint { stop, extra: 0, walk: EndWalk::None }).collect();
        for &to in stops {
            for f in &self.tt.footpaths[to as usize] {
                if self.group_of_stop(f.to) == group {
                    continue;
                }
                let walk_s = u32::from(f.walk_s);
                out.push(Endpoint { stop: f.to, extra: walk_s as i64, walk: EndWalk::ToTarget { to, walk_s } });
            }
        }
        out
    }
}

/// Počet kol RAPTORu (jízd) = přestupy + 1.
const ROUNDS: usize = MAX_TRANSFERS as usize + 1;
const INF: i64 = i64::MAX;

/// Začátek nebo konec běhu na stanovišti: cena navíc a chůze, která k němu patří.
#[derive(Clone, Copy, Debug)]
struct Endpoint {
    stop: u32,
    extra: i64,
    walk: EndWalk,
}

/// Chůze patřící ke konci běhu (ve skutečném směru cesty).
#[derive(Clone, Copy, Debug)]
enum EndWalk {
    None,
    /// Z polohy uživatele na `Endpoint::stop`.
    FromUser(u32),
    /// Z `Endpoint::stop` pěšky na stanoviště cílového uzlu.
    ToTarget { to: u32, walk_s: u32 },
}

/// Zpětný ukazatel jízdy: spoj, servisní den a pozice ve směru běhu
/// (`enter` = kde běh do spoje vstoupil, `exit` = kde vystoupil).
#[derive(Clone, Copy, Debug)]
struct RideBack {
    trip: u32,
    day: u8,
    enter_pos: u16,
    exit_pos: u16,
    enter_stop: u32,
}

/// Odkud se vzal štítek „připraven“ na stanovišti.
#[derive(Clone, Copy, Debug)]
enum ReadyBack {
    Source(usize),
    /// Přestup ze štítku jízdy na `from` (stejné stanoviště = bez chůze).
    Transfer { from: u32, seconds: u32 },
}

/// Úsek rekonstruovaného spojení ve skutečném pořadí, ještě se směrem běhu.
enum Step {
    End(Endpoint),
    Ride { back: RideBack, exit_stop: u32 },
    /// Přestup ve směru běhu (`from` má štítek jízdy, `to` štítek „připraven“).
    Transfer { from: u32, to: u32, seconds: u32 },
}

/// Jeden běh RAPTORu. Časy jsou „ceny“ v epoch sekundách, které se minimalizují:
/// dopředu (odjezd v čase) cena = čas, zpětně (příjezd do času) cena = −čas.
/// Zpětný běh tak prochází trasy proti směru jízdy a hledá nejpozdější spoje;
/// zdroje jsou cílová stanoviště a cíle běhu výchozí stanoviště.
struct Raptor<'a> {
    ix: &'a TimetableIndex,
    fwd: bool,
    excluded: &'a [RouteKind],
    /// Servisní dny D−1, D, D+1 a epoch sekundy jejich GTFS půlnoci.
    days: [NaiveDate; 3],
    bases: [i64; 3],
    sources: Vec<Endpoint>,
    sinks: Vec<Endpoint>,
    /// `arr[k][stop]`: cena po výstupu z k-té jízdy (index 0 se nepoužívá).
    arr: Vec<Vec<i64>>,
    /// `ready[k][stop]`: nejlepší cena, kdy lze na stanovišti nastoupit do (k+1)-té jízdy.
    ready: Vec<Vec<i64>>,
    best_arr: Vec<i64>,
    best_ready: Vec<i64>,
    ride_back: Vec<Vec<Option<RideBack>>>,
    ready_back: Vec<Vec<Option<ReadyBack>>>,
    best_sink: i64,
    /// `found[k]`: cíl běhu (index do `sinks`) s nejlepší cenou po k jízdách,
    /// jen pokud zlepšuje výsledek s méně jízdami.
    found: [Option<usize>; ROUNDS + 1],
}

impl<'a> Raptor<'a> {
    fn new(ix: &'a TimetableIndex, q: &'a JourneyQuery, fwd: bool, sources: Vec<Endpoint>, sinks: Vec<Endpoint>) -> Self {
        let n = ix.tt.stops.len();
        let d = q.time.date_naive();
        let days = [d - Duration::days(1), d, d + Duration::days(1)];
        let bases = days.map(|day| gtfs_instant(day, 0).timestamp());
        Raptor {
            ix,
            fwd,
            excluded: &q.excluded,
            days,
            bases,
            sources,
            sinks,
            arr: vec![vec![INF; n]; ROUNDS + 1],
            ready: vec![vec![INF; n]; ROUNDS + 1],
            best_arr: vec![INF; n],
            best_ready: vec![INF; n],
            ride_back: vec![vec![None; n]; ROUNDS + 1],
            ready_back: vec![vec![None; n]; ROUNDS + 1],
            best_sink: INF,
            found: [None; ROUNDS + 1],
        }
    }

    fn run(&mut self, start: i64) {
        let mut marked = Vec::new();
        for i in 0..self.sources.len() {
            let src = self.sources[i];
            let cost = start + src.extra;
            if self.improve_ready(0, src.stop, cost, ReadyBack::Source(i)) {
                marked.push(src.stop);
            }
        }
        marked.sort_unstable();
        marked.dedup();
        for k in 1..=ROUNDS {
            if marked.is_empty() {
                break;
            }
            let improved = self.scan_routes(k, &marked);
            self.check_sinks(k);
            marked = self.relax_transfers(k, &improved);
        }
    }

    fn cost(&self, time: i64) -> i64 {
        if self.fwd { time } else { -time }
    }

    /// Cena spoje na pozici `pos` jeho profilu.
    fn trip_cost(&self, trip: u32, day: u8, pos: usize) -> i64 {
        let tt = &self.ix.tt;
        let t = &tt.trips[trip as usize];
        let offset = tt.profiles[t.profile as usize].offsets[pos];
        self.cost(self.bases[day as usize] + i64::from(t.start) + i64::from(offset))
    }

    fn excluded_pattern(&self, pattern: u32) -> bool {
        let tt = &self.ix.tt;
        let route = &tt.routes[tt.patterns[pattern as usize].route as usize];
        self.excluded.contains(&RouteKind::from_gtfs(route.route_type))
    }

    /// Projde profily přes označená stanoviště; vrátí stanoviště se zlepšeným `arr[k]`.
    fn scan_routes(&mut self, k: usize, marked: &[u32]) -> Vec<u32> {
        let ix = self.ix;
        // (profil, první pozice ve směru běhu)
        let mut queue: Vec<(u32, usize)> = Vec::new();
        for &stop in marked {
            for &(p, pos) in &ix.stop_patterns[stop as usize] {
                if self.excluded_pattern(p) {
                    continue;
                }
                queue.extend(ix.pattern_profiles[p as usize].iter().map(|&prof| (prof, pos as usize)));
            }
        }
        let fwd = self.fwd;
        queue.sort_unstable_by_key(|&(prof, pos)| (prof, if fwd { pos as i64 } else { -(pos as i64) }));
        queue.dedup_by_key(|e| e.0);
        let mut improved = Vec::new();
        for (prof, pos) in queue {
            self.scan_profile(k, prof, pos, &mut improved);
        }
        improved.sort_unstable();
        improved.dedup();
        improved
    }

    /// Jízda po profilu od pozice `start` ve směru běhu: výstupy aktuálním spojem
    /// a přesedání na lepší spoj tam, kde je stanoviště připravené z kola k−1.
    fn scan_profile(&mut self, k: usize, prof: u32, start: usize, improved: &mut Vec<u32>) {
        let tt = &self.ix.tt;
        let pattern = &tt.patterns[tt.profiles[prof as usize].pattern as usize];
        let count = if self.fwd { pattern.stops.len() - start } else { start + 1 };
        let (exit_flag, enter_flag) = if self.fwd { (NO_DROP_OFF, NO_PICKUP) } else { (NO_PICKUP, NO_DROP_OFF) };
        let mut cur: Option<RideBack> = None;
        for i in 0..count {
            let pos = if self.fwd { start + i } else { start - i };
            let stop = pattern.stops[pos];
            let s = stop as usize;
            let flags = pattern.flags[pos];
            let cur_cost = cur.map_or(INF, |c| self.trip_cost(c.trip, c.day, pos));
            if let Some(c) = cur
                && flags & exit_flag == 0
                && cur_cost < self.arr[k][s].min(self.best_arr[s]).min(self.best_sink)
            {
                self.arr[k][s] = cur_cost;
                self.best_arr[s] = cur_cost;
                self.ride_back[k][s] = Some(RideBack { exit_pos: pos as u16, ..c });
                improved.push(stop);
            }
            let ready = self.ready[k - 1][s];
            let last = i + 1 == count;
            if last || flags & enter_flag != 0 || ready >= cur_cost {
                continue;
            }
            if let Some((trip, day, cost)) = self.catch_trip(prof, pos, ready)
                && cost < cur_cost
            {
                cur = Some(RideBack { trip, day, enter_pos: pos as u16, exit_pos: pos as u16, enter_stop: stop });
            }
        }
    }

    /// Nejlepší spoj profilu, který na pozici `pos` stihneme s cenou ≥ `ready`
    /// (dopředu nejdřívější odjezd, zpětně nejpozdější příjezd) přes dny D−1, D, D+1.
    fn catch_trip(&self, prof: u32, pos: usize, ready: i64) -> Option<(u32, u8, i64)> {
        let tt = &self.ix.tt;
        let (lo, hi) = self.ix.profile_trips[prof as usize];
        let trips = &tt.trips[lo..hi];
        let offset = i64::from(tt.profiles[prof as usize].offsets[pos]);
        let mut best: Option<(u32, u8, i64)> = None;
        for (d, (&date, &base)) in self.days.iter().zip(&self.bases).enumerate() {
            let runs = |t: &super::Trip| tt.service_runs(t.service, date);
            let idx = if self.fwd {
                let min_start = ready - base - offset;
                let first = trips.partition_point(|t| i64::from(t.start) < min_start);
                trips[first..].iter().position(runs).map(|i| first + i)
            } else {
                let max_start = -ready - base - offset;
                let end = trips.partition_point(|t| i64::from(t.start) <= max_start);
                trips[..end].iter().rposition(runs)
            };
            let Some(i) = idx else { continue };
            let trip = (lo + i) as u32;
            let cost = self.trip_cost(trip, d as u8, pos);
            if best.is_none_or(|b| cost < b.2) {
                best = Some((trip, d as u8, cost));
            }
        }
        best
    }

    fn check_sinks(&mut self, k: usize) {
        for (i, sink) in self.sinks.iter().enumerate() {
            let arr = self.arr[k][sink.stop as usize];
            if arr == INF {
                continue;
            }
            let cost = arr + sink.extra;
            if cost < self.best_sink {
                self.best_sink = cost;
                self.found[k] = Some(i);
            }
        }
    }

    /// Přestupy po kole k: stejné stanoviště, ostatní stanoviště uzlu a footpaths.
    /// Vrátí stanoviště se zlepšeným `ready[k]` (označená pro další kolo).
    fn relax_transfers(&mut self, k: usize, improved: &[u32]) -> Vec<u32> {
        let ix = self.ix;
        let mut marked = Vec::new();
        for &from in improved {
            let arr = self.arr[k][from as usize];
            let m = ix.is_metro_stop(from);
            let group = &ix.tt.groups[ix.group_of_stop(from) as usize].stops;
            let in_group = group.iter().map(|&to| {
                let walk = if to == from { 0 } else { walk_seconds(stop_distance_m(ix, from, to)) };
                (to, walk)
            });
            // Footpaths jsou symetrické, zpětný běh je tedy může použít obráceně.
            let paths = ix.tt.footpaths[from as usize].iter().map(|f| (f.to, u32::from(f.walk_s)));
            for (to, walk) in in_group.chain(paths) {
                let seconds = transfer_seconds(walk, m, ix.is_metro_stop(to));
                let cost = arr + i64::from(seconds);
                if cost < self.best_sink && self.improve_ready(k, to, cost, ReadyBack::Transfer { from, seconds }) {
                    marked.push(to);
                }
            }
        }
        marked.sort_unstable();
        marked.dedup();
        marked
    }

    fn improve_ready(&mut self, k: usize, stop: u32, cost: i64, back: ReadyBack) -> bool {
        let s = stop as usize;
        if cost >= self.ready[k][s].min(self.best_ready[s]) {
            return false;
        }
        self.ready[k][s] = cost;
        self.best_ready[s] = cost;
        self.ready_back[k][s] = Some(back);
        true
    }

    /// Rekonstrukce spojení ze zpětných ukazatelů, ve skutečném pořadí úseků.
    fn steps(&self, k: usize, sink: usize) -> Vec<Step> {
        let end = self.sinks[sink];
        let mut out = vec![Step::End(end)];
        let (mut k, mut stop) = (k, end.stop);
        loop {
            let back = self.ride_back[k][stop as usize].expect("štítek jízdy má zpětný ukazatel");
            out.push(Step::Ride { back, exit_stop: stop });
            let enter = back.enter_stop;
            match self.ready_back[k - 1][enter as usize].expect("štítek nástupu má zpětný ukazatel") {
                ReadyBack::Source(i) => {
                    out.push(Step::End(self.sources[i]));
                    break;
                }
                ReadyBack::Transfer { from, seconds } => {
                    if from != enter {
                        out.push(Step::Transfer { from, to: enter, seconds });
                    }
                    stop = from;
                    k -= 1;
                }
            }
        }
        // Dopředu se skládalo od cíle; zpětně od výchozího stanoviště, tj. už popořadě.
        if self.fwd {
            out.reverse();
        }
        out
    }

    fn leg(&self, step: &Step) -> Option<Leg> {
        let tt = &self.ix.tt;
        match *step {
            Step::End(Endpoint { walk: EndWalk::None, .. }) => None,
            Step::End(Endpoint { stop, walk: EndWalk::FromUser(seconds), .. }) => {
                Some(Leg::Walk { from: None, to: stop, seconds })
            }
            Step::End(Endpoint { stop, walk: EndWalk::ToTarget { to, walk_s }, .. }) => {
                Some(Leg::Walk { from: Some(stop), to, seconds: walk_s })
            }
            Step::Transfer { from, to, seconds } => {
                let (from, to) = if self.fwd { (from, to) } else { (to, from) };
                Some(Leg::Walk { from: Some(from), to, seconds })
            }
            Step::Ride { back, exit_stop } => {
                let ((board_pos, board_stop), (alight_pos, alight_stop)) = if self.fwd {
                    ((back.enter_pos, back.enter_stop), (back.exit_pos, exit_stop))
                } else {
                    ((back.exit_pos, exit_stop), (back.enter_pos, back.enter_stop))
                };
                let trip = &tt.trips[back.trip as usize];
                let offsets = &tt.profiles[trip.profile as usize].offsets;
                let at = |pos: u16| gtfs_instant(self.days[back.day as usize], trip.start + offsets[pos as usize]);
                Some(Leg::Ride {
                    trip: back.trip,
                    board_pos,
                    alight_pos,
                    board_stop,
                    alight_stop,
                    dep: at(board_pos),
                    arr: at(alight_pos),
                })
            }
        }
    }

    fn journey(&self, q: &JourneyQuery, steps: &[Step]) -> Journey {
        let legs: Vec<Leg> = steps.iter().filter_map(|s| self.leg(s)).collect();
        let rides: Vec<(u32, DateTime<Tz>, DateTime<Tz>)> = legs
            .iter()
            .filter_map(|l| match *l {
                Leg::Ride { board_stop, dep, arr, .. } => Some((board_stop, dep, arr)),
                Leg::Walk { .. } => None,
            })
            .collect();
        let walk_of = |l: Option<&Leg>, user: bool| match l {
            Some(&Leg::Walk { from, seconds, .. }) if from.is_none() == user => seconds,
            _ => 0,
        };
        let (first_stop, first_dep, _) = rides[0];
        let access = walk_of(legs.first(), true);
        let leave = first_dep - Duration::seconds(i64::from(self.ix.access_extra(first_stop, access, q.buffer_s)));
        let arrive = rides[rides.len() - 1].2 + Duration::seconds(i64::from(walk_of(legs.last(), false)));
        let walk_s = legs
            .iter()
            .map(|l| match *l {
                Leg::Walk { seconds, .. } => seconds,
                Leg::Ride { .. } => 0,
            })
            .sum();
        Journey { leave, arrive, transfers: (rides.len() - 1) as u8, walk_s, legs }
    }
}

fn stop_distance_m(ix: &TimetableIndex, a: u32, b: u32) -> f64 {
    let (sa, sb) = (&ix.tt.stops[a as usize], &ix.tt.stops[b as usize]);
    let deg = |e6: i32| f64::from(e6) / 1e6;
    haversine_m(deg(sa.lat_e6), deg(sa.lon_e6), deg(sb.lat_e6), deg(sb.lon_e6))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::TZ;
    use crate::timetable::{
        Footpath, NO_DROP_OFF, NO_PICKUP, Pattern, Profile, Route, Service, Stop, StopGroup, Timetable, Trip,
    };
    use chrono::{NaiveDate, TimeZone};

    // Stanoviště testovací sítě.
    const S1: u32 = 0;
    const H1: u32 = 1;
    const H2: u32 = 2;
    const T1: u32 = 3;
    const T2: u32 = 4;
    const J1: u32 = 5;
    // Uzly.
    const START: u32 = 0;
    const CIL: u32 = 2;
    const JINDE: u32 = 3;

    const TRAM_PATTERN: usize = 0;

    fn feed_start() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()
    }

    fn feed_end() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 30).unwrap()
    }

    fn hm(h: u32, m: u32) -> u32 {
        h * 3600 + m * 60
    }

    fn stop(name: u32, lat: f64, lon: f64, group: u32) -> Stop {
        Stop {
            gtfs_id: format!("U{name}-{group}-{lat}"),
            name,
            platform: None,
            lat_e6: (lat * 1e6).round() as i32,
            lon_e6: (lon * 1e6).round() as i32,
            group,
        }
    }

    fn trip(profile: u32, start: u32) -> Trip {
        Trip { gtfs_id: format!("T{profile}-{start}"), profile, service: 0, start, headsign: 0, operator: None }
    }

    /// Služba, která jede každý den platnosti feedu (bity přesně do `feed_end`).
    fn every_day() -> Service {
        let n = (feed_end() - feed_start()).num_days() as usize + 1;
        let mut days = vec![0u8; n.div_ceil(8)];
        for i in 0..n {
            days[i / 8] |= 1 << (i % 8);
        }
        Service { days }
    }

    /// Start (S1) — Uzel (H1 tram, H2 metro, 50 m) — Cíl (T1 tram, T2 metro); Jinde (J1) u T1.
    fn timetable() -> Timetable {
        Timetable {
            version: "test".into(),
            feed_start: feed_start(),
            feed_end: feed_end(),
            strings: ["Start", "Uzel", "Cíl", "Jinde"].map(String::from).to_vec(),
            stops: vec![
                stop(0, 50.10, 14.40, 0),
                stop(1, 50.08, 14.42, 1),
                stop(1, 50.08045, 14.42, 1),
                stop(2, 50.06, 14.44, 2),
                stop(2, 50.0605, 14.44, 2),
                stop(3, 50.058, 14.44, 3),
            ],
            groups: vec![
                StopGroup { name: 0, stops: vec![S1] },
                StopGroup { name: 1, stops: vec![H1, H2] },
                StopGroup { name: 2, stops: vec![T1, T2] },
                StopGroup { name: 3, stops: vec![J1] },
            ],
            routes: vec![
                Route { gtfs_id: "L9".into(), short_name: "9".into(), route_type: 0, color: 0 },
                Route { gtfs_id: "LA".into(), short_name: "A".into(), route_type: 1, color: 0 },
            ],
            patterns: vec![
                Pattern { route: 0, stops: vec![S1, H1, T1], flags: vec![0; 3] },
                Pattern { route: 1, stops: vec![H2, T2], flags: vec![0; 2] },
            ],
            profiles: vec![
                Profile { pattern: 0, offsets: vec![0, 300, 2400] },
                Profile { pattern: 1, offsets: vec![0, 600] },
            ],
            trips: vec![trip(0, hm(8, 0)), trip(1, hm(8, 8)), trip(1, hm(8, 10)), trip(1, hm(8, 20))],
            services: vec![every_day()],
            footpaths: vec![Vec::new(); 6],
        }
    }

    fn net() -> TimetableIndex {
        TimetableIndex::new(timetable())
    }

    fn t(h: u32, m: u32) -> DateTime<Tz> {
        TZ.with_ymd_and_hms(2026, 11, 16, h, m, 0).unwrap()
    }

    fn query(time: DateTime<Tz>) -> JourneyQuery {
        JourneyQuery {
            origins: vec![Access { stop: S1, walk_s: 0 }],
            target_group: CIL,
            mode: TimeMode::DepartAt,
            time,
            excluded: Vec::new(),
            buffer_s: 0,
        }
    }

    /// Jízdy spojení jako (nástup, výstup, odjezd, příjezd).
    fn rides(j: &Journey) -> Vec<(u32, u32, DateTime<Tz>, DateTime<Tz>)> {
        j.legs
            .iter()
            .filter_map(|l| match *l {
                Leg::Ride { board_stop, alight_stop, dep, arr, .. } => Some((board_stop, alight_stop, dep, arr)),
                Leg::Walk { .. } => None,
            })
            .collect()
    }

    #[test]
    fn pareto_fast_and_direct() {
        let js = net().plan_once(&query(t(7, 55)));
        assert_eq!(js.len(), 2);
        let direct = js.iter().find(|j| j.transfers == 0).unwrap();
        assert_eq!(direct.arrive, t(8, 40));
        assert_eq!(rides(direct), vec![(S1, T1, t(8, 0), t(8, 40))]);
        assert_eq!(direct.walk_s, 0);
        let fast = js.iter().find(|j| j.transfers == 1).unwrap();
        assert_eq!((fast.leave, fast.arrive), (t(8, 0), t(8, 20)));
        assert_eq!(rides(fast), vec![(S1, H1, t(8, 0), t(8, 5)), (H2, T2, t(8, 10), t(8, 20))]);
        assert_eq!(fast.legs[1], Leg::Walk { from: Some(H1), to: H2, seconds: 240 });
        assert_eq!(fast.walk_s, 240);
    }

    #[test]
    fn metro_transfer_needs_four_minutes() {
        let js = net().plan_once(&query(t(7, 55)));
        let metro_deps: Vec<_> = js.iter().flat_map(rides).filter(|r| r.0 == H2).map(|r| r.2).collect();
        assert_eq!(metro_deps, vec![t(8, 10)]);
    }

    #[test]
    fn excluded_metro_only_direct() {
        let mut q = query(t(7, 55));
        q.excluded = vec![RouteKind::Metro];
        let js = net().plan_once(&q);
        assert_eq!(js.len(), 1);
        assert_eq!((js[0].arrive, js[0].transfers), (t(8, 40), 0));
    }

    #[test]
    fn buffer_blocks_tight_first_boarding() {
        // Služba jen v den dotazu, aby se nenabídl zítřejší ranní spoj.
        let mut tt = timetable();
        let day = (t(0, 0).date_naive() - feed_start()).num_days() as usize;
        tt.services[0].days.iter_mut().for_each(|b| *b = 0);
        tt.services[0].days[day / 8] |= 1 << (day % 8);
        let ix = TimetableIndex::new(tt);
        let mut q = query(t(7, 59));
        q.buffer_s = 120;
        assert!(ix.plan_once(&q).is_empty());
        q.buffer_s = 0;
        assert!(!ix.plan_once(&q).is_empty());
    }

    #[test]
    fn access_walk_sets_leave() {
        let mut q = query(t(7, 50));
        q.origins = vec![Access { stop: S1, walk_s: 300 }];
        let js = net().plan_once(&q);
        let direct = js.iter().find(|j| j.transfers == 0).unwrap();
        assert_eq!(direct.leave, t(7, 55));
        assert_eq!(direct.legs[0], Leg::Walk { from: None, to: S1, seconds: 300 });
        assert_eq!(direct.walk_s, 300);
    }

    #[test]
    fn arrive_by_latest() {
        let mut q = query(t(8, 35));
        q.mode = TimeMode::ArriveBy;
        let js = net().plan_once(&q);
        // Přímá tramvaj dnes přijede až 8:40; bez přestupu jen včerejší (okno D−1..D+1).
        assert_eq!(js.len(), 2);
        let yesterday = |h, m| TZ.with_ymd_and_hms(2026, 11, 15, h, m, 0).unwrap();
        assert_eq!((js[0].transfers, js[0].arrive), (0, yesterday(8, 40)));
        let j = &js[1];
        assert_eq!((j.leave, j.arrive, j.transfers), (t(8, 0), t(8, 30), 1));
        assert_eq!(rides(j), vec![(S1, H1, t(8, 0), t(8, 5)), (H2, T2, t(8, 20), t(8, 30))]);
        assert_eq!(j.legs[1], Leg::Walk { from: Some(H1), to: H2, seconds: 240 });
    }

    #[test]
    fn finds_trip_after_midnight() {
        let mut tt = timetable();
        tt.trips.push(trip(0, hm(24, 5)));
        let ix = TimetableIndex::new(tt);
        let next = |h, m| TZ.with_ymd_and_hms(2026, 11, 17, h, m, 0).unwrap();
        // Dnešní služba (D) večer i včerejší služba (D−1) při dotazu po půlnoci.
        for time in [t(23, 55), next(0, 0)] {
            let js = ix.plan_once(&query(time));
            let direct = js.iter().find(|j| j.transfers == 0).unwrap();
            assert_eq!(rides(direct)[0].2, next(0, 5));
            assert_eq!(direct.arrive, next(0, 45));
        }
    }

    #[test]
    fn same_group_returns_empty() {
        let mut q = query(t(7, 55));
        q.target_group = START;
        assert!(net().plan_once(&q).is_empty());
    }

    #[test]
    fn all_excluded_returns_empty() {
        let mut q = query(t(7, 55));
        q.excluded = RouteKind::ALL.to_vec();
        assert!(net().plan_once(&q).is_empty());
    }

    #[test]
    fn query_on_last_feed_day() {
        let at = |h, m| TZ.from_local_datetime(&feed_end().and_hms_opt(h, m, 0).unwrap()).unwrap();
        assert_eq!(net().plan_once(&query(at(7, 55))).len(), 2);
        // Večer posledního dne už nic nejede (D+1 je mimo feed).
        assert!(net().plan_once(&query(at(22, 0))).is_empty());
    }

    #[test]
    fn footpath_to_target() {
        let mut tt = timetable();
        tt.footpaths[T1 as usize].push(Footpath { to: J1, walk_s: 180 });
        tt.footpaths[J1 as usize].push(Footpath { to: T1, walk_s: 180 });
        let ix = TimetableIndex::new(tt);
        let mut q = query(t(7, 55));
        q.target_group = JINDE;
        let js = ix.plan_once(&q);
        assert_eq!(js.len(), 1);
        let j = &js[0];
        assert_eq!(j.arrive, t(8, 43));
        assert_eq!(rides(j), vec![(S1, T1, t(8, 0), t(8, 40))]);
        assert_eq!(j.legs.last(), Some(&Leg::Walk { from: Some(T1), to: J1, seconds: 180 }));
        assert_eq!(j.walk_s, 180);
        // Zpětně totéž.
        q.mode = TimeMode::ArriveBy;
        q.time = t(8, 45);
        let back = ix.plan_once(&q);
        assert_eq!(back.len(), 1);
        assert_eq!((back[0].leave, back[0].arrive), (t(8, 0), t(8, 43)));
        assert_eq!(back[0].legs, j.legs);
    }

    #[test]
    fn no_pickup_no_dropoff_respected() {
        let mut tt = timetable();
        tt.patterns[TRAM_PATTERN].flags[0] = NO_PICKUP;
        assert!(TimetableIndex::new(tt).plan_once(&query(t(7, 55))).is_empty());

        let mut tt = timetable();
        tt.patterns[TRAM_PATTERN].flags[1] = NO_DROP_OFF;
        let js = TimetableIndex::new(tt).plan_once(&query(t(7, 55)));
        assert_eq!(js.len(), 1);
        assert_eq!(js[0].arrive, t(8, 40));
    }
}
