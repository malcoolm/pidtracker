//! Normalizace textu pro vyhledávání.

/// Malá písmena bez české (a běžné) diakritiky.
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'ä' | 'à' | 'â' => 'a',
            'č' | 'ç' => 'c',
            'ď' => 'd',
            'é' | 'ě' | 'ë' | 'è' | 'ê' => 'e',
            'í' | 'ï' | 'î' => 'i',
            'ľ' | 'ĺ' => 'l',
            'ň' => 'n',
            'ó' | 'ö' | 'ô' => 'o',
            'ř' | 'ŕ' => 'r',
            'š' => 's',
            'ť' => 't',
            'ú' | 'ů' | 'ü' => 'u',
            'ý' => 'y',
            'ž' => 'z',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fold_removes_czech_diacritics() {
        assert_eq!(fold("Náměstí Míru Ůž"), "namesti miru uz");
        assert_eq!(fold("ŘÍČANY Ďábel Ťuk Ňadra Óda Ýpsilon Ě"), "ricany dabel tuk nadra oda ypsilon e");
    }
}
