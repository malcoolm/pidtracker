//! Kontrola realtime feedu: vypíše všechna pole, která feed obsahuje, i ta, která
//! `pid_core::realtime` nedekóduje (vlastní čtení protobuf wire formátu, bez schématu).
//!
//! ```text
//! curl -o vp.pb https://api.golemio.cz/v2/vehiclepositions/gtfsrt/vehicle_positions.pb
//! cargo run -p pid-core --example rt_probe -- vp.pb [počet ukázkových entit]
//! ```
//!
//! Výstup: hlavička feedu, pokrytí každého pole (kolik entit ho má, kolik různých
//! hodnot, ukázky nebo rozdělení hodnot), stáří poloh a několik celých entit.
//! Pole neznámá ze specifikace GTFS-RT se ukážou jako `#<tag>` (např. rozšíření).

use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Clone, Debug)]
enum Raw {
    Varint(u64),
    Fixed64(u64),
    Fixed32(u32),
    Bytes(Vec<u8>),
}

fn varint(b: &[u8], i: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *b.get(*i)?;
        *i += 1;
        v |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

/// Rozloží zprávu na (tag, hodnota). `None` = nejde o platnou zprávu.
fn parse(b: &[u8]) -> Option<Vec<(u32, Raw)>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let key = varint(b, &mut i)?;
        let tag = u32::try_from(key >> 3).ok().filter(|t| *t > 0)?;
        let raw = match key & 7 {
            0 => Raw::Varint(varint(b, &mut i)?),
            1 => {
                let v = u64::from_le_bytes(b.get(i..i + 8)?.try_into().ok()?);
                i += 8;
                Raw::Fixed64(v)
            }
            2 => {
                let len = usize::try_from(varint(b, &mut i)?).ok()?;
                let v = b.get(i..i.checked_add(len)?)?.to_vec();
                i += len;
                Raw::Bytes(v)
            }
            5 => {
                let v = u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?);
                i += 4;
                Raw::Fixed32(v)
            }
            _ => return None,
        };
        out.push((tag, raw));
    }
    Some(out)
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Msg,
    Str,
    Uint,
    Bool,
    Float,
    Double,
    Enum(&'static [(u64, &'static str)]),
}

const SCHEDULE_REL: &[(u64, &str)] = &[
    (0, "SCHEDULED"),
    (1, "ADDED"),
    (2, "UNSCHEDULED"),
    (3, "CANCELED"),
    (5, "REPLACEMENT"),
    (6, "DUPLICATED"),
    (7, "DELETED"),
    (8, "NEW"),
];
const STOP_STATUS: &[(u64, &str)] = &[(0, "INCOMING_AT"), (1, "STOPPED_AT"), (2, "IN_TRANSIT_TO")];
const CONGESTION: &[(u64, &str)] = &[
    (0, "UNKNOWN_CONGESTION_LEVEL"),
    (1, "RUNNING_SMOOTHLY"),
    (2, "STOP_AND_GO"),
    (3, "CONGESTION"),
    (4, "SEVERE_CONGESTION"),
];
const OCCUPANCY: &[(u64, &str)] = &[
    (0, "EMPTY"),
    (1, "MANY_SEATS_AVAILABLE"),
    (2, "FEW_SEATS_AVAILABLE"),
    (3, "STANDING_ROOM_ONLY"),
    (4, "CRUSHED_STANDING_ROOM_ONLY"),
    (5, "FULL"),
    (6, "NOT_ACCEPTING_PASSENGERS"),
    (7, "NO_DATA_AVAILABLE"),
    (8, "NOT_BOARDABLE"),
];
const WHEELCHAIR: &[(u64, &str)] = &[
    (0, "NO_VALUE"),
    (1, "UNKNOWN"),
    (2, "WHEELCHAIR_ACCESSIBLE"),
    (3, "WHEELCHAIR_INACCESSIBLE"),
];
const INCREMENTALITY: &[(u64, &str)] = &[(0, "FULL_DATASET"), (1, "DIFFERENTIAL")];

