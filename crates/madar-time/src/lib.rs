//! # madar-time
//!
//! Business-day rules the backend (MadarRust) and the Rust cores (madar-core)
//! both run, in ONE copy. Every rule reads the branch's wall clock (its IANA
//! zone), never the device's and never UTC.
//!
//! - [`WEEK_START`] / [`week_start`]: weeks start on SATURDAY (owner rule,
//!   2026-09-17). The dashboard (`lib/week.ts`) and the backend's SQL helper
//!   (`tz::week_start_sql`) carry the same rule.
//! - [`business_date_of`]: the branch-local date of an instant.
//! - [`yymmdd`] / [`yymmdd_in`]: the `YYMMDD` segment of order, ticket and
//!   delivery refs.
//! - [`day_bounds`]: a branch-local calendar day as UTC bounds, with the DST
//!   gap rule. Pinned by `vectors/day_bounds_vectors.json` (Cairo and Beirut
//!   gap days, where midnight does not exist).
//!
//! Nothing here reads a clock or does I/O.

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

pub mod vectors;

/// The first day of a week, everywhere (owner rule, 2026-09-17): SATURDAY.
/// Every "this week"/"last week" window and every weekly bucket starts on it,
/// read on the scope's wall clock.
pub const WEEK_START: chrono::Weekday = chrono::Weekday::Sat;

/// The local date the week containing `day` starts on.
pub fn week_start(day: NaiveDate) -> NaiveDate {
    let back = (7 + day.weekday().num_days_from_monday() as i64
        - WEEK_START.num_days_from_monday() as i64)
        % 7;
    day - Duration::days(back)
}

/// The branch-local business date of an instant — the boundary [`day_bounds`]
/// draws. A DST gap cannot move a date, so the naive local date is exactly
/// right here.
pub fn business_date_of(tz: Tz, at: DateTime<Utc>) -> NaiveDate {
    tz.from_utc_datetime(&at.naive_utc()).date_naive()
}

/// `YYMMDD` of a date: the date segment of `<BRANCH>-<YYMMDD>-…` refs.
pub fn yymmdd(date: NaiveDate) -> String {
    date.format("%y%m%d").to_string()
}

/// `YYMMDD` of an instant read in `tz` (the order-ref date segment the till
/// predicts offline).
pub fn yymmdd_in(tz: Tz, at: DateTime<Utc>) -> String {
    at.with_timezone(&tz).format("%y%m%d").to_string()
}

/// The first instant of a branch-local calendar day. A DST gap can swallow
/// midnight (Cairo springs forward at 00:00 on the last Friday of April, Beirut
/// on the last Sunday of March): the day then starts at the first wall-clock
/// time that exists, stepping forward 30 minutes at a time. If none of the
/// first four half-hours exists, midnight is read as UTC.
pub fn local_midnight(tz: Tz, date: NaiveDate) -> DateTime<Utc> {
    let mut t = date.and_time(NaiveTime::MIN);
    for _ in 0..4 {
        if let Some(d) = tz.from_local_datetime(&t).earliest() {
            return d.with_timezone(&Utc);
        }
        t += Duration::minutes(30);
    }
    Utc.from_utc_datetime(&date.and_time(NaiveTime::MIN))
}

/// A branch-local calendar day as UTC bounds `[start, end)`: local midnight to
/// the next local midnight, so consecutive days tile with no gap and no
/// overlap, and a DST day is 23 or 25 hours long.
///
/// Moved from MadarRust `bookings::handlers::service_day_bounds`; madar-core's
/// `timefmt::local_day_bounds` is these instants at the zone's offset.
pub fn day_bounds(tz: Tz, date: NaiveDate) -> (DateTime<Utc>, DateTime<Utc>) {
    (
        local_midnight(tz, date),
        local_midnight(tz, date + Duration::days(1)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, dd: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, dd).unwrap()
    }

    #[test]
    fn weeks_start_on_saturday() {
        assert_eq!(WEEK_START, chrono::Weekday::Sat);
        // 2026-09-18 is a Friday, 09-19 a Saturday.
        assert_eq!(week_start(d(2026, 9, 18)), d(2026, 9, 12));
        assert_eq!(week_start(d(2026, 9, 19)), d(2026, 9, 19));
        assert_eq!(week_start(d(2026, 9, 25)), d(2026, 9, 19));
        for n in 0..14 {
            let day = d(2026, 9, 10) + Duration::days(n);
            let s = week_start(day);
            assert_eq!(s.weekday(), chrono::Weekday::Sat);
            assert!(s <= day && day - s < Duration::days(7));
        }
    }

    #[test]
    fn a_ref_date_is_the_branch_local_date() {
        let cairo = chrono_tz::Africa::Cairo;
        // 22:30 UTC on the 19th is 01:30 on the 20th in Cairo (UTC+3).
        let late = Utc.with_ymd_and_hms(2026, 9, 19, 22, 30, 0).unwrap();
        assert_eq!(yymmdd_in(cairo, late), "260920");
        assert_eq!(yymmdd(business_date_of(cairo, late)), "260920");
        assert_eq!(yymmdd(d(2027, 1, 5)), "270105");
    }

    /// Moved from MadarRust `bookings::handlers::day_tests`.
    #[test]
    fn dst_days_are_23_and_25_hours() {
        let tz: Tz = "Europe/London".parse().unwrap();
        let (s, e) = day_bounds(tz, d(2026, 3, 29));
        assert_eq!(e - s, Duration::hours(23));
        let (s, e) = day_bounds(tz, d(2026, 10, 25));
        assert_eq!(e - s, Duration::hours(25));
        // Midnight itself skipped (Asia/Beirut springs forward at 00:00).
        let tz: Tz = "Asia/Beirut".parse().unwrap();
        let (s, e) = day_bounds(tz, d(2026, 3, 29));
        assert_eq!(e - s, Duration::hours(23));
    }
}
