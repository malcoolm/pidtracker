//! Hledání spojení (RAPTOR) nad jízdním řádem: odjezd v čase i příjezd do času.

use chrono::{DateTime, Duration, NaiveDate};
use chrono_tz::Tz;

use super::query::{TimetableIndex, TripStop};
use super::walk::{ACCESS_MAX_M, MAX_TRANSFERS, METRO_ACCESS_EXTRA_S, transfer_seconds, walk_seconds};
use super::{NO_DROP_OFF, NO_PICKUP};
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
    /// Zastávky jízdy od nástupu po výstup včetně, s časy ve dni služby jízdy
    /// (odvozeném z `dep`). Pro `Walk` prázdné.
    pub fn leg_stops(&self, leg: &Leg) -> Vec<TripStop> {
        let Leg::Ride { trip, board_pos, alight_pos, dep, .. } = *leg else {
            return Vec::new();
        };
        let trip = &self.tt.trips[trip as usize];
        let profile = &self.tt.profiles[trip.profile as usize];
        let pattern = &self.tt.patterns[profile.pattern as usize];
        let (from, to) = (board_pos as usize, alight_pos as usize);
        let day = dep.date_naive();
        let Some(service_date) = [day - Duration::days(1), day, day + Duration::days(1)]
            .into_iter()
            .find(|&d| gtfs_instant(d, trip.start + profile.offsets[from]) == dep)
        else {
            return Vec::new();
        };
        (from..=to)
            .map(|pos| {
                let stop = pattern.stops[pos];
                let s = &self.tt.stops[stop as usize];
                TripStop {
                    stop,
                    gtfs_id: s.gtfs_id.clone(),
                    name: self.tt.stop_name(stop).to_string(),
                    lat: s.lat_e6 as f64 / 1e6,
                    lon: s.lon_e6 as f64 / 1e6,
                    time: gtfs_instant(service_date, trip.start + profile.offsets[pos]),
                }
            })
            .collect()
    }

    /// Jedno hledání: Pareto množina spojení (nejlepší čas pro každý počet přestupů
    /// 0..=`MAX_TRANSFERS`, jen zlepšující se), seřazená podle počtu přestupů.
    pub fn plan_once(&self, q: &JourneyQuery) -> Vec<Journey> {
        self.plan_once_with(q, &DayFilter::new(self, q.time.date_naive()))
    }

    /// `plan_once` s předpočítaným předfiltrem dnů (musí patřit ke dni `q.time`).
    fn plan_once_with(&self, q: &JourneyQuery, filter: &DayFilter) -> Vec<Journey> {
        debug_assert_eq!(filter.date, q.time.date_naive());
        let origins = self.origin_ends(q);
        if origins.is_empty() {
            return Vec::new();
        }
        let targets = self.target_ends(q.target_group);
        let fwd = q.mode == TimeMode::DepartAt;
        let (sources, sinks) = if fwd { (origins, targets) } else { (targets, origins) };
        let mut r = Raptor::new(self, q, filter, fwd, sources, sinks);
        let start = q.time.timestamp();
        r.run(if fwd { start } else { -start });
        (1..=ROUNDS)
            .filter_map(|k| r.found[k].map(|sink| r.journey(q, &r.steps(k, sink))))
            .collect()
    }

    /// Stanoviště uzlů do `ACCESS_MAX_M` od bodu, každé se svým `walk_seconds`.
    /// Stačí, aby do limitu bylo jedno stanoviště uzlu; přidají se všechna jeho stanoviště.
    pub fn access_from(&self, lat: f64, lon: f64) -> Vec<Access> {
        let stops = &self.tt.stops;
        let dist: Vec<f64> = (0..stops.len()).map(|i| self.stop_distance(i as u32, lat, lon)).collect();
        let mut near = vec![false; self.tt.groups.len()];
        for (s, &d) in stops.iter().zip(&dist) {
            if d <= ACCESS_MAX_M {
                near[s.group as usize] = true;
            }
        }
        stops
            .iter()
            .zip(&dist)
            .enumerate()
            .filter(|(_, (s, _))| near[s.group as usize])
            .map(|(i, (_, &d))| Access { stop: i as u32, walk_s: walk_seconds(d) })
            .collect()
    }

    /// Všechna stanoviště uzlu s `walk_s = 0`.
    pub fn access_group(&self, group: u32) -> Vec<Access> {
        self.tt.groups[group as usize].stops.iter().map(|&stop| Access { stop, walk_s: 0 }).collect()
    }

    /// 3–5 spojení (min. `min_count`), seřazená podle `leave`. Opakuje `plan_once`
    /// s časem posunutým za nejdřívější odchod (u příjezdu do: před nejpozdější příjezd)
    /// předchozího běhu, každé spojení utáhne, slučuje duplicity a skrývá dominovaná.
    /// Při přebytku vrací u odjezdu nejdřívější, u příjezdu do nejpozdější spojení
    /// (tj. vždy ta nejblíž zadanému času).
    pub fn plan(&self, q: &JourneyQuery, min_count: usize) -> Vec<Journey> {
        let fwd = q.mode == TimeMode::DepartAt;
        let mut q = q.clone();
        let mut out: Vec<Journey> = Vec::new();
        let mut filter = DayFilter::new(self, q.time.date_naive());
        for _ in 0..PLAN_MAX_RUNS {
            if filter.date != q.time.date_naive() {
                filter = DayFilter::new(self, q.time.date_naive());
            }
            let found = self.plan_once_with(&q, &filter);
            // Posun se počítá z neutažených časů běhu, aby se nepřeskočila žádná spojení.
            let next = if fwd {
                found.iter().map(|j| j.leave).min().map(|t| t + Duration::seconds(PLAN_STEP_S))
            } else {
                found.iter().map(|j| j.arrive).max().map(|t| t - Duration::seconds(PLAN_STEP_S))
            };
            let Some(next) = next else { break };
            let found: Vec<Journey> = found.into_iter().map(|j| self.tighten(j, q.buffer_s)).collect();
            for j in drop_far_off(found, fwd) {
                merge_journey(&mut out, j);
            }
            drop_dominated(&mut out);
            if out.len() >= min_count {
                break;
            }
            q.time = next;
        }
        out.sort_by_key(|j| (j.leave, j.arrive, j.transfers));
        if out.len() > PLAN_MAX_RESULTS {
            if fwd {
                out.truncate(PLAN_MAX_RESULTS);
            } else {
                out.drain(..out.len() - PLAN_MAX_RESULTS);
            }
        }
        out
    }

    /// Posune dřívější spoje na nejpozdější, které stihnou stejné navazující spoje
    /// (zpětně od posledního, který zůstává). Spoj se vyměňuje jen za spoj stejného
    /// patternu (stejná linka i stanoviště); `leave` se přepočítá z prvního nástupu.
    pub fn tighten(&self, mut j: Journey, buffer_s: u32) -> Journey {
        let rides: Vec<usize> =
            j.legs.iter().enumerate().filter(|(_, l)| matches!(l, Leg::Ride { .. })).map(|(i, _)| i).collect();
        let Some(&first) = rides.first() else { return j };
        for w in (0..rides.len() - 1).rev() {
            let (i, next) = (rides[w], rides[w + 1]);
            let Leg::Ride { dep: next_dep, board_stop: next_stop, .. } = j.legs[next] else { unreachable!() };
            // Mezi jízdami je buď úsek chůze s už započteným přestupem, nebo nic (stejné stanoviště).
            let walk = j.legs[i + 1..next].iter().find_map(|l| match *l {
                Leg::Walk { seconds, .. } => Some(seconds),
                Leg::Ride { .. } => None,
            });
            let Leg::Ride { trip, board_pos, alight_pos, alight_stop, dep, arr, .. } = &mut j.legs[i] else {
                unreachable!()
            };
            let transfer = walk.unwrap_or_else(|| {
                transfer_seconds(0, self.is_metro_stop(*alight_stop), self.is_metro_stop(next_stop))
            });
            let deadline = next_dep - Duration::seconds(i64::from(transfer));
            if let Some((t, d, a)) = self.latest_trip(*trip, *board_pos, *alight_pos, *dep, deadline) {
                (*trip, *dep, *arr) = (t, d, a);
            }
        }
        let Leg::Ride { board_stop, dep, .. } = j.legs[first] else { unreachable!() };
        let access = match j.legs.first() {
            Some(&Leg::Walk { from: None, seconds, .. }) => seconds,
            _ => 0,
        };
        j.leave = dep - Duration::seconds(i64::from(self.access_extra(board_stop, access, buffer_s)));
        j
    }

    /// Spoj stejného patternu jako `trip` s nejpozdějším odjezdem z `board_pos`
    /// (později než `after`), který na `alight_pos` přijede nejpozději v `deadline`.
    /// Vrací (spoj, odjezd, příjezd).
    fn latest_trip(
        &self,
        trip: u32,
        board_pos: u16,
        alight_pos: u16,
        after: DateTime<Tz>,
        deadline: DateTime<Tz>,
    ) -> Option<(u32, DateTime<Tz>, DateTime<Tz>)> {
        let tt = &self.tt;
        let pattern = tt.profiles[tt.trips[trip as usize].profile as usize].pattern;
        let deadline = deadline.timestamp();
        let mut best_dep = after.timestamp();
        // (spoj, servisní den, start + offset nástupu, start + offset výstupu)
        let mut best: Option<(u32, NaiveDate, u32, u32)> = None;
        let d = after.date_naive();
        for &prof in &self.pattern_profiles[pattern as usize] {
            let offsets = &tt.profiles[prof as usize].offsets;
            let (board, alight) = (offsets[board_pos as usize], offsets[alight_pos as usize]);
            let (lo, hi) = self.profile_trips[prof as usize];
            let trips = &tt.trips[lo..hi];
            for day in [d - Duration::days(1), d, d + Duration::days(1)] {
                let base = gtfs_instant(day, 0).timestamp();
                let max_start = deadline - base - i64::from(alight);
                let end = trips.partition_point(|t| i64::from(t.start) <= max_start);
                let Some(i) = trips[..end].iter().rposition(|t| tt.service_runs(t.service, day)) else { continue };
                let start = trips[i].start;
                let dep = base + i64::from(start + board);
                if dep > best_dep {
                    best_dep = dep;
                    best = Some(((lo + i) as u32, day, start + board, start + alight));
                }
            }
        }
        best.map(|(t, day, dep, arr)| (t, gtfs_instant(day, dep), gtfs_instant(day, arr)))
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

/// Štítky seznamu spojení (indexy do seznamu); může jít o totéž spojení.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Labels {
    pub fastest: Option<usize>,
    pub fewest: Option<usize>,
}

