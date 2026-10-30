//! # madar-time
//!
//! Business-day rules the backend (MadarRust) and the Rust cores (madar-core)
//! both run, in ONE copy. Every rule reads the branch's wall clock (its IANA
//! zone), never the device's and never UTC.
//!
//! - [`WEEK_START`] / [`week_start`]: weeks start on SATURDAY (owner rule,
//!   2026-09-17). The dashboard (`lib/week.ts`) and the backend's SQL helper
//!   (`tz::week_start_sql`) carry the same rule. Pinned by
//!   `vectors/week_vectors.json`.
//! - [`business_date_of`]: the branch-local date of an instant. Pinned by
//!   `vectors/business_date_vectors.json`.
//! - [`yymmdd`] / [`yymmdd_in`]: the `YYMMDD` segment of order, ticket and
//!   delivery refs.
//! - [`day_bounds`]: a branch-local calendar day as UTC bounds, with the DST
//!   gap rule. Pinned by `vectors/day_bounds_vectors.json` (Cairo and Beirut
//!   gap days, where midnight does not exist).
//! - [`local_parts`] / [`local_instant`]: the wall clock of a zone, both ways
//!   (a time field, a booking time, a till's hour). Pinned by
//!   `vectors/wallclock_vectors.json`. [`wall_instant`] is the general form,
//!   with the reading of a time that happens twice chosen ([`Repeated`]).
//! - [`DEFAULT_ZONE`] / [`parse_zone`] / [`zone_or_default`]: which zone. A
//!   zone is always an IANA name, never a fixed offset: Cairo is +03 from the
//!   last Friday of April to the last Thursday of October and +02 otherwise.
//! - [`offset_seconds`] / [`to_rfc3339_in`] / [`minutes_of_day`] /
//!   [`wall_clock`] / [`today_in`] / [`last_instant_of_day`]: an instant read
//!   on a zone's wall clock, at the offset in force at THAT instant.
//! - [`testing`]: wall-clock instants for tests and fixtures, so no test
//!   writes a fixed offset.
//!
//! Nothing here reads a clock or does I/O.

use chrono::offset::LocalResult;
use chrono::{
    DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Offset, SecondsFormat,
    TimeZone, Timelike, Utc,
};
use chrono_tz::Tz;

#[allow(
    clippy::unwrap_used,
    reason = "fixture code: a literal that fails to parse is a bug in this file"
)]
pub mod vectors;

pub mod testing;

/// The first day of a week, everywhere (owner rule, 2026-09-17): SATURDAY.
/// Every "this week"/"last week" window and every weekly bucket starts on it,
/// read on the scope's wall clock.
pub const WEEK_START: chrono::Weekday = chrono::Weekday::Sat;

/// The zone a shop is in when nothing says otherwise: `organizations.timezone`'s
/// column default, the end of `effective_timezone()`'s branch → org chain.
pub const DEFAULT_ZONE: Tz = chrono_tz::Africa::Cairo;

/// [`DEFAULT_ZONE`]'s IANA name, for SQL and payloads.
pub const DEFAULT_ZONE_NAME: &str = "Africa/Cairo";

/// An IANA zone name as a zone. A fixed offset (`+03:00`, `UTC+3`) or an
/// abbreviation (`EEST`) is not a zone: it is wrong half the year.
pub fn parse_zone(name: &str) -> Option<Tz> {
    name.parse().ok()
}

/// The zone to read a branch in, and whether it had to be the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zone {
    pub tz: Tz,
    /// No name, or one that is not an IANA zone: the caller should say so
    /// (a branch shown in the wrong zone is otherwise invisible).
    pub fell_back: bool,
}

/// `name` as a zone, else [`DEFAULT_ZONE`] with `fell_back` set.
pub fn zone_or_default(name: Option<&str>) -> Zone {
    match name.and_then(parse_zone) {
        Some(tz) => Zone {
            tz,
            fell_back: false,
        },
        None => Zone {
            tz: DEFAULT_ZONE,
            fell_back: true,
        },
    }
}

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

