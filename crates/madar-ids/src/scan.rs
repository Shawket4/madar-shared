//! What a string arriving from a scanner, a camera or a keyboard actually is.
//!
//! Capturing the bytes is platform I/O (a camera decoder, the OS text path for
//! a USB wedge scanner); deciding what a captured string MEANS is this rule, so
//! the till, the loyalty teller page and every input path agree. Many wedge
//! scanners type their payload and never send Enter, so a caller can hand every
//! keystroke to [`classify`] and fire the moment the buffer becomes a whole
//! token or a phone. Pinned by `vectors/scan_vectors.json`.

use serde::{Deserialize, Serialize};

use crate::{member, phone};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A complete member token, ready to look up.
    Token,
    /// A plausible phone number, ready to look up.
    Phone,
    /// Not yet either: keep collecting keystrokes.
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scan {
    pub kind: Kind,
    /// The cleaned value to send. Empty when `kind` is `partial`.
    pub value: String,
}

impl Scan {
    fn partial() -> Scan {
        Scan {
            kind: Kind::Partial,
            value: String::new(),
        }
    }
}

/// Classify a captured string. A phone leaves in the one canonical form
/// (`phone::canonical`), so `0100…`, `+20 100…` and `٠١٠٠…` find the same person.
#[must_use]
pub fn classify(raw: &str) -> Scan {
    let s = raw.trim();
    if member::is_member_token(s) {
        return Scan {
            kind: Kind::Token,
            value: s.to_string(),
        };
    }
    let only_phone_chars = !s.is_empty()
        && s.chars().all(|c| {
            c.is_ascii_digit()
                || matches!(c, '+' | ' ' | '-' | '(' | ')' | '\u{0660}'..='\u{0669}' | '\u{06F0}'..='\u{06F9}')
        });
    if only_phone_chars && (10..=15).contains(&phone::digits(s).len()) {
        if let Some(value) = phone::canonical(s) {
            return Scan {
                kind: Kind::Phone,
                value,
            };
        }
    }
    Scan::partial()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `[[input, kind, value], …]`
    #[test]
    fn scan_vectors() {
        let v: Vec<(String, Kind, String)> = serde_json::from_str(crate::vectors::SCAN).unwrap();
        assert!(!v.is_empty());
        for (input, kind, value) in v {
            assert_eq!(classify(&input), Scan { kind, value }, "{input:?}");
        }
    }
}