/// „Nejrychlejší“ = nejkratší `arrive − leave`; „nejméně přestupů“ = nejméně
/// přestupů, při shodě dřívější příjezd.
pub fn labels(js: &[Journey]) -> Labels {
    let fastest = js.iter().enumerate().min_by_key(|(_, j)| j.arrive - j.leave).map(|(i, _)| i);
    let fewest = js.iter().enumerate().min_by_key(|(_, j)| (j.transfers, j.arrive)).map(|(i, _)| i);
    Labels { fastest, fewest }
}

/// Nejvyšší počet běhů `plan_once` v jednom `plan`.
const PLAN_MAX_RUNS: usize = 8;
/// Nejvyšší počet vrácených spojení.
const PLAN_MAX_RESULTS: usize = 5;
/// Posun času dalšího běhu za předchozí výsledek.
const PLAN_STEP_S: i64 = 60;
/// Nejmenší tolerance pro Pareto kandidáty jednoho běhu (viz `drop_far_off`).
pub const PARETO_SLACK_MIN_S: i64 = 30 * 60;

/// Z kandidátů jednoho běhu vyřadí ty příliš vzdálené od nejlepšího (např. přímý noční
/// spoj o hodiny dřív jen proto, že má méně přestupů). Odjezd: nejlepší = nejdřívější
/// příjezd `a*` s dobou `d*`, vyřadí se příjezd po `a* + max(30 min, d*)`. Příjezd do:
/// nejlepší = nejpozdější odchod `l*`, vyřadí se odchod před `l* − max(30 min, d*)`.
/// Při shodě je nejlepší to kratší.
fn drop_far_off(js: Vec<Journey>, fwd: bool) -> Vec<Journey> {
    let duration = |j: &Journey| (j.arrive - j.leave).num_seconds();
    let best = if fwd {
        js.iter().min_by_key(|j| (j.arrive, duration(j)))
    } else {
        js.iter().min_by_key(|j| (std::cmp::Reverse(j.leave), duration(j)))
    };
    let Some(best) = best else { return js };
    let slack = Duration::seconds(PARETO_SLACK_MIN_S.max(duration(best)));
    let (arrive_max, leave_min) = (best.arrive + slack, best.leave - slack);
    js.into_iter().filter(|j| if fwd { j.arrive <= arrive_max } else { j.leave >= leave_min }).collect()
}

