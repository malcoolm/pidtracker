//! Politika tichého obnovení Google tokenu (čistá logika, bez prohlížeče).

/// Obnovu zkusíme, když do vypršení zbývá méně než 5 minut.
pub const REFRESH_BEFORE_MS: f64 = 300_000.0;
/// Nejvýš jeden pokus o obnovu za 120 s.
pub const MIN_ATTEMPT_GAP_MS: f64 = 120_000.0;

#[derive(Clone, Copy, Debug)]
pub struct RefreshInput {
    pub now_ms: f64,
    /// Vypršení uloženého tokenu (i prošlého); `None`, když token chybí.
    pub expires_at_ms: Option<f64>,
    pub signed_in_before: bool,
    pub online: bool,
    pub in_flight: bool,
    pub last_attempt_ms: Option<f64>,
}

pub fn should_refresh(i: &RefreshInput) -> bool {
    if !(i.signed_in_before && i.online) {
        return false;
    }
    let recent = i.last_attempt_ms.is_some_and(|t| i.now_ms - t < MIN_ATTEMPT_GAP_MS);
    // Zaseknutý pokus (popup bez callbacku) starší než odstup se za probíhající nepočítá.
    if recent || (i.in_flight && i.last_attempt_ms.is_none()) {
        return false;
    }
    i.expires_at_ms.is_none_or(|e| e - i.now_ms < REFRESH_BEFORE_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> RefreshInput {
        RefreshInput {
            now_ms: 1_000_000.0,
            expires_at_ms: None,
            signed_in_before: true,
            online: true,
            in_flight: false,
            last_attempt_ms: None,
        }
    }

    #[test]
    fn refresh_when_expired_or_near() {
        assert!(should_refresh(&base()));
        let near = RefreshInput { expires_at_ms: Some(1_000_000.0 + 240_000.0), ..base() };
        assert!(should_refresh(&near));
        let far = RefreshInput { expires_at_ms: Some(1_000_000.0 + 600_000.0), ..base() };
        assert!(!should_refresh(&far));
        let expired = RefreshInput { expires_at_ms: Some(900_000.0), ..base() };
        assert!(should_refresh(&expired));
    }

    #[test]
    fn refresh_throttled_and_offline() {
        let recent = RefreshInput { last_attempt_ms: Some(1_000_000.0 - 60_000.0), ..base() };
        assert!(!should_refresh(&recent));
        let older = RefreshInput { last_attempt_ms: Some(1_000_000.0 - 130_000.0), ..base() };
        assert!(should_refresh(&older));
        assert!(!should_refresh(&RefreshInput { online: false, ..base() }));
        assert!(!should_refresh(&RefreshInput { in_flight: true, ..base() }));
        let stuck = RefreshInput { in_flight: true, last_attempt_ms: Some(1_000_000.0 - 130_000.0), ..base() };
        assert!(should_refresh(&stuck));
        let busy = RefreshInput { in_flight: true, last_attempt_ms: Some(1_000_000.0 - 30_000.0), ..base() };
        assert!(!should_refresh(&busy));
        assert!(!should_refresh(&RefreshInput { signed_in_before: false, ..base() }));
    }
}
