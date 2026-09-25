//! Sale windows: when a combo or a deal may be sold (C4, C18).
//!
//! A window is an optional weekday set, an optional time range and an
//! optional date range, for every branch or one. No window at all means
//! always available. Dates are `YYYY-MM-DD`, times `HH:MM` or `HH:MM:SS`, both
//! on the branch's local wall clock, which the caller supplies ([`LocalNow`]):
//! this crate reads no clock and knows no time zone.
//!
//! - `[starts_at, ends_at)` is half-open; `ends_at < starts_at` crosses
//!   midnight.
//! - The weekday bit (bit0 = Sunday … bit6 = Saturday) and the
//!   `valid_from`/`valid_to` range (inclusive) are judged on the day the
//!   window STARTED: 01:00 on Saturday inside a Friday 22:00–02:00 window is
//!   Friday's.
//! - Input that does not parse never matches (and never panics).
//!
//! Pinned by `vectors/sale_window_vectors.json` (hand-authored).

use serde::{Deserialize, Serialize};

/// One window, as the `sale_windows` row and the feed carry it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Window {
    /// `None` = every branch.
    #[serde(default)]
    pub branch_id: Option<String>,
    /// bit0 = Sunday … bit6 = Saturday; 127 = every day.
    #[serde(default = "all_days")]
    pub weekdays: u8,
    #[serde(default)]
    pub starts_at: Option<String>,
    #[serde(default)]
    pub ends_at: Option<String>,
    /// The first day (inclusive) the window starts on.
    #[serde(default)]
    pub valid_from: Option<String>,
    /// The last day (inclusive) the window starts on.
    #[serde(default)]
    pub valid_to: Option<String>,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            branch_id: None,
            weekdays: all_days(),
            starts_at: None,
            ends_at: None,
            valid_from: None,
            valid_to: None,
        }
    }
}

fn all_days() -> u8 {
    127
}

/// The branch's local wall clock: `date` = `YYYY-MM-DD`, `time` =
/// `HH:MM[:SS]`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalNow {
    pub date: String,
    pub time: String,
}

impl LocalNow {
    pub fn new(date: impl Into<String>, time: impl Into<String>) -> Self {
        Self {
            date: date.into(),
            time: time.into(),
        }
    }
}

/// Days since 1970-01-01 of a valid `YYYY-MM-DD` (Hinnant's days_from_civil).
fn day_number(date: &str) -> Option<i64> {
    let mut it = date.trim().splitn(3, '-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

/// Seconds since midnight of `HH:MM` or `HH:MM:SS`.
fn seconds_of(time: &str) -> Option<u32> {
    let mut it = time.trim().split(':');
    let h: u32 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let s: u32 = match it.next() {
        Some(s) => s.parse().ok()?,
        None => 0,
    };
    if it.next().is_some() || h > 23 || m > 59 || s > 59 {
        return None;
    }
    Some(h * 3600 + m * 60 + s)
}

/// The weekday of a date: 0 = Sunday … 6 = Saturday. `None` when it does not
/// parse.
pub fn weekday_of(date: &str) -> Option<u8> {
    day_number(date).map(|n| (n + 4).rem_euclid(7) as u8)
}

/// Does `w` cover `now`?
pub fn matches(w: &Window, now: &LocalNow) -> bool {
    let (Some(today), Some(t)) = (day_number(&now.date), seconds_of(&now.time)) else {
        return false;
    };
    let start_day = match (w.starts_at.as_deref(), w.ends_at.as_deref()) {
        (None, None) => today,
        (Some(s), Some(e)) => {
            let (Some(s), Some(e)) = (seconds_of(s), seconds_of(e)) else {
                return false;
            };
            if s == e {
                return false;
            } else if s < e {
                if t < s || t >= e {
                    return false;
                }
                today
            } else if t >= s {
                today
            } else if t < e {
                today - 1
            } else {
                return false;
            }
        }
        _ => return false,
    };
    let weekday = (start_day + 4).rem_euclid(7) as u32;
    if w.weekdays & (1u8 << weekday) == 0 {
        return false;
    }
    let bound = |d: &Option<String>| -> Option<Option<i64>> {
        match d.as_deref() {
            None => Some(None),
            Some(s) => day_number(s).map(Some),
        }
    };
    let (Some(from), Some(to)) = (bound(&w.valid_from), bound(&w.valid_to)) else {
        return false;
    };
    from.is_none_or(|f| start_day >= f) && to.is_none_or(|t| start_day <= t)
}

/// Is the thing these windows belong to on sale at `branch_id` now?
///
/// The windows that apply are the all-branch ones and, when a branch is
/// given, that branch's. None apply → always on sale; otherwise any one that
/// matches.
pub fn open(windows: &[Window], branch_id: Option<&str>, now: &LocalNow) -> bool {
    let mut applicable = windows
        .iter()
        .filter(|w| {
            w.branch_id.is_none() || (branch_id.is_some() && w.branch_id.as_deref() == branch_id)
        })
        .peekable();
    if applicable.peek().is_none() {
        return true;
    }
    applicable.any(|w| matches(w, now))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Vectors {
        weekdays: Vec<WeekdayCase>,
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct WeekdayCase {
        date: String,
        weekday: Option<u8>,
    }

    #[derive(Deserialize)]
    struct Case {
        name: String,
        windows: Vec<Window>,
        branch_id: Option<String>,
        now: LocalNow,
        expected: bool,
    }

    #[test]
    fn sale_window_vectors() {
        let v: Vectors = serde_json::from_str(crate::vectors::SALE_WINDOW).unwrap();
        for c in &v.weekdays {
            assert_eq!(weekday_of(&c.date), c.weekday, "{}", c.date);
        }
        assert!(v.cases.len() >= 30);
        for c in &v.cases {
            assert_eq!(
                open(&c.windows, c.branch_id.as_deref(), &c.now),
                c.expected,
                "{}",
                c.name
            );
        }
    }

    #[test]
    fn every_day_of_a_leap_cycle_has_the_next_weekday() {
        let mut prev = weekday_of("2023-12-31").unwrap();
        let mut n = day_number("2024-01-01").unwrap();
        for y in 2024..=2028 {
            for m in 1..=12 {
                for d in 1..=days_in_month(y, m) {
                    let s = format!("{y:04}-{m:02}-{d:02}");
                    assert_eq!(day_number(&s), Some(n), "{s}");
                    let w = weekday_of(&s).unwrap();
                    assert_eq!(w, (prev + 1) % 7, "{s}");
                    prev = w;
                    n += 1;
                }
            }
        }
    }
}