fn trip_seq(j: &Journey) -> impl Iterator<Item = u32> + '_ {
    j.legs.iter().filter_map(|l| match *l {
        Leg::Ride { trip, .. } => Some(trip),
        Leg::Walk { .. } => None,
    })
}

/// Přidá spojení; duplicitu (stejná posloupnost spojů) sloučí – zůstane to s pozdějším
/// odchodem, při shodě s dřívějším příjezdem.
fn merge_journey(out: &mut Vec<Journey>, j: Journey) {
    match out.iter_mut().find(|o| trip_seq(o).eq(trip_seq(&j))) {
        Some(o) => {
            if (j.leave, std::cmp::Reverse(j.arrive)) > (o.leave, std::cmp::Reverse(o.arrive)) {
                *o = j;
            }
        }
        None => out.push(j),
    }
}

/// `a` dominuje `b`: odchází stejně nebo později, přijíždí stejně nebo dřív,
/// nejvýš stejně přestupů a aspoň v jednom ostře lépe.
fn dominates(a: &Journey, b: &Journey) -> bool {
    a.leave >= b.leave
        && a.arrive <= b.arrive
        && a.transfers <= b.transfers
        && (a.leave > b.leave || a.arrive < b.arrive || a.transfers < b.transfers)
}

fn drop_dominated(out: &mut Vec<Journey>) {
    let keep: Vec<bool> = out.iter().map(|b| !out.iter().any(|a| dominates(a, b))).collect();
    let mut keep = keep.into_iter();
    out.retain(|_| keep.next().unwrap());
}