/// Today on `tz`'s wall clock, `now` being the caller's (server-corrected)
/// clock. The same date as [`business_date_of`].
pub fn today_in(tz: Tz, now: DateTime<Utc>) -> NaiveDate {
    business_date_of(tz, now)
}

/// The zone's offset from UTC at `at`, in seconds (+10800 for Cairo in
/// summer, +7200 in winter).
pub fn offset_seconds(tz: Tz, at: DateTime<Utc>) -> i32 {
    tz.offset_from_utc_datetime(&at.naive_utc())
        .fix()
        .local_minus_utc()
}

/// An offset in seconds as RFC 3339 writes it: `+03:00`, `-04:30`.
pub fn format_offset(seconds: i32) -> String {
    let sign = if seconds < 0 { '-' } else { '+' };
    let m = seconds.unsigned_abs() / 60;
    format!("{sign}{:02}:{:02}", m / 60, m % 60)
}

/// `at` written in `tz`: `2026-09-23T09:02:00+03:00` in summer,
/// `2026-10-30T08:12:00+02:00` in winter, sub-seconds kept when there are any.
/// What the screens get, so they show the branch's wall clock whatever the
/// device's zone (madar-core `dawam::in_branch_zone`).
pub fn to_rfc3339_in(tz: Tz, at: DateTime<Utc>) -> String {
    at.with_timezone(&tz)
        .to_rfc3339_opts(SecondsFormat::AutoSi, false)
}

/// Minutes since local midnight on `tz`'s wall clock (0–1439).
pub fn minutes_of_day(tz: Tz, at: DateTime<Utc>) -> u32 {
    local_parts(tz, at).minutes_of_day()
}

/// `at` on `tz`'s wall clock as `YYYY-MM-DD` and `HH:MM:SS` (madar-catalog's
/// `LocalNow`, the sale-window rule's input).
pub fn wall_clock(tz: Tz, at: DateTime<Utc>) -> (String, String) {
    let local = at.with_timezone(&tz);
    (
        local.format("%Y-%m-%d").to_string(),
        local.format("%H:%M:%S").to_string(),
    )
}

/// The last millisecond of a branch-local day: [`day_bounds`]'s end minus
/// 1 ms, for an inclusive "to" bound.
pub fn last_instant_of_day(tz: Tz, date: NaiveDate) -> DateTime<Utc> {
    day_bounds(tz, date).1 - Duration::milliseconds(1)
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

/// An instant read on a zone's wall clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalParts {
    pub date: NaiveDate,
    /// 0–23.
    pub hour: u32,
    pub minute: u32,
}

impl LocalParts {
    /// Minutes since local midnight (0–1439).
    pub fn minutes_of_day(&self) -> u32 {
        self.hour * 60 + self.minute
    }
}

/// `at` on `tz`'s wall clock: its date, hour and minute. The backend reads an
/// instant the same way (`at.with_timezone(&tz)`: MadarRust
/// `bookings::whatsapp::format_when`, `bookings::handlers::service_today`);
/// the dashboard's `localHHMM`, `toZonedInput` and the till report's
/// `hourOf` / `minutesOfDay` are this.
pub fn local_parts(tz: Tz, at: DateTime<Utc>) -> LocalParts {
    let local = at.with_timezone(&tz);
    LocalParts {
        date: local.date_naive(),
        hour: local.hour(),
        minute: local.minute(),
    }
}

/// `date` at `hour`:`minute` on `tz`'s wall clock, as an instant (the
/// dashboard's `localInstant` and `fromZonedInput`). An hour or minute past
/// its range rolls over (24:00 is the next midnight), as JavaScript's `Date`
/// does.
///
/// - A time that happens twice (the autumn fall-back) is the EARLIEST.
/// - A time that never happens (the spring-forward gap) moves forward by the
///   gap: it is read at the offset in force just before the jump, so Cairo's
///   00:30 on its gap day is 01:30 +03. At midnight that is exactly
///   [`local_midnight`] (proved over the day-bound vectors); unlike a
///   30-minute step it keeps later times later, and it is what the web's
///   `TZDate` and Postgres (`madar_dawam::shift::wall_instant`) do.
///
/// The backend's booking slots (`bookings::availability::slot_starts`)
/// instead skip a day whose opening time is in a gap or happens twice.
pub fn local_instant(tz: Tz, date: NaiveDate, hour: u32, minute: u32) -> DateTime<Utc> {
    let midnight = date.and_time(NaiveTime::MIN);
    // ponytail: hours past ~2·10⁹ (beyond chrono's calendar) read as the date's midnight.
    let wall = midnight
        .checked_add_signed(Duration::minutes(i64::from(hour) * 60 + i64::from(minute)))
        .unwrap_or(midnight);
    wall_instant(tz, wall, Repeated::Earliest)
}