/// Název a typ pole podle specifikace GTFS-RT; `parent` je název nadřazené zprávy.
fn field(parent: &str, tag: u32) -> Option<(&'static str, Kind)> {
    use Kind::*;
    Some(match (parent, tag) {
        ("FeedMessage", 1) => ("header", Msg),
        ("FeedMessage", 2) => ("entity", Msg),
        ("header", 1) => ("gtfs_realtime_version", Str),
        ("header", 2) => ("incrementality", Enum(INCREMENTALITY)),
        ("header", 3) => ("timestamp", Uint),
        ("entity", 1) => ("id", Str),
        ("entity", 2) => ("is_deleted", Bool),
        ("entity", 3) => ("trip_update", Msg),
        ("entity", 4) => ("vehicle", Msg),
        ("entity", 5) => ("alert", Msg),
        // VehiclePosition
        ("vehicle", 1) => ("trip", Msg),
        ("vehicle", 2) => ("position", Msg),
        ("vehicle", 3) => ("current_stop_sequence", Uint),
        ("vehicle", 4) => ("current_status", Enum(STOP_STATUS)),
        ("vehicle", 5) => ("timestamp", Uint),
        ("vehicle", 6) => ("congestion_level", Enum(CONGESTION)),
        ("vehicle", 7) => ("stop_id", Str),
        ("vehicle", 8) => ("vehicle", Msg),
        ("vehicle", 9) => ("occupancy_status", Enum(OCCUPANCY)),
        ("vehicle", 10) => ("occupancy_percentage", Uint),
        ("vehicle", 11) => ("multi_carriage_details", Msg),
        // TripDescriptor
        ("trip", 1) => ("trip_id", Str),
        ("trip", 2) => ("start_time", Str),
        ("trip", 3) => ("start_date", Str),
        ("trip", 4) => ("schedule_relationship", Enum(SCHEDULE_REL)),
        ("trip", 5) => ("route_id", Str),
        ("trip", 6) => ("direction_id", Uint),
        // Position
        ("position", 1) => ("latitude", Float),
        ("position", 2) => ("longitude", Float),
        ("position", 3) => ("bearing", Float),
        ("position", 4) => ("odometer", Double),
        ("position", 5) => ("speed", Float),
        // VehicleDescriptor (uvnitř VehiclePosition se také jmenuje „vehicle“)
        ("vehicle.vehicle", 1) => ("id", Str),
        ("vehicle.vehicle", 2) => ("label", Str),
        ("vehicle.vehicle", 3) => ("license_plate", Str),
        ("vehicle.vehicle", 4) => ("wheelchair_accessible", Enum(WHEELCHAIR)),
        _ => return None,
    })
}

/// Hodnota pole jako text; zprávy se rozbalí do `fields`.
fn render(kind: Option<Kind>, raw: &Raw) -> String {
    match (kind, raw) {
        (Some(Kind::Bool), Raw::Varint(v)) => (*v != 0).to_string(),
        (Some(Kind::Enum(names)), Raw::Varint(v)) => names
            .iter()
            .find(|(n, _)| n == v)
            .map_or_else(|| format!("{v}?"), |(_, s)| (*s).to_string()),
        (_, Raw::Varint(v)) => v.to_string(),
        (Some(Kind::Double) | None, Raw::Fixed64(v)) => format!("{}", f64::from_bits(*v)),
        (_, Raw::Fixed64(v)) => format!("0x{v:016x}"),
        (_, Raw::Fixed32(v)) => format!("{}", f32::from_bits(*v)),
        (_, Raw::Bytes(b)) => match std::str::from_utf8(b) {
            Ok(s) => format!("{s:?}"),
            Err(_) => format!("<{} B>", b.len()),
        },
    }
}

