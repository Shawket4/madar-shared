//! A shift's wall-clock times as instants: the one rule (DW1).
//!
//! The server places a shift in SQL — `(date + start) AT TIME ZONE zone`, and
//! the end on the next date when it does not come after the start (an
//! overnight shift) — in its roster resolver (MadarRust
//! `dawam_roster(...)`), and sends the instants. The staff app's core places a
//! shift the server sent no instant for (madar-core `dawam.rs`, which used
//! chrono's `.earliest()` first: no instant at all in the spring gap, and one
//! hour early in the autumn). This is Postgres's placement, pinned to the
//! server's SQL by `vectors/shift_vectors.json` (MadarRust runs every case
//! through Postgres):
//!
//! - a time that happens once is that instant;
//! - a time that happens twice (the autumn fall-back) is the LATER one, the
//!   standard-time reading;
//! - a time that never happens (the spring-forward gap) moves forward by the
//!   gap: it is read at the offset in force before the jump.

use chrono::offset::LocalResult;
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone, Utc};
use chrono_tz::Tz;

/// A wall-clock time in `tz` as an instant, the way Postgres's `timestamp AT
/// TIME ZONE tz` places it.
pub fn wall_instant(tz: Tz, wall: NaiveDateTime) -> Option<DateTime<Utc>> {
    match tz.from_local_datetime(&wall) {
        LocalResult::Single(x) => Some(x.with_timezone(&Utc)),
        LocalResult::Ambiguous(_, later) => Some(later.with_timezone(&Utc)),
        // In the gap: `wall` read at the offset in force just before the jump
        // (found from the first wall time after it that exists).
        LocalResult::None => {
            let jump = (1..=180).find_map(|m| {
                tz.from_local_datetime(&(wall + Duration::minutes(m)))
                    .earliest()
            })?;
            let before = tz
                .offset_from_utc_datetime(&(jump.naive_utc() - Duration::seconds(1)))
                .fix();
            Some(Utc.from_utc_datetime(
                &(wall - Duration::seconds(i64::from(before.local_minus_utc()))),
            ))
        }
    }
}

/// Whether a shift from `start` to `end` crosses midnight: its end does not
/// come after its start (`end <= start`, a 24-hour shift included).
pub fn overnight(start: NaiveTime, end: NaiveTime) -> bool {
    end <= start
}

/// A shift on `date` from `start` to `end`, wall-clock in `tz`, as instants:
/// the end on the next date when the shift is [`overnight`].
pub fn instants(
    tz: Tz,
    date: NaiveDate,
    start: NaiveTime,
    end: NaiveTime,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let end_date = if overnight(start, end) {
        date.succ_opt()?
    } else {
        date
    };
    Some((
        wall_instant(tz, date.and_time(start))?,
        wall_instant(tz, end_date.and_time(end))?,
    ))
}

pub mod vectors {
    //! Shifts on ordinary days, across midnight and on DST days in the zones
    //! a branch may be in. Regenerate deliberately:
    //! `MADAR_REGENERATE_SHIFT_VECTORS=1 cargo test -p madar-dawam shift_vectors`
    //! — then MadarRust's `tests/dawam_shared_rules_tests.rs` checks every case
    //! against Postgres.

    use std::path::PathBuf;

    use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
    use serde::{Deserialize, Serialize};

    use super::instants;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct ShiftVector {
        pub zone: String,
        pub date: NaiveDate,
        pub start: NaiveTime,
        pub end: NaiveTime,
        pub start_at: DateTime<Utc>,
        pub end_at: DateTime<Utc>,
    }

    pub fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/shift_vectors.json")
    }

    pub fn generate() -> Vec<ShiftVector> {
        let days: &[(&str, &[&str])] = &[
            // Cairo: spring forward at 00:00 on the last Friday of April,
            // fall back at 24:00 on the last Thursday of October.
            (
                "Africa/Cairo",
                &[
                    "2026-04-23",
                    "2026-04-24",
                    "2026-10-29",
                    "2026-10-30",
                    "2026-06-15",
                ],
            ),
            (
                "Asia/Beirut",
                &["2026-03-28", "2026-03-29", "2026-10-24", "2026-10-25"],
            ),
            ("Europe/Berlin", &["2026-03-29", "2026-10-25"]),
            ("Asia/Riyadh", &["2026-03-29"]),
            ("UTC", &["2026-10-25"]),
        ];
        let times = [
            ("00:00", "08:00"),
            ("00:30", "08:30"),
            ("02:30", "10:00"),
            ("09:00", "17:00"),
            ("22:00", "06:00"),
            ("23:30", "00:30"),
            ("23:00", "23:00"),
            ("16:00", "00:00"),
        ];
        let mut out = Vec::new();
        for (zone, dates) in days {
            let tz: chrono_tz::Tz = zone.parse().unwrap();
            for d in *dates {
                let date: NaiveDate = d.parse().unwrap();
                for (s, e) in times {
                    let start: NaiveTime = format!("{s}:00").parse().unwrap();
                    let end: NaiveTime = format!("{e}:00").parse().unwrap();
                    let (start_at, end_at) = instants(tz, date, start, end).unwrap();
                    out.push(ShiftVector {
                        zone: zone.to_string(),
                        date,
                        start,
                        end,
                        start_at,
                        end_at,
                    });
                }
            }
        }
        out
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn shift_vectors() {
            let generated = generate();
            if std::env::var("MADAR_REGENERATE_SHIFT_VECTORS").is_ok() {
                std::fs::write(
                    fixture_path(),
                    serde_json::to_string_pretty(&generated).unwrap() + "\n",
                )
                .unwrap();
                return;
            }
            let expected: Vec<ShiftVector> = serde_json::from_str(crate::vectors::SHIFT).unwrap();
            assert_eq!(generated, expected, "the shift placement drifted");
        }

        /// DW1's two cases, as the discovery found them.
        #[test]
        fn cairo_dst_days_are_placed_as_postgres_places_them() {
            let tz: chrono_tz::Tz = "Africa/Cairo".parse().unwrap();
            let at = |d: &str, t: &str| {
                super::super::wall_instant(tz, format!("{d}T{t}:00").parse().unwrap()).unwrap()
            };
            assert_eq!(
                at("2026-04-24", "00:30").to_rfc3339(),
                "2026-04-23T22:30:00+00:00"
            );
            assert_eq!(
                at("2026-10-29", "23:30").to_rfc3339(),
                "2026-10-29T21:30:00+00:00"
            );
        }
    }
}