/// Which reading of a wall time that happens twice (the autumn fall-back).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repeated {
    /// The first, still at summer time: [`local_instant`], the web's `TZDate`.
    Earliest,
    /// The second, at standard time: Postgres's `timestamp AT TIME ZONE`, and
    /// so a Dawam shift (`madar_dawam::shift::wall_instant`).
    Later,
}

/// A wall time on `tz`'s clock as an instant. A time that happens once is that
/// instant; one that happens twice is read as `repeated` says; one that never
/// happens (the spring-forward gap) moves forward by the gap, read at the
/// offset in force just before the jump (Cairo's 00:30 on its gap day is 01:30
/// +03). If no wall minute in the next three hours exists, `wall` is read as
/// UTC.
pub fn wall_instant(tz: Tz, wall: NaiveDateTime, repeated: Repeated) -> DateTime<Utc> {
    try_wall_instant(tz, wall, repeated).unwrap_or_else(|| Utc.from_utc_datetime(&wall))
}

/// [`wall_instant`], `None` where it would fall back to UTC.
pub fn try_wall_instant(tz: Tz, wall: NaiveDateTime, repeated: Repeated) -> Option<DateTime<Utc>> {
    match tz.from_local_datetime(&wall) {
        LocalResult::Single(x) => Some(x.with_timezone(&Utc)),
        LocalResult::Ambiguous(first, later) => Some(
            match repeated {
                Repeated::Earliest => first,
                Repeated::Later => later,
            }
            .with_timezone(&Utc),
        ),
        // In the gap: the first wall minute after it that exists is the jump.
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

    fn utc(y: i32, mo: u32, dd: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, dd, h, mi, 0).unwrap()
    }

    #[test]
    fn the_default_zone_is_the_db_default() {
        assert_eq!(DEFAULT_ZONE, chrono_tz::Africa::Cairo);
        assert_eq!(DEFAULT_ZONE.name(), DEFAULT_ZONE_NAME);
        assert_eq!(DEFAULT_ZONE_NAME, "Africa/Cairo");
    }

    #[test]
    fn a_zone_is_an_iana_name_never_an_offset() {
        assert_eq!(parse_zone("Asia/Beirut"), Some(chrono_tz::Asia::Beirut));
        for bad in ["", "Mars/Base", "+03:00", "UTC+3", "EEST"] {
            assert_eq!(parse_zone(bad), None, "{bad}");
        }
        assert_eq!(
            zone_or_default(Some("Asia/Beirut")),
            Zone {
                tz: chrono_tz::Asia::Beirut,
                fell_back: false
            }
        );
        for missing in [None, Some(""), Some("Mars/Base")] {
            assert_eq!(
                zone_or_default(missing),
                Zone {
                    tz: DEFAULT_ZONE,
                    fell_back: true
                },
                "{missing:?}"
            );
        }
    }

    /// Cairo is +03 from the last Friday of April to the last Thursday of
    /// October, +02 otherwise (it went back to +02 at 2026-10-29 24:00).
    #[test]
    fn the_offset_follows_the_zones_rules() {
        let cairo = chrono_tz::Africa::Cairo;
        assert_eq!(offset_seconds(cairo, utc(2026, 7, 15, 12, 0)), 3 * 3600);
        assert_eq!(offset_seconds(cairo, utc(2026, 1, 15, 12, 0)), 2 * 3600);
        assert_eq!(offset_seconds(cairo, utc(2026, 10, 29, 20, 59)), 3 * 3600);
        assert_eq!(offset_seconds(cairo, utc(2026, 10, 29, 21, 0)), 2 * 3600);
        assert_eq!(format_offset(3 * 3600), "+03:00");
        assert_eq!(format_offset(0), "+00:00");
        assert_eq!(format_offset(-(4 * 3600 + 30 * 60)), "-04:30");
    }

    #[test]
    fn an_instant_is_written_at_the_zones_offset_for_that_instant() {
        let cairo = chrono_tz::Africa::Cairo;
        assert_eq!(
            to_rfc3339_in(cairo, utc(2026, 9, 23, 6, 2)),
            "2026-09-23T09:02:00+03:00"
        );
        assert_eq!(
            to_rfc3339_in(cairo, utc(2026, 10, 30, 6, 12)),
            "2026-10-30T08:12:00+02:00"
        );
        let micros = utc(2026, 9, 23, 6, 2) + Duration::microseconds(123_456);
        assert_eq!(
            to_rfc3339_in(cairo, micros),
            "2026-09-23T09:02:00.123456+03:00"
        );
    }

    #[test]
    fn the_wall_clock_of_an_instant() {
        let cairo = chrono_tz::Africa::Cairo;
        // 07:40Z is 10:40 in summer and 09:40 in winter.
        assert_eq!(minutes_of_day(cairo, utc(2026, 7, 15, 7, 40)), 640);
        assert_eq!(minutes_of_day(cairo, utc(2026, 1, 15, 7, 40)), 580);
        assert_eq!(
            local_parts(cairo, utc(2026, 1, 15, 7, 40)).minutes_of_day(),
            580
        );
        assert_eq!(
            wall_clock(cairo, utc(2026, 9, 19, 21, 0) + Duration::seconds(5)),
            ("2026-09-20".to_string(), "00:00:05".to_string())
        );
        assert_eq!(today_in(cairo, utc(2026, 9, 19, 22, 30)), d(2026, 9, 20));
        assert_eq!(today_in(cairo, utc(2026, 1, 15, 21, 59)), d(2026, 1, 15));
    }

    #[test]
    fn a_wall_time_that_happens_twice_is_read_as_asked() {
        let cairo = chrono_tz::Africa::Cairo;
        // 23:30 on 2026-10-29 happens at +03 and again at +02.
        let twice = d(2026, 10, 29).and_hms_opt(23, 30, 0).unwrap();
        assert_eq!(
            wall_instant(cairo, twice, Repeated::Earliest),
            utc(2026, 10, 29, 20, 30)
        );
        assert_eq!(
            wall_instant(cairo, twice, Repeated::Later),
            utc(2026, 10, 29, 21, 30)
        );
        // 00:30 on 2026-04-24 never happens: read at the +02 in force before the jump.
        let gap = d(2026, 4, 24).and_hms_opt(0, 30, 0).unwrap();
        for r in [Repeated::Earliest, Repeated::Later] {
            assert_eq!(
                wall_instant(cairo, gap, r),
                utc(2026, 4, 23, 22, 30),
                "{r:?}"
            );
        }
        let once = d(2026, 1, 15).and_hms_opt(9, 0, 0).unwrap();
        assert_eq!(
            wall_instant(cairo, once, Repeated::Later),
            utc(2026, 1, 15, 7, 0)
        );
        // `local_instant` is the earliest reading.
        assert_eq!(
            local_instant(cairo, d(2026, 10, 29), 23, 30),
            utc(2026, 10, 29, 20, 30)
        );
    }

    #[test]
    fn the_last_instant_of_a_day_is_a_millisecond_before_the_next() {
        let cairo = chrono_tz::Africa::Cairo;
        for day in [
            d(2026, 1, 15),
            d(2026, 4, 23),
            d(2026, 4, 24),
            d(2026, 10, 29),
        ] {
            let (_, end) = day_bounds(cairo, day);
            assert_eq!(
                last_instant_of_day(cairo, day),
                end - Duration::milliseconds(1),
                "{day}"
            );
            assert_eq!(
                business_date_of(cairo, last_instant_of_day(cairo, day)),
                day,
                "{day}"
            );
        }
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
