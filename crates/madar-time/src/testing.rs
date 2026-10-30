//! Wall-clock instants for tests and fixtures, at the zone's REAL offset.
//!
//! A fixture that writes `…T08:12:00+03:00` is right for Cairo only half the
//! year: from 2026-10-29 24:00 Cairo is +02 again, and a test that reads that
//! instant back in the branch's zone gets 07:12. Build the instant from the
//! wall clock instead, so it reads back as written on any day:
//! `wall_rfc3339(tz, today, "08:12")`. To pin both halves of the year, run the
//! test on [`SEASONS`].

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use chrono_tz::Tz;

use crate::{to_rfc3339_in, wall_instant, Repeated};

/// A summer and a winter day (northern hemisphere): Cairo is +03 on the first
/// and +02 on the second.
pub const SEASONS: [NaiveDate; 2] = [
    match NaiveDate::from_ymd_opt(2026, 7, 15) {
        Some(d) => d,
        None => panic!("a valid date"),
    },
    match NaiveDate::from_ymd_opt(2026, 1, 15) {
        Some(d) => d,
        None => panic!("a valid date"),
    },
];

/// `date` at `hh_mm` (`HH:MM` or `HH:MM:SS`) on `tz`'s wall clock, as an
/// instant ([`wall_instant`], earliest reading). Panics on a malformed time:
/// that is a bug in the test.
pub fn wall(tz: Tz, date: NaiveDate, hh_mm: &str) -> DateTime<Utc> {
    let time = NaiveTime::parse_from_str(hh_mm, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(hh_mm, "%H:%M"))
        .unwrap_or_else(|_| panic!("a wall time is HH:MM or HH:MM:SS, not {hh_mm:?}"));
    wall_instant(tz, date.and_time(time), Repeated::Earliest)
}

/// [`wall`] written in `tz` at its real offset that day:
/// `wall_rfc3339(cairo, 2026-10-30, "08:12")` is `2026-10-30T08:12:00+02:00`.
pub fn wall_rfc3339(tz: Tz, date: NaiveDate, hh_mm: &str) -> String {
    to_rfc3339_in(tz, wall(tz, date, hh_mm))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone, Utc};

    fn d(y: i32, m: u32, dd: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, dd).unwrap()
    }

    #[test]
    fn a_wall_time_reads_back_as_written_in_either_season() {
        let cairo = chrono_tz::Africa::Cairo;
        assert_eq!(
            wall(cairo, d(2026, 10, 30), "08:12"),
            Utc.with_ymd_and_hms(2026, 10, 30, 6, 12, 0).unwrap()
        );
        assert_eq!(
            wall_rfc3339(cairo, d(2026, 10, 30), "08:12"),
            "2026-10-30T08:12:00+02:00"
        );
        assert_eq!(
            wall_rfc3339(cairo, d(2026, 7, 15), "08:12:30"),
            "2026-07-15T08:12:30+03:00"
        );
        for day in SEASONS {
            assert_eq!(
                crate::minutes_of_day(cairo, wall(cairo, day, "10:40")),
                640,
                "{day}"
            );
        }
    }

    #[test]
    fn the_seasons_differ_where_the_zone_keeps_dst() {
        let cairo = chrono_tz::Africa::Cairo;
        let [summer, winter] =
            SEASONS.map(|day| crate::offset_seconds(cairo, wall(cairo, day, "12:00")));
        assert_eq!((summer, winter), (3 * 3600, 2 * 3600));
    }

    #[test]
    #[should_panic(expected = "HH:MM")]
    fn a_malformed_wall_time_is_a_bug_in_the_test() {
        let _ = wall(chrono_tz::Africa::Cairo, d(2026, 1, 15), "8h12");
    }
}
