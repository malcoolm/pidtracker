//! Sezónní vzhled aplikace.

use chrono::{Datelike, Days, NaiveDate, Weekday};

/// Sezónní vzhled aplikace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Season {
    Halloween,
    Vanoce,
}

/// Sezóna pro dané datum, mimo sezóny `None`.
pub fn season(date: NaiveDate) -> Option<Season> {
    if is_halloween(date) {
        Some(Season::Halloween)
    } else if is_vanoce(date) {
        Some(Season::Vanoce)
    } else {
        None
    }
}

/// Halloweenský vzhled: 24. 10. – 4. 11. včetně, každý rok.
pub fn is_halloween(date: NaiveDate) -> bool {
    matches!((date.month(), date.day()), (10, 24..) | (11, ..=4))
}

/// Sobota před 1. adventní nedělí (ta připadá na první neděli od 27. 11.).
pub fn advent_saturday(year: i32) -> NaiveDate {
    let nov27 = NaiveDate::from_ymd_opt(year, 11, 27).expect("27. 11. existuje každý rok");
    let to_sunday = (7 - nov27.weekday().days_since(Weekday::Sun)) % 7;
    nov27 + Days::new(u64::from(to_sunday)) - Days::new(1)
}

/// Vánoční vzhled: od soboty před 1. adventní nedělí do 28. 12. včetně.
pub fn is_vanoce(date: NaiveDate) -> bool {
    date >= advent_saturday(date.year()) && (date.month(), date.day()) <= (12, 28)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halloween_window() {
        let d = |m, dd| NaiveDate::from_ymd_opt(2026, m, dd).unwrap();
        assert!(!is_halloween(d(10, 23)));
        assert!(is_halloween(d(10, 24)));
        assert!(is_halloween(d(10, 31)));
        assert!(is_halloween(d(11, 4)));
        assert!(!is_halloween(d(11, 5)));
        assert!(!is_halloween(d(6, 1)));
    }

    #[test]
    fn advent_saturday_by_year() {
        let d = |y, m, dd| NaiveDate::from_ymd_opt(y, m, dd).unwrap();
        assert_eq!(advent_saturday(2026), d(2026, 11, 28));
        assert_eq!(advent_saturday(2027), d(2027, 11, 27));
        assert_eq!(advent_saturday(2028), d(2028, 12, 2));
        assert_eq!(advent_saturday(2029), d(2029, 12, 1));
        assert_eq!(advent_saturday(2030), d(2030, 11, 30));
    }

    #[test]
    fn vanoce_window() {
        let d = |y, m, dd| NaiveDate::from_ymd_opt(y, m, dd).unwrap();
        assert!(!is_vanoce(d(2026, 11, 27)));
        assert!(is_vanoce(d(2026, 11, 28)));
        assert!(is_vanoce(d(2026, 12, 24)));
        assert!(is_vanoce(d(2026, 12, 28)));
        assert!(!is_vanoce(d(2026, 12, 29)));
        assert!(!is_vanoce(d(2027, 1, 1)));
        assert!(!is_vanoce(d(2028, 12, 1)));
        assert!(is_vanoce(d(2028, 12, 2)));
    }

    #[test]
    fn season_picks_period() {
        let d = |m, dd| NaiveDate::from_ymd_opt(2026, m, dd).unwrap();
        assert_eq!(season(d(10, 31)), Some(Season::Halloween));
        assert_eq!(season(d(12, 1)), Some(Season::Vanoce));
        assert_eq!(season(d(11, 15)), None);
        assert_eq!(season(d(6, 1)), None);
    }
}
