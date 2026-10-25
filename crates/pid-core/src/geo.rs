//! Geografické výpočty.

const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// Vzdálenost dvou bodů po povrchu Země v metrech.
pub fn haversine_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = (lat2 - lat1).to_radians();
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_M * a.sqrt().asin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haversine_known_distance() {
        let d = haversine_m(50.079, 14.430, 50.0755, 14.4375);
        assert!((d - 662.0).abs() < 5.0, "{d}");
    }

    #[test]
    fn haversine_zero() {
        assert_eq!(haversine_m(50.0, 14.0, 50.0, 14.0), 0.0);
    }
}
