//! The pay period.

use chrono::{Datelike, Duration, Months, NaiveDate};

/// The pay window holding `day`: from `start_day` of one month to the day
/// before it in the next. `start_day` is clamped to 1–28; 1 is the calendar
/// month. MadarRust `staff/dawam/pay.rs` `period_window`; madar-core
/// `dawam.rs` `period_around`.
pub fn period_window(day: NaiveDate, start_day: i64) -> (NaiveDate, NaiveDate) {
    let start_day = start_day.clamp(1, 28) as u32;
    let this = NaiveDate::from_ymd_opt(day.year(), day.month(), start_day).expect("day <= 28");
    let start = if day >= this {
        this
    } else {
        this.checked_sub_months(Months::new(1)).expect("in range")
    };
    let end = start.checked_add_months(Months::new(1)).expect("in range") - Duration::days(1);
    (start, end)
}