/// Je to (asi) vnořená zpráva? Známé pole podle schématu, neznámé heuristikou.
fn as_message(kind: Option<Kind>, raw: &Raw) -> Option<Vec<(u32, Raw)>> {
    let Raw::Bytes(b) = raw else { return None };
    match kind {
        Some(Kind::Msg) => parse(b),
        Some(_) => None,
        // Neznámé: text, pokud je to čitelné UTF-8, jinak zkusit zprávu.
        None => {
            let printable = std::str::from_utf8(b)
                .is_ok_and(|s| s.chars().all(|c| !c.is_control() || c == '\n'));
            if printable { None } else { parse(b) }
        }
    }
}

#[derive(Default)]
struct Stat {
    entities: usize,
    values: BTreeMap<String, usize>,
}

/// Projde zprávu, do `stats` zapíše pole (cesta → hodnoty) a vrátí řádky výpisu.
fn walk(
    scope: &str,
    path: &str,
    fields: &[(u32, Raw)],
    depth: usize,
    seen: &mut BTreeSet<String>,
    stats: &mut BTreeMap<String, Stat>,
    lines: &mut Vec<String>,
) {
    for (tag, raw) in fields {
        let f = field(scope, *tag);
        let name = f.map_or_else(|| format!("#{tag}"), |(n, _)| n.to_string());
        let kind = f.map(|(_, k)| k);
        let p = if path.is_empty() {
            name.clone()
        } else {
            format!("{path}.{name}")
        };
        let st = stats.entry(p.clone()).or_default();
        if seen.insert(p.clone()) {
            st.entities += 1;
        }
        let indent = "  ".repeat(depth);
        if let Some(sub) = as_message(kind, raw) {
            *st.values.entry("<zpráva>".into()).or_default() += 1;
            lines.push(format!("{indent}{name} {{  // tag {tag}"));
            let child_scope = if scope == "vehicle" && name == "vehicle" {
                "vehicle.vehicle".to_string()
            } else {
                name.clone()
            };
            walk(&child_scope, &p, &sub, depth + 1, seen, stats, lines);
            lines.push(format!("{indent}}}"));
        } else {
            let v = render(kind, raw);
            lines.push(format!("{indent}{name}: {v}  // tag {tag}"));
            *st.values.entry(v).or_default() += 1;
        }
    }
}

