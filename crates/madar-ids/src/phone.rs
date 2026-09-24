//! The one canonical phone form: E.164 digits without the `+`
//! (`201001234567`). Customers, loyalty, delivery and bookings all key on it.
//!
//! Moved from MadarRust `src/phone.rs` (madar-core `phone.rs` was the same
//! rule, 2 000 000 fuzzed inputs apart by nothing). The same rule lives in SQL
//! (`phone_canonical`) and in the dashboard (`src/lib/phone.ts`); all run
//! `vectors/phone_vectors.json`. The rule, in order:
//!
//!   1. raw input longer than 32 characters is invalid;
//!   2. Arabic-Indic (U+0660–0669) and Extended Arabic-Indic (U+06F0–06F9)
//!      digits become ASCII;
//!   3. only ASCII digits are kept;
//!   4. a leading `00` is stripped; else a leading `20` is kept; else a leading
//!      `0` becomes `20`; else exactly ten digits starting with `1` get `20`
//!      prefixed; else unchanged;
//!   5. a result outside 10–15 digits is invalid;
//!   6. Egyptian mobile guard: a result starting with `2010`, `2011`, `2012`
//!      or `2015` must be exactly 12 digits, else invalid. A truncated or
//!      over-long mobile is the commonest typo; landlines such as
//!      `2013xxxxxxx` are untouched.

/// Longest raw input considered at all.
pub const MAX_PHONE_RAW_LEN: usize = 32;

/// An Egyptian mobile in canonical form: `20` + `1x` + eight digits.
pub const EG_MOBILE_LEN: usize = 12;

fn ascii_digit(c: char) -> Option<char> {
    match c {
        '0'..='9' => Some(c),
        '\u{0660}'..='\u{0669}' => char::from_digit(c as u32 - 0x0660, 10),
        '\u{06F0}'..='\u{06F9}' => char::from_digit(c as u32 - 0x06F0, 10),
        _ => None,
    }
}

/// ASCII digits of `raw`, with Arabic-Indic digits mapped. No validation —
/// what a search box holds while a number is still being typed.
pub fn digits(raw: &str) -> String {
    raw.chars().filter_map(ascii_digit).collect()
}

/// The canonical form, or `None` when `raw` is not a phone number.
pub fn canonical(raw: &str) -> Option<String> {
    if raw.chars().count() > MAX_PHONE_RAW_LEN {
        return None;
    }
    let d = digits(raw);
    let n = if let Some(rest) = d.strip_prefix("00") {
        rest.to_string()
    } else if d.starts_with("20") {
        d
    } else if let Some(rest) = d.strip_prefix('0') {
        format!("20{rest}")
    } else if d.len() == 10 && d.starts_with('1') {
        format!("20{d}")
    } else {
        d
    };
    if n.len() < 10 || n.len() > 15 {
        return None;
    }
    let mobile = ["2010", "2011", "2012", "2015"]
        .iter()
        .any(|p| n.starts_with(p));
    if mobile && n.len() != EG_MOBILE_LEN {
        return None;
    }
    Some(n)
}

/// What a search box's digits should be matched against canonical keys with:
/// a partial number cannot be canonicalised, but its local prefix can be
/// dropped (`0100…` is stored as `20100…`, so `100…` is what both contain).
pub fn search_digits(raw: &str) -> Option<String> {
    let d = digits(raw);
    let d = d
        .strip_prefix("00")
        .or_else(|| d.strip_prefix('0'))
        .unwrap_or(&d);
    (d.len() >= 3).then(|| d.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vectors() -> (Vec<(String, String)>, Vec<String>) {
        let v: serde_json::Value = serde_json::from_str(crate::vectors::PHONE).unwrap();
        let valid = v["valid"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p[0].as_str().unwrap().to_string(),
                    p[1].as_str().unwrap().to_string(),
                )
            })
            .collect();
        let invalid = v["invalid"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect();
        (valid, invalid)
    }

    // Moved from MadarRust `phone.rs` (`the_shared_vectors_hold_in_rust`) and
    // madar-core `phone.rs` (`every_valid_vector_canonicalises`,
    // `every_invalid_vector_is_refused`): one test, the stricter counts.
    #[test]
    fn the_shared_vectors_hold() {
        let (valid, invalid) = vectors();
        assert!(valid.len() >= 20 && invalid.len() >= 8);
        for (raw, want) in valid {
            assert_eq!(canonical(&raw).as_deref(), Some(want.as_str()), "{raw:?}");
            // Canonical is a fixed point: a stored key re-canonicalises to itself.
            assert_eq!(canonical(&want).as_deref(), Some(want.as_str()), "{want:?}");
        }
        for raw in invalid {
            assert_eq!(canonical(&raw), None, "{raw:?}");
        }
    }

    // Moved from MadarRust `phone.rs`.
    #[test]
    fn a_search_drops_the_local_prefix() {
        assert_eq!(search_digits("0100 123").as_deref(), Some("100123"));
        assert_eq!(search_digits("+20100").as_deref(), Some("20100"));
        assert_eq!(search_digits("٠١٠٠").as_deref(), Some("100"));
        assert_eq!(search_digits("01"), None);
        assert_eq!(search_digits("Ali"), None);
    }

    // Moved from madar-core `phone.rs`.
    #[test]
    fn digits_fold_arabic_numerals_without_validating() {
        assert_eq!(digits("٠١٠-۰۱"), "01001");
        assert_eq!(digits("abc"), "");
    }
}
