//! Day-bound vectors: the UTC bounds of branch-local days, DST days included.
//!
//! Before this crate the server stepped forward over a DST gap and the till
//! read the missing midnight as UTC, starting Cairo's spring-forward day two
//! hours late (madar-shared discovery X1). The till was aligned to the server
//! first; these vectors, generated from that aligned rule, now pin it: the
//! Cairo and Beirut gap days (midnight does not exist), their neighbours, the
//! autumn fall-back days, and ordinary days.
//!
//! Regenerate deliberately:
//! `MADAR_REGENERATE_TIME_VECTORS=1 cargo test -p madar-time day_bound_vectors`.

use std::path::PathBuf;

use chrono::{Datelike, Duration, NaiveDate, SecondsFormat};
use serde::{Deserialize, Serialize};

use crate::day_bounds;

/// The bytes of `vectors/day_bounds_vectors.json`.
pub const DAY_BOUNDS: &str = include_str!("../vectors/day_bounds_vectors.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DayBoundVector {
    pub tz: String,
    pub date: String,
    /// RFC 3339, UTC.
    pub start: String,
    pub end: String,
    /// `end - start` in hours: 23 on a spring-forward day, 25 on a fall-back one.
    pub hours: i64,
}

pub fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/day_bounds_vectors.json")
}

pub fn generate() -> Vec<DayBoundVector> {
    let days: &[(&str, &[&str])] = &[
        (
            "Africa/Cairo",
            &[
                // Spring forward at 00:00, last Friday of April.
                "2024-04-26",
                "2025-04-25",
                "2026-04-24",
                "2027-04-30",
                // Fall back, last Thursday of October.
                "2024-10-31",
                "2025-10-30",
                "2026-10-29",
                // Ordinary days.
                "2026-01-15",
                "2026-09-17",
            ],
        ),
        (
            "Asia/Beirut",
            &[
                // Spring forward at 00:00, last Sunday of March.
                "2024-03-31",
                "2025-03-30",
                "2026-03-29",
                "2027-03-28",
                // Fall back, last Sunday of October.
                "2026-10-25",
                "2026-07-01",
            ],
        ),
        ("Europe/London", &["2026-03-29", "2026-10-25", "2026-06-01"]),
        ("UTC", &["2026-04-24"]),
    ];
    let mut out = Vec::new();
    for (tz, dates) in days {
        let zone: chrono_tz::Tz = tz.parse().unwrap();
        for date in *dates {
            let date: NaiveDate = date.parse().unwrap();
            // The day itself and its two neighbours, so contiguity is pinned.
            for day in [date - Duration::days(1), date, date + Duration::days(1)] {
                let (start, end) = day_bounds(zone, day);
                let v = DayBoundVector {
                    tz: tz.to_string(),
                    date: day.to_string(),
                    start: start.to_rfc3339_opts(SecondsFormat::Secs, true),
                    end: end.to_rfc3339_opts(SecondsFormat::Secs, true),
                    hours: (end - start).num_hours(),
                };
                if !out.contains(&v) {
                    out.push(v);
                }
            }
        }
    }
    out
}

/// The bytes of `vectors/week_vectors.json`: [`crate::week_start`] of a date.
/// Regenerate deliberately:
/// `MADAR_REGENERATE_WEEK_VECTORS=1 cargo test -p madar-time week_vectors`.
pub const WEEK: &str = include_str!("../vectors/week_vectors.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WeekVector {
    pub date: String,
    /// `Mon`..`Sun`, for reading the file.
    pub weekday: String,
    pub week_start: String,
}

pub fn week_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/week_vectors.json")
}

