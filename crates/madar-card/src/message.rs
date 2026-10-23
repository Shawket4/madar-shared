//! What a dashboard message may say, and how often a shop may send one.
//!
//! A message rides on the card as one field (`wallet::notices`): Apple prints
//! it on the card and raises a notification with it; Google posts it as a
//! message. One line, short, plain: a field is not a page, and a link in it is
//! neither tappable on Apple nor something a shop's card should carry.

use serde::{Deserialize, Serialize};

use crate::validate::Problem;

/// Characters per language.
pub const LINE_MAX: usize = 100;
/// Messages a shop may send in any 24 hours.
pub const DAILY_LIMIT: u32 = 3;
/// A member gets at most one message in this many hours; later ones skip them.
pub const MEMBER_GAP_HOURS: u32 = 20;
/// A scheduled message goes at least this far ahead…
pub const SCHEDULE_MIN_MINUTES: u32 = 5;
/// …and at most this far.
pub const SCHEDULE_MAX_DAYS: u32 = 30;

/// The message, in either or both languages.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Message {
    #[serde(default)]
    pub en: String,
    #[serde(default)]
    pub ar: String,
}

impl Message {
    /// The line a member reading `arabic` gets: their language, else the other.
    pub fn line_for(&self, arabic: bool) -> &str {
        let (en, ar) = (self.en.trim(), self.ar.trim());
        match (arabic, en.is_empty(), ar.is_empty()) {
            (true, _, false) | (false, true, false) => ar,
            _ => en,
        }
    }
}

fn has_link(s: &str) -> bool {
    let l = s.to_lowercase();
    l.contains("http://")
        || l.contains("https://")
        || l.contains("www.")
        || l.split_whitespace().any(|w| {
            let w = w.trim_matches(|c: char| !c.is_alphanumeric());
            // a bare domain: letters, a dot, a known-looking ending
            w.contains('.')
                && [
                    ".com", ".net", ".org", ".eg", ".io", ".co", ".ly", ".me", ".app", ".link",
                ]
                .iter()
                .any(|t| w.ends_with(t))
        })
}

/// The problems with `m`; empty when it may be sent.
pub fn validate_message(m: &Message) -> Vec<Problem> {
    let mut out = Vec::new();
    if m.en.trim().is_empty() && m.ar.trim().is_empty() {
        out.push(Problem::error("message_empty", "message"));
    }
    for (path, s) in [("message.en", &m.en), ("message.ar", &m.ar)] {
        if s.trim().chars().count() > LINE_MAX {
            out.push(Problem::error("message_long", path));
        }
        if s.chars().any(char::is_control) {
            out.push(Problem::error("message_one_line", path));
        }
        if has_link(s) {
            out.push(Problem::error("message_link", path));
        }
    }
    out
}

/// Whether a shop that has sent `sent_last_24h` messages may send another.
pub fn may_send(sent_last_24h: u32) -> Result<(), Problem> {
    if sent_last_24h >= DAILY_LIMIT {
        Err(Problem::error("daily_limit", "message"))
    } else {
        Ok(())
    }
}

/// Whether a message `minutes_ahead` from now is a valid schedule.
pub fn schedule_ok(minutes_ahead: i64) -> bool {
    minutes_ahead >= i64::from(SCHEDULE_MIN_MINUTES)
        && minutes_ahead <= i64::from(SCHEDULE_MAX_DAYS) * 24 * 60
}

/// The limits, for the composer to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Limits {
    pub line_max: u32,
    pub daily_limit: u32,
    pub member_gap_hours: u32,
    pub schedule_min_minutes: u32,
    pub schedule_max_days: u32,
}

pub const LIMITS: Limits = Limits {
    // LINE_MAX is 100: exact.
    #[allow(clippy::cast_possible_truncation)]
    line_max: LINE_MAX as u32,
    daily_limit: DAILY_LIMIT,
    member_gap_hours: MEMBER_GAP_HOURS,
    schedule_min_minutes: SCHEDULE_MIN_MINUTES,
    schedule_max_days: SCHEDULE_MAX_DAYS,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn m(en: &str, ar: &str) -> Message {
        Message {
            en: en.into(),
            ar: ar.into(),
        }
    }

    #[test]
    fn messages() {
        let codes = |x: Message| {
            validate_message(&x)
                .into_iter()
                .map(|p| p.code)
                .collect::<Vec<_>>()
        };
        assert!(codes(m("Double stamps on Friday!", "")).is_empty());
        assert_eq!(codes(m("", " ")), ["message_empty"]);
        assert_eq!(codes(m(&"a".repeat(101), "")), ["message_long"]);
        assert_eq!(codes(m("see drops.com now", "")), ["message_link"]);
        assert_eq!(codes(m("a\nb", "")), ["message_one_line"]);
        assert!(
            codes(m("3.5 EGP off", "")).is_empty(),
            "a decimal is not a link"
        );
        assert_eq!(m("Hi", "أهلا").line_for(true), "أهلا");
        assert_eq!(m("Hi", "").line_for(true), "Hi");
        assert_eq!(m("", "أهلا").line_for(false), "أهلا");
        assert!(may_send(2).is_ok());
        assert!(may_send(3).is_err());
        assert!(
            schedule_ok(5)
                && !schedule_ok(4)
                && schedule_ok(30 * 24 * 60)
                && !schedule_ok(30 * 24 * 60 + 1)
        );
    }
}
