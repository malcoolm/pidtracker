//! Pěší přestupy: konstanty a převody vzdálenosti na čas.

/// Rychlost chůze v m/s.
pub const WALK_SPEED_MPS: f64 = 1.0;
/// Koeficient okliky oproti vzdušné čáře.
pub const DETOUR: f64 = 1.3;
/// Největší vzdušná vzdálenost stanovišť propojených předpočítaným footpath.
pub const FOOTPATH_MAX_M: f64 = 400.0;
/// Největší vzdušná vzdálenost od/k zadanému bodu (přístup, odchod).
pub const ACCESS_MAX_M: f64 = 1500.0;
/// Minimální čas přestupu v sekundách.
pub const MIN_TRANSFER_S: u32 = 120;
/// Minimální čas přestupu, pokud je kterákoli strana metro.
pub const METRO_MIN_TRANSFER_S: u32 = 240;
/// Příplatek za sestup/výstup z metra při přístupu pěšky.
pub const METRO_ACCESS_EXTRA_S: u32 = 120;
/// Nejvyšší počet přestupů ve hledaném spojení.
pub const MAX_TRANSFERS: u8 = 4;

/// Doba chůze pro vzdušnou vzdálenost (s okliky), zaokrouhleno nahoru.
pub fn walk_seconds(distance_m: f64) -> u32 {
    (distance_m * DETOUR / WALK_SPEED_MPS).ceil() as u32
}

/// Čas přestupu: chůze, nejméně však minimum (vyšší, je-li některá strana metro).
pub fn transfer_seconds(walk_s: u32, from_metro: bool, to_metro: bool) -> u32 {
    let min = if from_metro || to_metro {
        METRO_MIN_TRANSFER_S
    } else {
        MIN_TRANSFER_S
    };
    walk_s.max(min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_seconds_applies_detour() {
        assert_eq!(walk_seconds(100.0), 130);
        assert_eq!(walk_seconds(0.0), 0);
    }

    #[test]
    fn transfer_seconds_applies_minimum() {
        assert_eq!(transfer_seconds(30, false, false), 120);
        assert_eq!(transfer_seconds(30, true, false), 240);
        assert_eq!(transfer_seconds(300, false, true), 300);
    }
}