pub fn generate_week() -> Vec<WeekVector> {
    [
        // Every weekday, Saturday to Friday.
        "2026-09-12",
        "2026-09-13",
        "2026-09-14",
        "2026-09-15",
        "2026-09-16",
        "2026-09-17",
        "2026-09-18",
        // Month boundaries.
        "2026-09-30",
        "2026-10-01",
        "2026-10-03",
        // Year boundaries.
        "2025-12-31",
        "2026-01-01",
        "2026-12-31",
        "2027-01-01",
        "2027-01-02",
        "2028-01-01",
        // Leap days, and weeks across them (2000 is a leap year, 2100 is not).
        "2024-02-29",
        "2024-03-01",
        "2028-02-29",
        "2028-03-03",
        "2028-03-04",
        "2000-02-29",
        "2100-02-28",
        "2100-03-01",
    ]
    .iter()
    .map(|s| {
        let date: NaiveDate = s.parse().unwrap();
        WeekVector {
            date: date.to_string(),
            weekday: date.weekday().to_string(),
            week_start: crate::week_start(date).to_string(),
        }
    })
    .collect()
}

/// The bytes of `vectors/business_date_vectors.json`:
/// [`crate::business_date_of`] of an instant in a zone. Regenerate deliberately:
/// `MADAR_REGENERATE_BUSINESS_DATE_VECTORS=1 cargo test -p madar-time business_date_vectors`.
pub const BUSINESS_DATE: &str = include_str!("../vectors/business_date_vectors.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BusinessDateVector {
    pub tz: String,
    /// RFC 3339, UTC.
    pub at: String,
    /// `at` on the zone's wall clock, with its offset, for reading the file.
    pub local: String,
    pub business_date: String,
}

pub fn business_date_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/business_date_vectors.json")
}

