//! Převod PID GTFS na kompaktní jízdní řád (`pid_core::timetable`).

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use pid_core::timetable::codec::encode;
use pid_core::timetable::{
    NO_DROP_OFF, NO_PICKUP, Pattern, Profile, Route, Service, Stop, StopGroup, Timetable, Trip,
};

pub const PID_GTFS_URL: &str = "https://data.pid.cz/PID_GTFS.zip";
pub const MAX_BYTES: u64 = 15 * 1024 * 1024;

/// Soubory GTFS, které potřebujeme (shapes.txt má 144 MB a nepotřebujeme ho).
const NEEDED: [&str; 7] = [
    "feed_info.txt",
    "routes.txt",
    "stops.txt",
    "trips.txt",
    "stop_times.txt",
    "calendar.txt",
    "calendar_dates.txt",
];

pub enum FeedSource {
    Fetch,
    Path(PathBuf),
}

/// Připraví rozbalenou složku s GTFS. Zip (stažený nebo lokální) rozbalí do `work_dir/feed`.
pub fn prepare_feed(source: FeedSource, work_dir: &Path) -> Result<PathBuf> {
    let zip_path = match source {
        FeedSource::Path(p) if p.is_dir() => return Ok(p),
        FeedSource::Path(p) => p,
        FeedSource::Fetch => {
            std::fs::create_dir_all(work_dir)?;
            let target = work_dir.join("PID_GTFS.zip");
            eprintln!("stahuji {PID_GTFS_URL} …");
            let mut body = ureq::get(PID_GTFS_URL).call()?.into_body();
            let mut file = File::create(&target)?;
            std::io::copy(&mut body.as_reader(), &mut file)?;
            target
        }
    };
    let out = work_dir.join("feed");
    std::fs::create_dir_all(&out)?;
    let mut archive = zip::ZipArchive::new(File::open(&zip_path).with_context(|| format!("{}", zip_path.display()))?)?;
    for name in NEEDED {
        let mut entry = archive.by_name(name).with_context(|| format!("{name} chybí v zipu"))?;
        let mut file = File::create(out.join(name))?;
        std::io::copy(&mut entry, &mut file)?;
    }
    Ok(out)
}

/// CSV čtečka se sloupci podle názvu (odolná vůči BOM a pořadí sloupců).
struct Csv {
    reader: csv::Reader<BufReader<File>>,
    columns: HashMap<String, usize>,
}

impl Csv {
    fn open(dir: &Path, name: &str) -> Result<Self> {
        let file = File::open(dir.join(name)).with_context(|| format!("{name} nelze otevřít"))?;
        let mut reader = csv::ReaderBuilder::new().flexible(true).from_reader(BufReader::new(file));
        let columns = reader
            .headers()?
            .iter()
            .enumerate()
            .map(|(i, h)| (h.trim_start_matches('\u{feff}').trim().to_string(), i))
            .collect();
        Ok(Csv { reader, columns })
    }

    fn col(&self, name: &str) -> Result<usize> {
        self.columns.get(name).copied().with_context(|| format!("chybí sloupec {name}"))
    }

    fn opt_col(&self, name: &str) -> Option<usize> {
        self.columns.get(name).copied()
    }
}

fn field<'a>(rec: &'a csv::StringRecord, idx: usize) -> &'a str {
    rec.get(idx).unwrap_or("").trim()
}

fn opt_field(rec: &csv::StringRecord, idx: Option<usize>) -> &str {
    idx.map(|i| field(rec, i)).unwrap_or("")
}

fn parse_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y%m%d").with_context(|| format!("neplatné datum {s}"))
}

fn parse_gtfs_time(s: &str) -> Option<u32> {
    let mut parts = s.split(':').map(|p| p.trim().parse::<u32>().ok());
    let (h, m, sec) = (parts.next()??, parts.next()??, parts.next()??);
    Some(h * 3600 + m * 60 + sec)
}

