//! Čas v Europe/Prague a převod GTFS časů.

use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;

pub const TZ: Tz = chrono_tz::Europe::Prague;

/// GTFS čas se počítá od „poledne − 12 h“ servisního dne (správně i v den změny času).
pub fn gtfs_instant(service_date: NaiveDate, secs: u32) -> DateTime<Tz> {
    let noon = TZ
        .from_local_datetime(&service_date.and_hms_opt(12, 0, 0).unwrap())
        .single()
        .expect("poledne je vždy jednoznačné");
    noon - Duration::hours(12) + Duration::seconds(secs as i64)
}

/// Okamžik z `Date.now()` (milisekundy od epochy) v pražském čase.
pub fn now_prague(epoch_ms: f64) -> DateTime<Tz> {
    Utc.timestamp_millis_opt(epoch_ms as i64).unwrap().with_timezone(&TZ)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone, Utc};

    #[test]
    fn gtfs_time_on_dst_end_day() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        assert_eq!(
            gtfs_instant(d, 3600).with_timezone(&Utc),
            Utc.with_ymd_and_hms(2026, 10, 25, 0, 0, 0).unwrap()
        );
        assert_eq!(
            gtfs_instant(d, 12 * 3600).with_timezone(&Utc),
            Utc.with_ymd_and_hms(2026, 10, 25, 11, 0, 0).unwrap()
        );
    }

    #[test]
    fn gtfs_time_past_midnight() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();
        let t = gtfs_instant(d, 24 * 3600 + 10 * 60);
        assert_eq!(
            t.naive_local(),
            NaiveDate::from_ymd_opt(2026, 11, 1).unwrap().and_hms_opt(0, 10, 0).unwrap()
        );
    }

    #[test]
    fn now_prague_from_epoch() {
        // 2026-10-26T07:00:00Z = 08:00 CET
        let t = now_prague(1_792_998_000_000.0);
        assert_eq!(t.format("%Y-%m-%d %H:%M").to_string(), "2026-10-26 08:00");
    }
}