pub fn generate_business_dates() -> Vec<BusinessDateVector> {
    let cases: &[(&str, &[&str])] = &[
        (
            "Africa/Cairo",
            &[
                // Winter (UTC+2): the last second of the day and the next.
                "2026-01-15T21:59:59Z",
                "2026-01-15T22:00:00Z",
                // Summer (UTC+3).
                "2026-09-19T20:59:59Z",
                "2026-09-19T21:00:00Z",
                "2026-09-19T22:30:00Z",
                "2026-09-20T09:00:00Z",
                // Spring forward: 2026-04-24 00:00 +02 does not exist, 01:00 +03 follows 23:59:59 +02.
                "2026-04-23T21:59:59Z",
                "2026-04-23T22:00:00Z",
                // Fall back: Thursday 2026-10-29 23:00-24:00 happens twice (+03, then +02).
                "2026-10-29T20:59:59Z",
                "2026-10-29T21:00:00Z",
                "2026-10-29T21:59:59Z",
                "2026-10-29T22:00:00Z",
                // New year, leap day.
                "2026-12-31T21:59:59Z",
                "2026-12-31T22:00:00Z",
                "2028-02-28T22:00:00Z",
            ],
        ),
        (
            // UTC+3 all year.
            "Asia/Riyadh",
            &[
                "2026-01-15T20:59:59Z",
                "2026-01-15T21:00:00Z",
                "2026-07-01T20:59:59Z",
                "2026-07-01T21:00:00Z",
            ],
        ),
        (
            // GMT/BST; the clocks move at 01:00 UTC, not at midnight.
            "Europe/London",
            &[
                "2026-01-15T23:59:59Z",
                "2026-01-16T00:00:00Z",
                "2026-03-28T23:59:59Z",
                "2026-03-29T00:00:00Z",
                "2026-03-29T00:59:59Z",
                "2026-03-29T01:00:00Z",
                "2026-06-30T22:59:59Z",
                "2026-06-30T23:00:00Z",
                "2026-10-24T22:59:59Z",
                "2026-10-24T23:00:00Z",
                "2026-10-25T00:59:59Z",
                "2026-10-25T01:00:00Z",
                "2026-10-25T23:59:59Z",
                "2026-10-26T00:00:00Z",
            ],
        ),
        (
            // A zone behind UTC.
            "America/New_York",
            &[
                "2026-01-16T04:59:59Z",
                "2026-01-16T05:00:00Z",
                "2026-07-02T03:59:59Z",
                "2026-07-02T04:00:00Z",
            ],
        ),
        ("UTC", &["2026-04-23T23:59:59Z", "2026-04-24T00:00:00Z"]),
    ];
    let mut out = Vec::new();
    for (tz, instants) in cases {
        let zone: chrono_tz::Tz = tz.parse().unwrap();
        for at in *instants {
            let at = chrono::DateTime::parse_from_rfc3339(at).unwrap().to_utc();
            out.push(BusinessDateVector {
                tz: tz.to_string(),
                at: at.to_rfc3339_opts(SecondsFormat::Secs, true),
                local: at
                    .with_timezone(&zone)
                    .to_rfc3339_opts(SecondsFormat::Secs, false),
                business_date: crate::business_date_of(zone, at).to_string(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::business_date_of;

    #[test]
    fn week_vectors() {
        let generated = generate_week();
        if std::env::var("MADAR_REGENERATE_WEEK_VECTORS").is_ok() {
            std::fs::write(
                week_fixture_path(),
                serde_json::to_string_pretty(&generated).unwrap() + "\n",
            )
            .unwrap();
            return;
        }
        let expected: Vec<WeekVector> = serde_json::from_str(WEEK).unwrap();
        assert_eq!(
            generated, expected,
            "week starts drifted from their vectors"
        );
    }

    /// Worked by hand (and checked against Python's calendar): every start is
    /// a Saturday at most six days back.
    #[test]
    fn week_vectors_say_saturday() {
        let v: Vec<WeekVector> = serde_json::from_str(WEEK).unwrap();
        let find = |date: &str| v.iter().find(|x| x.date == date).unwrap();
        for (date, weekday, start) in [
            ("2026-09-12", "Sat", "2026-09-12"),
            ("2026-09-18", "Fri", "2026-09-12"),
            ("2026-10-01", "Thu", "2026-09-26"),
            ("2027-01-01", "Fri", "2026-12-26"),
            ("2028-02-29", "Tue", "2028-02-26"),
            ("2028-03-04", "Sat", "2028-03-04"),
            ("2100-03-01", "Mon", "2100-02-27"),
        ] {
            let x = find(date);
            assert_eq!(
                (x.weekday.as_str(), x.week_start.as_str()),
                (weekday, start)
            );
        }
        for x in &v {
            let date: NaiveDate = x.date.parse().unwrap();
            let start: NaiveDate = x.week_start.parse().unwrap();
            assert_eq!(start.weekday(), chrono::Weekday::Sat, "{}", x.date);
            assert!(
                start <= date && date - start < Duration::days(7),
                "{}",
                x.date
            );
        }
    }

    #[test]
    fn business_date_vectors() {
        let generated = generate_business_dates();
        if std::env::var("MADAR_REGENERATE_BUSINESS_DATE_VECTORS").is_ok() {
            std::fs::write(
                business_date_fixture_path(),
                serde_json::to_string_pretty(&generated).unwrap() + "\n",
            )
            .unwrap();
            return;
        }
        let expected: Vec<BusinessDateVector> = serde_json::from_str(BUSINESS_DATE).unwrap();
        assert_eq!(
            generated, expected,
            "business dates drifted from their vectors"
        );
    }

    /// Worked by hand from the zones' offsets and transition instants.
    #[test]
    fn business_date_vectors_say_the_wall_clock_date() {
        let v: Vec<BusinessDateVector> = serde_json::from_str(BUSINESS_DATE).unwrap();
        let find = |tz: &str, at: &str| v.iter().find(|x| x.tz == tz && x.at == at).unwrap();
        for (tz, at, local, date) in [
            (
                "Africa/Cairo",
                "2026-01-15T21:59:59Z",
                "2026-01-15T23:59:59+02:00",
                "2026-01-15",
            ),
            (
                "Africa/Cairo",
                "2026-09-19T21:00:00Z",
                "2026-09-20T00:00:00+03:00",
                "2026-09-20",
            ),
            (
                "Africa/Cairo",
                "2026-04-23T22:00:00Z",
                "2026-04-24T01:00:00+03:00",
                "2026-04-24",
            ),
            (
                "Africa/Cairo",
                "2026-10-29T21:00:00Z",
                "2026-10-29T23:00:00+02:00",
                "2026-10-29",
            ),
            (
                "Africa/Cairo",
                "2026-10-29T22:00:00Z",
                "2026-10-30T00:00:00+02:00",
                "2026-10-30",
            ),
            (
                "Asia/Riyadh",
                "2026-07-01T21:00:00Z",
                "2026-07-02T00:00:00+03:00",
                "2026-07-02",
            ),
            (
                "Europe/London",
                "2026-06-30T23:00:00Z",
                "2026-07-01T00:00:00+01:00",
                "2026-07-01",
            ),
            (
                "Europe/London",
                "2026-10-25T01:00:00Z",
                "2026-10-25T01:00:00+00:00",
                "2026-10-25",
            ),
            (
                "America/New_York",
                "2026-01-16T04:59:59Z",
                "2026-01-15T23:59:59-05:00",
                "2026-01-15",
            ),
        ] {
            let x = find(tz, at);
            assert_eq!((x.local.as_str(), x.business_date.as_str()), (local, date));
        }
        for x in &v {
            assert_eq!(&x.local[..10], x.business_date, "{} {}", x.tz, x.at);
        }
    }

    #[test]
    fn day_bound_vectors() {
        let generated = generate();
        if std::env::var("MADAR_REGENERATE_TIME_VECTORS").is_ok() {
            std::fs::write(
                fixture_path(),
                serde_json::to_string_pretty(&generated).unwrap() + "\n",
            )
            .unwrap();
            return;
        }
        let expected: Vec<DayBoundVector> = serde_json::from_str(DAY_BOUNDS).unwrap();
        assert_eq!(generated, expected, "day bounds drifted from their vectors");
    }

    /// The facts the vectors must keep saying, written out by hand: a gap day
    /// starts at the first real local time (01:00 local), and every day's
    /// bounds tile with the next day's.
    #[test]
    fn gap_days_start_at_the_first_real_local_time_and_days_tile() {
        let v: Vec<DayBoundVector> = serde_json::from_str(DAY_BOUNDS).unwrap();
        let find = |tz: &str, date: &str| v.iter().find(|x| x.tz == tz && x.date == date).unwrap();
        for (tz, date, start) in [
            ("Africa/Cairo", "2026-04-24", "2026-04-23T22:00:00Z"),
            ("Africa/Cairo", "2024-04-26", "2024-04-25T22:00:00Z"),
            ("Africa/Cairo", "2025-04-25", "2025-04-24T22:00:00Z"),
            ("Asia/Beirut", "2026-03-29", "2026-03-28T22:00:00Z"),
        ] {
            let x = find(tz, date);
            assert_eq!(x.start, start, "{tz} {date}");
            assert_eq!(x.hours, 23, "{tz} {date}");
        }
        assert_eq!(find("Africa/Cairo", "2026-10-29").hours, 25);
        assert_eq!(
            find("Africa/Cairo", "2026-09-17").start,
            "2026-09-16T21:00:00Z"
        );
        for x in &v {
            let next: NaiveDate = x.date.parse::<NaiveDate>().unwrap() + Duration::days(1);
            if let Some(n) = v
                .iter()
                .find(|n| n.tz == x.tz && n.date == next.to_string())
            {
                assert_eq!(
                    x.end, n.start,
                    "{} {} tiles with the next day",
                    x.tz, x.date
                );
            }
            // Every instant inside the day is that business date.
            let tz: chrono_tz::Tz = x.tz.parse().unwrap();
            let start = chrono::DateTime::parse_from_rfc3339(&x.start)
                .unwrap()
                .to_utc();
            let end = chrono::DateTime::parse_from_rfc3339(&x.end)
                .unwrap()
                .to_utc();
            assert_eq!(business_date_of(tz, start).to_string(), x.date);
            assert_eq!(
                business_date_of(tz, end - Duration::seconds(1)).to_string(),
                x.date
            );
        }
    }
}