fn percentile(sorted: &[i64], p: f64) -> i64 {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("cesta k .pb");
    let samples: usize = args.next().map_or(4, |n| n.parse().expect("počet ukázek"));
    let bytes = std::fs::read(&path).unwrap();
    let top = parse(&bytes).expect("není platný protobuf");

    println!("soubor: {path} ({} B)", bytes.len());
    let mut header_ts = None;
    let mut entities = Vec::new();
    for (tag, raw) in &top {
        match (tag, raw) {
            (1, Raw::Bytes(b)) => {
                println!("\n== header ==");
                for (t, r) in parse(b).unwrap() {
                    let f = field("header", t);
                    if let (3, Raw::Varint(v)) = (t, &r) {
                        header_ts = Some(*v as i64);
                    }
                    println!(
                        "  {}: {}",
                        f.map_or_else(|| format!("#{t}"), |(n, _)| n.to_string()),
                        render(f.map(|(_, k)| k), &r)
                    );
                }
            }
            (2, Raw::Bytes(b)) => entities.push(parse(b).expect("entita není zpráva")),
            _ => println!("neznámé pole FeedMessage #{tag}"),
        }
    }
    println!("entit: {}", entities.len());

    let mut stats: BTreeMap<String, Stat> = BTreeMap::new();
    // Druh vozu → (počet entit, pole → počet entit s polem).
    let mut kinds: BTreeMap<String, (usize, BTreeMap<String, usize>)> = BTreeMap::new();
    let mut dumps = Vec::new();
    let mut ages = Vec::new();
    for e in &entities {
        let mut seen = BTreeSet::new();
        let mut lines = Vec::new();
        walk("entity", "", e, 1, &mut seen, &mut stats, &mut lines);
        let k = kinds.entry(vehicle_kind(&lines)).or_default();
        k.0 += 1;
        for p in &seen {
            *k.1.entry(p.clone()).or_default() += 1;
        }
        // Stáří polohy = header.timestamp − vehicle.timestamp.
        if let (Some(h), Some(Raw::Bytes(vb))) =
            (header_ts, e.iter().find(|(t, _)| *t == 4).map(|(_, r)| r))
            && let Some((_, Raw::Varint(ts))) = parse(vb)
                .unwrap_or_default()
                .into_iter()
                .find(|(t, _)| *t == 5)
        {
            ages.push(h - ts as i64);
        }
        dumps.push(lines);
    }

    let n = entities.len().max(1);
    println!("\n== pokrytí polí (entity s polem / všechny entity) ==");
    for (p, st) in &stats {
        let distinct = st.values.len();
        let pct = st.entities as f64 * 100.0 / n as f64;
        println!(
            "\n{p}: {}/{} ({pct:.1} %), různých hodnot: {distinct}",
            st.entities,
            entities.len()
        );
        if st.values.contains_key("<zpráva>") {
            continue;
        }
        if distinct <= 12 {
            let mut v: Vec<_> = st.values.iter().collect();
            v.sort_by(|a, b| b.1.cmp(a.1));
            for (val, c) in v {
                println!("    {c:>6}× {}", truncate(val, 200));
            }
        } else {
            // Ukázky: první, prostřední, poslední (seřazené) a nejčastější.
            let keys: Vec<_> = st.values.keys().collect();
            let top = st.values.iter().max_by_key(|(_, c)| **c).unwrap();
            println!(
                "    ukázky: {} | {} | {}",
                truncate(keys[0], 120),
                truncate(keys[keys.len() / 2], 120),
                truncate(keys[keys.len() - 1], 120)
            );
            println!("    nejčastější: {}× {}", top.1, truncate(top.0, 200));
        }
    }

    if !ages.is_empty() {
        ages.sort_unstable();
        println!(
            "\n== stáří polohy (header.timestamp − vehicle.timestamp, s) ==\n  min {}  p50 {}  p90 {}  p99 {}  max {}",
            ages[0],
            percentile(&ages, 0.5),
            percentile(&ages, 0.9),
            percentile(&ages, 0.99),
            ages[ages.len() - 1]
        );
    }

    println!(
        "\n== pokrytí podle druhu vozu (service-0 tramvaj, -3 autobus, -11 trolejbus …; GTFS route_type) =="
    );
    let leaves: Vec<_> = stats
        .iter()
        .filter(|(_, st)| !st.values.contains_key("<zpráva>"))
        .map(|(p, _)| p.clone())
        .collect();
    for (kind, (count, fields)) in &kinds {
        println!("\n{kind}: {count} vozů");
        for p in &leaves {
            let c = fields.get(p).copied().unwrap_or(0);
            if c < *count {
                println!("    {p:<40} {:>5.1} %", c as f64 * 100.0 / *count as f64);
            }
        }
    }

    // Ukázky: z každého druhu vozu (prefix vehicle.vehicle.id) jedna entita, celkem nejvýš `samples`.
    println!("\n== ukázkové entity ==");
    let mut by_kind: HashMap<String, usize> = HashMap::new();
    let mut shown = 0;
    for lines in &dumps {
        if shown >= samples {
            break;
        }
        let c = by_kind.entry(vehicle_kind(lines)).or_default();
        if *c > 0 {
            continue;
        }
        *c += 1;
        shown += 1;
        println!("entity {{");
        for l in lines {
            println!("{l}");
        }
        println!("}}");
    }
}

/// Druh vozu z `vehicle.vehicle.id`: `service-3-5050` → `service-3`, `metro-C-25-25` → `metro`.
fn vehicle_kind(lines: &[String]) -> String {
    let id = lines
        .iter()
        .find_map(|l| l.strip_prefix("      id: \""))
        .unwrap_or("?");
    let mut parts = id.split('-');
    match parts.next() {
        Some("service") => format!("service-{}", parts.next().unwrap_or("?")),
        Some(p) => p.to_string(),
        None => "?".into(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}