/// Předfiltr pro servisní dny D−1, D, D+1 (počítá se jednou na `plan`):
/// ve které z nich jede která služba a které profily nejedou vůbec.
struct DayFilter {
    date: NaiveDate,
    /// služba → bit i = jede v den D−1+i
    services: Vec<u8>,
    /// profil → jede v některém z dnů některý jeho spoj
    profiles: Vec<bool>,
}

impl DayFilter {
    fn new(ix: &TimetableIndex, date: NaiveDate) -> Self {
        let tt = &ix.tt;
        let days = [date - Duration::days(1), date, date + Duration::days(1)];
        let services: Vec<u8> = (0..tt.services.len() as u32)
            .map(|s| days.iter().enumerate().filter(|&(_, &d)| tt.service_runs(s, d)).fold(0, |m, (i, _)| m | 1 << i))
            .collect();
        let profiles = ix
            .profile_trips
            .iter()
            .map(|&(lo, hi)| tt.trips[lo..hi].iter().any(|t| services[t.service as usize] != 0))
            .collect();
        DayFilter { date, services, profiles }
    }

    /// Jede služba v den D−1+`day`?
    fn runs(&self, service: u32, day: usize) -> bool {
        self.services[service as usize] & (1 << day) != 0
    }
}

/// Počet kol RAPTORu (jízd) = přestupy + 1.
const ROUNDS: usize = MAX_TRANSFERS as usize + 1;
const INF: i64 = i64::MAX;
/// Nejdelší doba mezi časem dotazu a příjezdem (odjezd) / odchodem (příjezd do).
/// Spojení ze včerejška nebo na zítřek nedávají smysl.
pub const SEARCH_HORIZON_S: i64 = 12 * 3600;

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
    filter: &'a DayFilter,
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
    /// Nejlepší cena cíle zatím; na začátku mez horizontu.
    best_sink: i64,
    /// `found[k]`: cíl běhu (index do `sinks`) s nejlepší cenou po k jízdách,
    /// jen pokud zlepšuje výsledek s méně jízdami.
    found: [Option<usize>; ROUNDS + 1],
    /// Pracovní pole fronty v `scan_routes` (mezi koly vždy `NOT_QUEUED`).
    queue_pos: Vec<u32>,
}

const NOT_QUEUED: u32 = u32::MAX;

impl<'a> Raptor<'a> {
    fn new(
        ix: &'a TimetableIndex,
        q: &'a JourneyQuery,
        filter: &'a DayFilter,
        fwd: bool,
        sources: Vec<Endpoint>,
        sinks: Vec<Endpoint>,
    ) -> Self {
        let n = ix.tt.stops.len();
        let d = q.time.date_naive();
        let days = [d - Duration::days(1), d, d + Duration::days(1)];
        let bases = days.map(|day| gtfs_instant(day, 0).timestamp());
        Raptor {
            ix,
            filter,
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
            queue_pos: vec![NOT_QUEUED; ix.tt.profiles.len()],
        }
    }

