//! Sezónní vzhled aplikace.

use chrono::{Datelike, NaiveDate};

/// Halloweenský vzhled: 24. 10. – 4. 11. včetně, každý rok.
pub fn is_halloween(date: NaiveDate) -> bool {
    matches!((date.month(), date.day()), (10, 24..) | (11, ..=4))
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
}