#[derive(Default)]
struct Strings {
    list: Vec<String>,
    index: HashMap<String, u32>,
}

impl Strings {
    fn intern(&mut self, s: &str) -> u32 {
        if let Some(&i) = self.index.get(s) {
            return i;
        }
        let i = self.list.len() as u32;
        self.list.push(s.to_string());
        self.index.insert(s.to_string(), i);
        i
    }
}

pub fn build_timetable(gtfs_dir: &Path, generated_at: DateTime<Utc>) -> Result<Timetable> {
    let mut strings = Strings::default();

    // feed_info
    let mut csv = Csv::open(gtfs_dir, "feed_info.txt")?;
    let (c_start, c_end) = (csv.col("feed_start_date")?, csv.col("feed_end_date")?);
    let rec = csv.reader.records().next().context("feed_info.txt je prázdný")??;
    let feed_start = parse_date(field(&rec, c_start))?;
    let feed_end = parse_date(field(&rec, c_end))?;
    let days = (feed_end - feed_start).num_days() as usize + 1;

    // routes
    let mut csv = Csv::open(gtfs_dir, "routes.txt")?;
    let (c_id, c_short, c_type, c_color) = (
        csv.col("route_id")?,
        csv.col("route_short_name")?,
        csv.col("route_type")?,
        csv.opt_col("route_color"),
    );
    let mut routes = Vec::new();
    let mut route_index = HashMap::new();
    for rec in csv.reader.records() {
        let rec = rec?;
        route_index.insert(field(&rec, c_id).to_string(), routes.len() as u32);
        routes.push(Route {
            gtfs_id: field(&rec, c_id).to_string(),
            short_name: field(&rec, c_short).to_string(),
            route_type: field(&rec, c_type).parse().unwrap_or(3),
            color: u32::from_str_radix(opt_field(&rec, c_color), 16).unwrap_or(0x00559e),
        });
    }

    // stops (jen stanoviště, location_type 0)
    let mut csv = Csv::open(gtfs_dir, "stops.txt")?;
    let (c_id, c_name, c_lat, c_lon) =
        (csv.col("stop_id")?, csv.col("stop_name")?, csv.col("stop_lat")?, csv.col("stop_lon")?);
    let (c_loc, c_platform, c_node) =
        (csv.opt_col("location_type"), csv.opt_col("platform_code"), csv.opt_col("asw_node_id"));
    let mut stops = Vec::new();
    let mut stop_index = HashMap::new();
    let mut groups: Vec<StopGroup> = Vec::new();
    let mut group_index: HashMap<String, u32> = HashMap::new();
    for rec in csv.reader.records() {
        let rec = rec?;
        if !matches!(opt_field(&rec, c_loc), "" | "0") {
            continue;
        }
        let name = field(&rec, c_name);
        let node = opt_field(&rec, c_node);
        let key = if node.is_empty() { format!("name:{name}") } else { format!("node:{node}") };
        let stop_idx = stops.len() as u32;
        let group = *group_index.entry(key).or_insert_with(|| {
            groups.push(StopGroup { name: strings.intern(name), stops: Vec::new() });
            groups.len() as u32 - 1
        });
        groups[group as usize].stops.push(stop_idx);
        let platform = opt_field(&rec, c_platform);
        let deg = |s: &str| (s.parse::<f64>().unwrap_or(0.0) * 1e6).round() as i32;
        stop_index.insert(field(&rec, c_id).to_string(), stop_idx);
        stops.push(Stop {
            gtfs_id: field(&rec, c_id).to_string(),
            name: strings.intern(name),
            platform: (!platform.is_empty()).then(|| strings.intern(platform)),
            lat_e6: deg(field(&rec, c_lat)),
            lon_e6: deg(field(&rec, c_lon)),
            group,
        });
    }

    // calendar + calendar_dates → bitmapy
    let mut services: Vec<Service> = Vec::new();
    let mut service_index: HashMap<String, u32> = HashMap::new();
    let mut service_of = |id: &str, services: &mut Vec<Service>| -> u32 {
        *service_index.entry(id.to_string()).or_insert_with(|| {
            services.push(Service { days: vec![0; days.div_ceil(8)] });
            services.len() as u32 - 1
        })
    };
    let set_day = |svc: &mut Service, date: NaiveDate, on: bool| {
        let d = (date - feed_start).num_days();
        if d < 0 || d as usize >= days {
            return;
        }
        let d = d as usize;
        if on {
            svc.days[d / 8] |= 1 << (d % 8);
        } else {
            svc.days[d / 8] &= !(1 << (d % 8));
        }
    };
    let mut csv = Csv::open(gtfs_dir, "calendar.txt")?;
    let c_sid = csv.col("service_id")?;
    let weekday_cols = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"]
        .map(|d| csv.col(d))
        .into_iter()
        .collect::<Result<Vec<_>>>()?;
    let (c_s, c_e) = (csv.col("start_date")?, csv.col("end_date")?);
    for rec in csv.reader.records() {
        let rec = rec?;
        let svc = service_of(field(&rec, c_sid), &mut services);
        let (start, end) = (parse_date(field(&rec, c_s))?, parse_date(field(&rec, c_e))?);
        let mut date = start.max(feed_start);
        while date <= end.min(feed_end) {
            let wd = date.weekday().num_days_from_monday() as usize;
            if field(&rec, weekday_cols[wd]) == "1" {
                set_day(&mut services[svc as usize], date, true);
            }
            date = date.succ_opt().unwrap();
        }
    }
    let mut csv = Csv::open(gtfs_dir, "calendar_dates.txt")?;
    let (c_sid, c_date, c_ex) = (csv.col("service_id")?, csv.col("date")?, csv.col("exception_type")?);
    for rec in csv.reader.records() {
        let rec = rec?;
        let svc = service_of(field(&rec, c_sid), &mut services);
        let on = field(&rec, c_ex) == "1";
        set_day(&mut services[svc as usize], parse_date(field(&rec, c_date))?, on);
    }

    // trips
    struct TripInfo {
        route: u32,
        service: u32,
        headsign: u32,
    }
    let mut csv = Csv::open(gtfs_dir, "trips.txt")?;
    let (c_route, c_sid, c_tid, c_head) =
        (csv.col("route_id")?, csv.col("service_id")?, csv.col("trip_id")?, csv.opt_col("trip_headsign"));
    let mut trip_infos: HashMap<String, TripInfo> = HashMap::new();
    for rec in csv.reader.records() {
        let rec = rec?;
        let Some(&route) = route_index.get(field(&rec, c_route)) else { continue };
        let service = service_of(field(&rec, c_sid), &mut services);
        trip_infos.insert(
            field(&rec, c_tid).to_string(),
            TripInfo { route, service, headsign: strings.intern(opt_field(&rec, c_head)) },
        );
    }

    // stop_times (≈ 2 M řádků) – seskupit podle trip_id
    let mut csv = Csv::open(gtfs_dir, "stop_times.txt")?;
    let (c_tid, c_arr, c_dep, c_stop, c_seq) = (
        csv.col("trip_id")?,
        csv.col("arrival_time")?,
        csv.col("departure_time")?,
        csv.col("stop_id")?,
        csv.col("stop_sequence")?,
    );
    let (c_pick, c_drop) = (csv.opt_col("pickup_type"), csv.opt_col("drop_off_type"));
    // (sequence, stop, departure secs, flags)
    let mut per_trip: HashMap<String, Vec<(u32, u32, u32, u8)>> = HashMap::new();
    let mut rec = csv::StringRecord::new();
    while csv.reader.read_record(&mut rec)? {
        let trip_id = field(&rec, c_tid);
        if !trip_infos.contains_key(trip_id) {
            continue;
        }
        let Some(&stop) = stop_index.get(field(&rec, c_stop)) else { continue };
        let Some(dep) = parse_gtfs_time(field(&rec, c_dep)).or_else(|| parse_gtfs_time(field(&rec, c_arr))) else {
            continue;
        };
        let seq = field(&rec, c_seq).parse().unwrap_or(0);
        let mut flags = 0;
        if opt_field(&rec, c_pick) == "1" {
            flags |= NO_PICKUP;
        }
        if opt_field(&rec, c_drop) == "1" {
            flags |= NO_DROP_OFF;
        }
        match per_trip.get_mut(trip_id) {
            Some(v) => v.push((seq, stop, dep, flags)),
            None => {
                per_trip.insert(trip_id.to_string(), vec![(seq, stop, dep, flags)]);
            }
        }
    }

    // patterns, profiles, trips
    let mut patterns: Vec<Pattern> = Vec::new();
    let mut pattern_index: HashMap<(u32, Vec<u32>, Vec<u8>), u32> = HashMap::new();
    let mut profiles: Vec<Profile> = Vec::new();
    let mut profile_index: HashMap<(u32, Vec<u32>), u32> = HashMap::new();
    let mut trips = Vec::new();
    let mut trip_ids: Vec<_> = per_trip.keys().cloned().collect();
    trip_ids.sort();
    for trip_id in trip_ids {
        let mut st = per_trip.remove(&trip_id).unwrap();
        if st.len() < 2 {
            continue;
        }
        st.sort_by_key(|x| x.0);
        let info = &trip_infos[&trip_id];
        let stops_seq: Vec<u32> = st.iter().map(|x| x.1).collect();
        let flags: Vec<u8> = st.iter().map(|x| x.3).collect();
        let start = st[0].2;
        let offsets: Vec<u32> = st.iter().map(|x| x.2.saturating_sub(start)).collect();
        let pattern = *pattern_index.entry((info.route, stops_seq.clone(), flags.clone())).or_insert_with(|| {
            patterns.push(Pattern { route: info.route, stops: stops_seq, flags });
            patterns.len() as u32 - 1
        });
        let profile = *profile_index.entry((pattern, offsets.clone())).or_insert_with(|| {
            profiles.push(Profile { pattern, offsets });
            profiles.len() as u32 - 1
        });
        trips.push(Trip { gtfs_id: trip_id, profile, service: info.service, start, headsign: info.headsign });
    }
    trips.sort_by_key(|t| (t.profile, t.start));

    Ok(Timetable {
        version: format!("{}-{}", feed_start.format("%Y%m%d"), generated_at.format("%Y%m%d%H%M")),
        feed_start,
        feed_end,
        strings: strings.list,
        stops,
        groups,
        routes,
        patterns,
        profiles,
        trips,
        services,
    })
}

pub struct OutputInfo {
    pub version: String,
    pub file: String,
    pub bytes: u64,
}

/// Zapíše `timetable-<version>.pidt` a `manifest.json` do `out_dir`.
pub fn write_output(t: &Timetable, out_dir: &Path) -> Result<OutputInfo> {
    std::fs::create_dir_all(out_dir)?;
    let file = format!("timetable-{}.pidt", t.version);
    let bytes = encode(t);
    std::fs::write(out_dir.join(&file), &bytes)?;
    let manifest = serde_json::json!({
        "version": t.version,
        "file": file,
        "feed_start": t.feed_start.format("%Y-%m-%d").to_string(),
        "feed_end": t.feed_end.format("%Y-%m-%d").to_string(),
        "bytes": bytes.len(),
    });
    std::fs::write(out_dir.join("manifest.json"), serde_json::to_string_pretty(&manifest)?)?;
    let info = OutputInfo { version: t.version.clone(), file, bytes: bytes.len() as u64 };
    if info.bytes > MAX_BYTES {
        bail!("jízdní řád má {} B, limit je {MAX_BYTES} B", info.bytes);
    }
    Ok(info)
}
