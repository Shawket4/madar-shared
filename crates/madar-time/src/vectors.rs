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

use chrono::{Duration, NaiveDate, SecondsFormat};
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::business_date_of;

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
