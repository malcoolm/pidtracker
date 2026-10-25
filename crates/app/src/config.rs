//! Konfigurace a verze vložené při buildu.

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const BUILD_ID: &str = env!("PIDTRACKER_BUILD_ID");
pub const BUILT_AT: &str = env!("PIDTRACKER_BUILT_AT");
/// Client ID z buildu; prázdná hodnota (nenastavená proměnná v CI) se bere jako chybějící.
pub const GOOGLE_CLIENT_ID: Option<&str> = match option_env!("PIDTRACKER_GOOGLE_CLIENT_ID") {
    Some(id) if !id.is_empty() => Some(id),
    _ => None,
};

/// Pevně nastavená Google tabulka s jízdami (sdílená mezi uživateli).
pub const SHEET_ID: &str = "13_ofodGGIDmTKrkgJjoTp5OVAdcJkMLr2Lt411UNhxg";