    fn run(&mut self, start: i64) {
        // Horizont: cena cíle (příjezd, resp. −odchod) nejvýš start + horizont. Ceny podél
        // spojení jen rostou, takže mez zároveň ořezává štítky už během kol.
        self.best_sink = start + SEARCH_HORIZON_S + 1;
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
        let fwd = self.fwd;
        // Fronta profilů; `queue_pos[profil]` = první pozice ve směru běhu.
        let mut queue: Vec<u32> = Vec::new();
        for &stop in marked {
            for &(p, pos) in &ix.stop_patterns[stop as usize] {
                if self.excluded_pattern(p) {
                    continue;
                }
                for &prof in &ix.pattern_profiles[p as usize] {
                    if !self.filter.profiles[prof as usize] {
                        continue;
                    }
                    let slot = &mut self.queue_pos[prof as usize];
                    if *slot == NOT_QUEUED {
                        queue.push(prof);
                        *slot = pos;
                    } else if (fwd && pos < *slot) || (!fwd && pos > *slot) {
                        *slot = pos;
                    }
                }
            }
        }
        queue.sort_unstable();
        let mut improved = Vec::new();
        for prof in queue {
            let pos = std::mem::replace(&mut self.queue_pos[prof as usize], NOT_QUEUED);
            self.scan_profile(k, prof, pos as usize, &mut improved);
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
    /// Spoje s cenou ≥ `best_sink` se nehledají (nemohou nic zlepšit).
    fn catch_trip(&self, prof: u32, pos: usize, ready: i64) -> Option<(u32, u8, i64)> {
        let tt = &self.ix.tt;
        let (lo, hi) = self.ix.profile_trips[prof as usize];
        let trips = &tt.trips[lo..hi];
        let (Some(first), Some(last)) = (trips.first(), trips.last()) else { return None };
        let offset = i64::from(tt.profiles[prof as usize].offsets[pos]);
        let mut best: Option<(u32, u8, i64)> = None;
        // Den D napřed: jeho spoj obvykle vyhraje a ostatní dny se pak rychle odříznou.
        for d in [1, 0, 2] {
            let base = self.bases[d] + offset;
            let limit = best.map_or(self.best_sink, |b| b.2.min(self.best_sink));
            let cost = |t: &super::Trip| self.cost(base + i64::from(t.start));
            // Ceny spojů dne d leží mezi cenami prvního a posledního spoje profilu.
            let (lo_cost, hi_cost) = if self.fwd { (cost(first), cost(last)) } else { (cost(last), cost(first)) };
            if hi_cost < ready || lo_cost.max(ready) >= limit {
                continue;
            }
            let runs = |t: &super::Trip| self.filter.runs(t.service, d);
            let found = if self.fwd {
                let from = trips.partition_point(|t| base + i64::from(t.start) < ready);
                trips[from..].iter().take_while(|t| cost(t) < limit).position(runs).map(|i| from + i)
            } else {
                let to = trips.partition_point(|t| base + i64::from(t.start) <= -ready);
                trips[..to].iter().rev().take_while(|t| cost(t) < limit).position(runs).map(|i| to - 1 - i)
            };
            if let Some(i) = found {
                best = Some(((lo + i) as u32, d as u8, cost(&trips[i])));
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
                let walk = if to == from { 0 } else { walk_seconds(ix.stop_distance_m(from, to)) };
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
    fn leg_stops_returns_ride_segment() {
        let ix = net();
        let js = ix.plan_once(&query(t(7, 55)));
        let direct = js.iter().find(|j| j.transfers == 0).unwrap();
        let ride = direct.legs.iter().find(|l| matches!(l, Leg::Ride { .. })).unwrap();
        let stops = ix.leg_stops(ride);
        assert_eq!(stops.iter().map(|s| s.stop).collect::<Vec<_>>(), vec![S1, H1, T1]);
        assert_eq!(stops.iter().map(|s| s.time).collect::<Vec<_>>(), vec![t(8, 0), t(8, 5), t(8, 40)]);
        // Úsek uprostřed: jen H1 → T1.
        let fast = js.iter().find(|j| j.transfers == 1).unwrap();
        let second = fast.legs.iter().filter(|l| matches!(l, Leg::Ride { .. })).nth(1).unwrap();
        let seg = ix.leg_stops(second);
        assert_eq!(seg.iter().map(|s| s.stop).collect::<Vec<_>>(), vec![H2, T2]);
        assert_eq!(seg[0].time, t(8, 10));
        assert!(ix.leg_stops(&fast.legs[1]).is_empty());
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
        let ix = net();
        let mut q = query(t(7, 59));
        q.buffer_s = 120;
        assert!(ix.plan_once(&q).is_empty());
        q.buffer_s = 0;
        assert!(!ix.plan_once(&q).is_empty());
    }

    #[test]
    fn horizon_excludes_next_day() {
        // Po poslední tramvaji; zítřejší ranní spoj je za horizontem.
        assert!(net().plan_once(&query(t(8, 45))).is_empty());
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
        assert_eq!(js.len(), 1);
        let j = &js[0];
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

    /// Varianta sítě s vlastními odjezdy tramvaje (ze S1) a metra (z H2).
    fn with_trips(trams: &[u32], metros: &[u32]) -> TimetableIndex {
        let mut tt = timetable();
        tt.trips = trams.iter().map(|&s| trip(0, s)).chain(metros.iter().map(|&s| trip(1, s))).collect();
        TimetableIndex::new(tt)
    }

    /// Bez metra 8:08 a 8:10, s druhou tramvají 8:02 (H1 8:07).
    fn tight_net() -> TimetableIndex {
        with_trips(&[hm(8, 0), hm(8, 2)], &[hm(8, 20)])
    }

    fn trips_of(j: &Journey) -> Vec<u32> {
        j.legs
            .iter()
            .filter_map(|l| match *l {
                Leg::Ride { trip, .. } => Some(trip),
                Leg::Walk { .. } => None,
            })
            .collect()
    }

    #[test]
    fn tighten_moves_first_ride_later() {
        let ix = tight_net();
        let js = ix.plan_once(&query(t(7, 55)));
        let j = js.into_iter().find(|j| j.transfers == 1).unwrap();
        assert_eq!(rides(&j), vec![(S1, H1, t(8, 0), t(8, 5)), (H2, T2, t(8, 20), t(8, 30))]);
        let tight = ix.tighten(j, 0);
        assert_eq!(rides(&tight), vec![(S1, H1, t(8, 2), t(8, 7)), (H2, T2, t(8, 20), t(8, 30))]);
        assert_eq!((tight.leave, tight.arrive, tight.transfers), (t(8, 2), t(8, 30), 1));
        assert_eq!(tight.legs[1], Leg::Walk { from: Some(H1), to: H2, seconds: 240 });
    }

    #[test]
    fn plan_drops_dominated() {
        let js = tight_net().plan(&query(t(7, 55)), 3);
        let with_metro: Vec<_> = js.iter().filter(|j| rides(j).iter().any(|r| r.2 == t(8, 20))).collect();
        assert_eq!(with_metro.len(), 1, "{js:#?}");
        assert_eq!(rides(with_metro[0])[0].2, t(8, 2));
        assert_eq!(with_metro[0].leave, t(8, 2));
    }

    #[test]
    fn plan_returns_at_least_three() {
        let ix = with_trips(
            &[hm(7, 40), hm(7, 50), hm(8, 0)],
            &[hm(7, 50), hm(8, 0), hm(8, 10), hm(8, 20), hm(8, 30)],
        );
        let js = ix.plan(&query(t(7, 30)), 3);
        assert!((3..=5).contains(&js.len()), "{js:#?}");
        assert!(js.windows(2).all(|w| w[0].leave <= w[1].leave));
        let mut seqs: Vec<_> = js.iter().map(trips_of).collect();
        seqs.sort();
        seqs.dedup();
        assert_eq!(seqs.len(), js.len());
    }

    #[test]
    fn plan_arrive_by_lists_latest() {
        let ix = with_trips(
            &[hm(7, 40), hm(7, 50), hm(8, 0)],
            &[hm(7, 50), hm(8, 0), hm(8, 10), hm(8, 20), hm(8, 30)],
        );
        let mut q = query(t(8, 45));
        q.mode = TimeMode::ArriveBy;
        let js = ix.plan(&q, 3);
        assert!((3..=5).contains(&js.len()), "{js:#?}");
        assert!(js.windows(2).all(|w| w[0].leave <= w[1].leave));
        assert!(js.iter().all(|j| j.arrive <= t(8, 45)));
        // Nejpozdější odchod: tram 8:00 → metro 8:10 (příjezd 8:20) i přímý tram 8:00 (8:40).
        assert_eq!(js.last().unwrap().leave, t(8, 0));
    }

    #[test]
    fn plan_drops_far_off_pareto() {
        // Přímá tramvaj trvá 2 h (T1 až za 7200 s), s přestupem na metro je to 20–40 min.
        let slow = |trams: &[u32]| {
            let mut tt = timetable();
            tt.profiles[0].offsets = vec![0, 300, 7200];
            tt.trips = trams.iter().map(|&s| trip(0, s)).chain([8, 10, 20, 30].map(|m| trip(1, hm(8, m)))).collect();
            TimetableIndex::new(tt)
        };
        // Odjezd: nejlepší příjezd 8:20 (20 min) → přímá s příjezdem 10:00 je za 8:50 a zmizí.
        let ix = slow(&[hm(8, 0)]);
        assert_eq!(ix.plan_once(&query(t(7, 55))).len(), 2);
        let js = ix.plan(&query(t(7, 55)), 3);
        assert!(!js.is_empty());
        assert!(js.iter().all(|j| j.transfers == 1 && j.arrive <= t(8, 50)), "{js:#?}");
        // Příjezd do 8:45: nejpozdější odchod 8:00 (40 min) → přímá z 6:40 je před 7:20 a zmizí.
        let ix = slow(&[hm(6, 40), hm(7, 40), hm(7, 50), hm(8, 0)]);
        let mut q = query(t(8, 45));
        q.mode = TimeMode::ArriveBy;
        assert!(ix.plan_once(&q).iter().any(|j| j.leave == t(6, 40)));
        let js = ix.plan(&q, 3);
        assert!(!js.is_empty());
        assert!(js.iter().all(|j| j.leave >= t(7, 20)), "{js:#?}");
        assert_eq!(js.last().unwrap().leave, t(8, 0));
    }

    #[test]
    fn labels_fastest_and_fewest() {
        let js = net().plan_once(&query(t(7, 55)));
        let l = labels(&js);
        assert_eq!(js[l.fastest.unwrap()].transfers, 1);
        assert_eq!(js[l.fewest.unwrap()].transfers, 0);
        assert_eq!(labels(&[]), Labels { fastest: None, fewest: None });
    }

    /// Bod `m` metrů severně od S1.
    fn north_of_s1(m: f64) -> (f64, f64) {
        (50.10 + (m / crate::geo::EARTH_RADIUS_M).to_degrees(), 14.40)
    }

    #[test]
    fn access_from_within_1500m() {
        let ix = net();
        // O kousek blíž než 1 400 m, aby zaokrouhlení vzdálenosti nepřelezlo celou sekundu.
        let (lat, lon) = north_of_s1(1399.99);
        assert_eq!(ix.access_from(lat, lon), vec![Access { stop: S1, walk_s: walk_seconds(1400.0) }]);
        let (lat, lon) = north_of_s1(1600.0);
        assert!(ix.access_from(lat, lon).iter().all(|a| a.stop != S1));
    }

    #[test]
    fn access_group_lists_all_stops() {
        assert_eq!(net().access_group(1), vec![Access { stop: H1, walk_s: 0 }, Access { stop: H2, walk_s: 0 }]);
    }

    #[test]
    fn metro_access_extra() {
        let ix = net();
        let first_wait = |stop| {
            let mut q = query(t(7, 50));
            q.origins = vec![Access { stop, walk_s: 300 }];
            let js = ix.plan_once(&q);
            let j = &js[0];
            (rides(j)[0].2 - j.leave).num_seconds()
        };
        assert_eq!(first_wait(H1), 300);
        assert_eq!(first_wait(H2), 300 + 120);
    }

    #[test]
    fn arrive_by_respects_no_dropoff() {
        let mut tt = timetable();
        tt.patterns[TRAM_PATTERN].flags[2] = NO_DROP_OFF;
        tt.patterns[1].flags[1] = NO_DROP_OFF;
        let mut q = query(t(8, 35));
        q.mode = TimeMode::ArriveBy;
        assert!(TimetableIndex::new(tt).plan_once(&q).is_empty());
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
