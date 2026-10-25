//! Řádky Google tabulky.

use std::collections::HashMap;

use chrono::{DateTime, FixedOffset};

use crate::ride::{Ride, RideStatus};

pub const RIDES_SHEET: &str = "rides";
pub const PEOPLE_SHEET: &str = "people";
pub const PEOPLE_HEADERS: [&str; 2] = ["email", "nickname"];

/// Sloupce listu `rides`. Nové sloupce se smí přidávat jen na konec.
pub const HEADERS: [&str; 24] = [
    "id",
    "person",
    "board_stop_id",
    "board_stop_name",
    "route_id",
    "route_short_name",
    "route_type",
    "headsign",
    "trip_id",
    "pattern_id",
    "board_time",
    "alight_stop_id",
    "alight_stop_name",
    "alight_time",
    "board_lat",
    "board_lon",
    "alight_lat",
    "alight_lon",
    "status",
    "deleted",
    "created_at",
    "updated_at",
    "app_version",
    "data_version",
];
pub const LAST_COL: &str = "X";

/// Písmeno sloupce tabulky (1 → A, 27 → AA).
pub fn column_letter(mut n: usize) -> String {
    let mut out = Vec::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        out.push(b'A' + rem as u8);
        n = (n - 1) / 26;
    }
    out.reverse();
    String::from_utf8(out).unwrap()
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum RowError {
    #[error("chybí povinný sloupec {0}")]
    Missing(&'static str),
    #[error("neplatná hodnota ve sloupci {0}")]
    Invalid(&'static str),
}

pub struct ParsedRows {
    /// (číslo řádku v tabulce od 1, jízda)
    pub rides: Vec<(u32, Ride)>,
    pub skipped: u32,
}

fn opt_str(v: &Option<String>) -> String {
    v.clone().unwrap_or_default()
}

fn opt_num<T: ToString>(v: Option<T>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

fn opt_time(v: &Option<DateTime<FixedOffset>>) -> String {
    v.map(|t| t.to_rfc3339()).unwrap_or_default()
}

pub fn to_row(r: &Ride) -> Vec<String> {
    vec![
        r.id.clone(),
        r.person.clone(),
        r.board_stop_id.clone(),
        r.board_stop_name.clone(),
        r.route_id.clone(),
        r.route_short_name.clone(),
        opt_num(r.route_type),
        r.headsign.clone(),
        r.trip_id.clone(),
        opt_num(r.pattern_id),
        r.board_time.to_rfc3339(),
        opt_str(&r.alight_stop_id),
        opt_str(&r.alight_stop_name),
        opt_time(&r.alight_time),
        opt_num(r.board_lat),
        opt_num(r.board_lon),
        opt_num(r.alight_lat),
        opt_num(r.alight_lon),
        r.status.as_str().to_string(),
        if r.deleted { "TRUE" } else { "FALSE" }.to_string(),
        r.created_at.to_rfc3339(),
        r.updated_at.to_rfc3339(),
        r.app_version.clone(),
        r.data_version.clone(),
    ]
}

struct Cells<'a> {
    map: HashMap<&'a str, &'a str>,
}

impl<'a> Cells<'a> {
    fn new(headers: &'a [String], row: &'a [String]) -> Self {
        let map = headers
            .iter()
            .zip(row.iter())
            .map(|(h, v)| (h.trim(), v.trim()))
            .collect();
        Cells { map }
    }

    fn get(&self, key: &'static str) -> &'a str {
        self.map.get(key).copied().unwrap_or("")
    }

    fn opt(&self, key: &'static str) -> Option<String> {
        let v = self.get(key);
        (!v.is_empty()).then(|| v.to_string())
    }

    fn required(&self, key: &'static str) -> Result<String, RowError> {
        self.opt(key).ok_or(RowError::Missing(key))
    }

    fn num<T: std::str::FromStr>(&self, key: &'static str) -> Result<Option<T>, RowError> {
        match self.get(key) {
            "" => Ok(None),
            v => v.parse().map(Some).map_err(|_| RowError::Invalid(key)),
        }
    }

    fn time(&self, key: &'static str) -> Result<Option<DateTime<FixedOffset>>, RowError> {
        match self.get(key) {
            "" => Ok(None),
            v => DateTime::parse_from_rfc3339(v).map(Some).map_err(|_| RowError::Invalid(key)),
        }
    }

    fn required_time(&self, key: &'static str) -> Result<DateTime<FixedOffset>, RowError> {
        self.time(key)?.ok_or(RowError::Missing(key))
    }
}

pub fn from_row(headers: &[String], row: &[String]) -> Result<Ride, RowError> {
    let c = Cells::new(headers, row);
    let board_time = c.required_time("board_time")?;
    Ok(Ride {
        id: c.required("id")?,
        person: c.required("person")?,
        board_stop_id: c.get("board_stop_id").to_string(),
        board_stop_name: c.get("board_stop_name").to_string(),
        route_id: c.get("route_id").to_string(),
        route_short_name: c.get("route_short_name").to_string(),
        route_type: c.num("route_type")?,
        headsign: c.get("headsign").to_string(),
        trip_id: c.get("trip_id").to_string(),
        pattern_id: c.num("pattern_id")?,
        board_time,
        alight_stop_id: c.opt("alight_stop_id"),
        alight_stop_name: c.opt("alight_stop_name"),
        alight_time: c.time("alight_time")?,
        board_lat: c.num("board_lat")?,
        board_lon: c.num("board_lon")?,
        alight_lat: c.num("alight_lat")?,
        alight_lon: c.num("alight_lon")?,
        status: RideStatus::parse(c.get("status")).ok_or(RowError::Invalid("status"))?,
        deleted: matches!(c.get("deleted").to_ascii_lowercase().as_str(), "true" | "1"),
        created_at: c.time("created_at")?.unwrap_or(board_time),
        updated_at: c.time("updated_at")?.unwrap_or(board_time),
        app_version: c.get("app_version").to_string(),
        data_version: c.get("data_version").to_string(),
    })
}

/// První řádek jsou hlavičky; nečitelné řádky se přeskočí a spočítají.
pub fn parse_rows(values: &[Vec<String>]) -> ParsedRows {
    let mut out = ParsedRows { rides: Vec::new(), skipped: 0 };
    let Some((headers, rows)) = values.split_first() else { return out };
    for (i, row) in rows.iter().enumerate() {
        if row.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        match from_row(headers, row) {
            Ok(ride) => out.rides.push((i as u32 + 2, ride)),
            Err(_) => out.skipped += 1,
        }
    }
    out
}

/// Řádky listu `people`: (číslo řádku, e-mail, přezdívka).
pub fn people_rows(values: &[Vec<String>]) -> Vec<(u32, String, String)> {
    values
        .iter()
        .enumerate()
        .skip(1)
        .filter_map(|(i, row)| {
            let email = row.first()?.trim();
            (!email.is_empty()).then(|| {
                let nick = row.get(1).map(|s| s.trim().to_string()).unwrap_or_default();
                (i as u32 + 1, email.to_string(), nick)
            })
        })
        .collect()
}

/// Vytáhne ID tabulky z URL `…/spreadsheets/d/<id>/…` nebo přijme samotné ID.
pub fn parse_sheet_id(input: &str) -> Option<String> {
    let input = input.trim();
    let candidate = match input.split_once("/spreadsheets/d/") {
        Some((_, rest)) => rest.split(['/', '?', '#']).next().unwrap_or(""),
        None => input,
    };
    let valid = candidate.len() >= 25
        && candidate.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    valid.then(|| candidate.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sheet_id_variants() {
        let id = "1AbC-dEf_123456789012345678901234567890";
        assert_eq!(
            parse_sheet_id(&format!("https://docs.google.com/spreadsheets/d/{id}/edit#gid=0")).as_deref(),
            Some(id)
        );
        assert_eq!(parse_sheet_id(id).as_deref(), Some(id));
        assert_eq!(parse_sheet_id("  https://example.com/x "), None);
        assert_eq!(parse_sheet_id(""), None);
    }
}

#[cfg(test)]
mod row_tests {
    use super::*;
    use crate::ride::tests::sample_ride;

    fn header_row() -> Vec<String> {
        HEADERS.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn row_roundtrip() {
        let r = sample_ride();
        assert_eq!(from_row(&header_row(), &to_row(&r)).unwrap(), r);
    }

    #[test]
    fn headers_len_matches_last_col() {
        assert_eq!(HEADERS.len(), 24);
        assert_eq!(LAST_COL, "X");
    }

    #[test]
    fn deleted_and_optionals_serialization() {
        let mut r = sample_ride();
        r.deleted = true;
        r.alight_time = None;
        let row = to_row(&r);
        assert_eq!(row[19], "TRUE");
        assert_eq!(row[13], "");
    }

    #[test]
    fn parse_rows_skips_garbage() {
        let mut v = vec![header_row()];
        v.push(to_row(&sample_ride()));
        v.push(vec!["".into(), "x".into()]);
        v.push(vec!["abc".into(), "a@b.cz".into()]);
        let p = parse_rows(&v);
        assert_eq!(p.rides.len(), 1);
        assert_eq!(p.rides[0].0, 2);
        assert_eq!(p.skipped, 2);
    }

    #[test]
    fn parse_rows_by_header_name() {
        let r = sample_ride();
        let row = to_row(&r);
        let mut headers = header_row();
        headers.reverse();
        headers.push("poznamka".into());
        let mut reordered: Vec<String> = row.iter().rev().cloned().collect();
        reordered.push("něco".into());
        let p = parse_rows(&[headers, reordered]);
        assert_eq!(p.skipped, 0);
        assert_eq!(p.rides[0].1, r);
    }

    #[test]
    fn parse_rows_tolerates_short_rows() {
        // Sheets API vynechává prázdné buňky na konci řádku.
        let mut r = sample_ride();
        r.app_version = String::new();
        r.data_version = String::new();
        let mut row = to_row(&r);
        row.truncate(22);
        let p = parse_rows(&[header_row(), row]);
        assert_eq!(p.rides[0].1, r);
    }

    #[test]
    fn people_rows_parse() {
        let v = vec![
            vec!["email".into(), "nickname".into()],
            vec!["tata@gmail.com".into(), "Táta".into()],
            vec!["syn@gmail.com".into()],
        ];
        assert_eq!(
            people_rows(&v),
            vec![(2, "tata@gmail.com".into(), "Táta".into()), (3, "syn@gmail.com".into(), "".into())]
        );
    }
}

#[cfg(test)]
mod column_tests {
    use super::*;

    #[test]
    fn column_letters() {
        assert_eq!(column_letter(1), "A");
        assert_eq!(column_letter(24), "X");
        assert_eq!(column_letter(26), "Z");
        assert_eq!(column_letter(27), "AA");
        assert_eq!(column_letter(53), "BA");
    }
}
