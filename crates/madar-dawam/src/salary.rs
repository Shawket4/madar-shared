//! Salary money: the rates a monthly salary divides into, and the first pay of
//! someone who joins mid-period. The SERVER's arithmetic, moved here so the
//! dashboards stop carrying copies (`salary-calc.ts`, `employees_data.dart`):
//! money in piastres, every intermediate an exact `Decimal`, multiply before
//! dividing, one rounding at the end, half away from zero. Pinned by
//! `vectors/salary_vectors.json`, worked out by hand.

use chrono::{Duration, NaiveDate};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};

use crate::pay::period_window;

/// Piastres, half away from zero (`Decimal::round` would be banker's).
/// MadarRust `costing::service::round_piastres`.
pub fn round_piastres(piastres: Decimal) -> i64 {
    piastres
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i64()
        .unwrap_or(0)
}

/// A monthly salary and the two divisors that break it into days and minutes.
/// MadarRust `staff::rules::PayRates`, line for line: the rates are not
/// precomputed, every accessor multiplies by the quantity BEFORE dividing
/// (10,000/day ÷ 480 min × 30 min is 625, not 624.99…).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayRates {
    base_salary_piastres: i64,
    working_days_per_month: Decimal,
    scheduled_minutes_per_day: i64,
}

impl PayRates {
    /// A negative salary reads as zero; a zero or negative divisor yields a
    /// zero rate, never a panic (`PayRates::from_base`).
    pub fn from_base(
        base_salary_piastres: i64,
        working_days_per_month: Decimal,
        scheduled_minutes_per_day: i64,
    ) -> Self {
        Self {
            base_salary_piastres: base_salary_piastres.max(0),
            working_days_per_month,
            scheduled_minutes_per_day,
        }
    }

    fn base(&self) -> Decimal {
        Decimal::from(self.base_salary_piastres)
    }

    /// What `days` days of work are worth (`PayRates::days_piastres`).
    pub fn days_piastres(&self, days: Decimal) -> Decimal {
        if self.working_days_per_month <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        self.base() * days / self.working_days_per_month
    }

    /// One day's pay (`PayRates::daily_piastres`).
    pub fn daily_piastres(&self) -> Decimal {
        self.days_piastres(Decimal::ONE)
    }

    /// What `minutes` minutes are worth at the plain rate
    /// (`PayRates::minutes_piastres`).
    pub fn minutes_piastres(&self, minutes: Decimal) -> Decimal {
        if self.working_days_per_month <= Decimal::ZERO || self.scheduled_minutes_per_day <= 0 {
            return Decimal::ZERO;
        }
        self.base() * minutes
            / (self.working_days_per_month * Decimal::from(self.scheduled_minutes_per_day))
    }
}

/// The one rate the salary calculator was typed with, in piastres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typed {
    Monthly(i64),
    Daily(i64),
    Hourly(i64),
}

/// A salary as a month, a day and an hour, in piastres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rates {
    pub monthly: i64,
    pub daily: i64,
    pub hourly: i64,
}

/// The three rates from whichever one was typed (owner decision 9, RU-6: the
/// day rate is the monthly ÷ working days, the hour is 60 minutes at the
/// minute rate, the day rate ÷ the day's minutes).
///
/// From a monthly salary these are the server's figures, rounded once:
/// `round_piastres(PayRates::daily_piastres())` and
/// `round_piastres(PayRates::minutes_piastres(60))`. The server stores only
/// the monthly salary, so it has no day-to-month or hour-to-month rule: those
/// directions are the same products, multiplied before divided and rounded
/// once. A negative figure, or a zero divisor, reads as zero.
pub fn rates(typed: Typed, working_days_per_month: Decimal, day_minutes: i64) -> Rates {
    let r = |x: Decimal| round_piastres(x).max(0);
    let days = working_days_per_month;
    let mins = Decimal::from(day_minutes);
    let sixty = Decimal::from(60);
    match typed {
        Typed::Monthly(m) => {
            let p = PayRates::from_base(m, days, day_minutes);
            Rates {
                monthly: m.max(0),
                daily: r(p.daily_piastres()),
                hourly: r(p.minutes_piastres(sixty)),
            }
        }
        Typed::Daily(d) => {
            let d = d.max(0);
            Rates {
                monthly: r(Decimal::from(d) * days),
                daily: d,
                hourly: if day_minutes <= 0 {
                    0
                } else {
                    r(Decimal::from(d) * sixty / mins)
                },
            }
        }
        Typed::Hourly(h) => {
            let h = h.max(0);
            Rates {
                monthly: r(Decimal::from(h) * mins * days / sixty),
                daily: r(Decimal::from(h) * mins / sixty),
                hourly: h,
            }
        }
    }
}

/// The salary in force on `day` from a dated history (the newest row at or
/// before the day wins); `fallback` when the history starts later.
/// MadarRust `staff::pricing::salary_on`.
pub fn salary_on(history: &[(NaiveDate, i64)], day: NaiveDate, fallback: i64) -> i64 {
    history
        .iter()
        .filter(|(from, _)| *from <= day)
        .max_by_key(|(from, _)| *from)
        .map_or(fallback, |(_, s)| *s)
}

/// Base pay for a window, pro rata by calendar days at each day's salary
/// (PAY-13): `Σ salary(day) ÷ window days`, summed before divided, rounded
/// once, never below zero. A full window at one salary pays exactly that
/// salary. MadarRust `staff::pricing::prorated_base`, which payroll
/// (`staff::payroll`) calls with the period and the days employed in it.
pub fn prorated_base(
    history: &[(NaiveDate, i64)],
    fallback_salary: i64,
    window_start: NaiveDate,
    window_end: NaiveDate,
    paid_from: NaiveDate,
    paid_to: NaiveDate,
) -> i64 {
    let window_days = (window_end - window_start).num_days() + 1;
    if window_days <= 0 || paid_to < paid_from {
        return 0;
    }
    let mut sum = Decimal::ZERO;
    let mut day = paid_from.max(window_start);
    let last = paid_to.min(window_end);
    while day <= last {
        sum += Decimal::from(salary_on(history, day, fallback_salary));
        day += Duration::days(1);
    }
    round_piastres(sum / Decimal::from(window_days)).max(0)
}

/// What someone hired on a day earns in their first pay period.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirstPay {
    /// The hire date.
    pub from: NaiveDate,
    /// The period's last day.
    pub to: NaiveDate,
    /// Days employed in the period, both ends counted.
    pub days: i64,
    pub period_days: i64,
    pub piastres: i64,
}

/// The first pay of someone hired on `hire_date` at `monthly`: the period
/// holding the hire date ([`period_window`], periods opening on `start_day`),
/// paid from the hire date to the period's end at the server's pro rata —
/// payroll's `prorated_base(history, salary, start, end, hire.max(start), end)`
/// with one salary, i.e. `monthly × days ÷ period days`, rounded once.
pub fn first_pay(monthly: i64, hire_date: NaiveDate, start_day: i64) -> FirstPay {
    let (start, end) = period_window(hire_date, start_day);
    FirstPay {
        from: hire_date,
        to: end,
        days: (end - hire_date).num_days() + 1,
        period_days: (end - start).num_days() + 1,
        piastres: prorated_base(&[], monthly, start, end, hire_date, end),
    }
}

pub mod vectors {
    //! `vectors/salary_vectors.json`: every figure worked out by hand (exact
    //! fractions, half away from zero), never generated from this crate. A
    //! case the dashboards' copies get wrong carries `client`: what
    //! `salary-calc.ts` and `employees_data.dart` give instead.

    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct SalaryVectors {
        pub rates: Vec<RatesVector>,
        pub first_pay: Vec<FirstPayVector>,
        pub prorated: Vec<ProratedVector>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct RatesVector {
        /// `monthly`, `daily` or `hourly`.
        pub typed: String,
        pub value: i64,
        /// As text (`numeric(5,2)`).
        pub working_days: String,
        pub day_minutes: i64,
        pub monthly: i64,
        pub daily: i64,
        pub hourly: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub client: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct FirstPayVector {
        pub monthly: i64,
        pub hire_date: String,
        pub start_day: i64,
        pub from: String,
        pub to: String,
        pub days: i64,
        pub period_days: i64,
        pub piastres: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub client: Option<String>,
    }

    /// `prorated_base` with a salary history (date from which a salary holds).
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct ProratedVector {
        pub history: Vec<(String, i64)>,
        pub fallback: i64,
        pub window: (String, String),
        pub paid: (String, String),
        pub piastres: i64,
        pub note: String,
    }

    #[cfg(test)]
    mod tests {
        use chrono::NaiveDate;
        use rust_decimal::Decimal;

        use super::*;
        use crate::salary::{first_pay, prorated_base, rates, Typed};

        fn d(s: &str) -> NaiveDate {
            s.parse().unwrap()
        }

        #[test]
        fn salary_vectors() {
            let v: SalaryVectors = serde_json::from_str(crate::vectors::SALARY).unwrap();
            for x in &v.rates {
                let typed = match x.typed.as_str() {
                    "monthly" => Typed::Monthly(x.value),
                    "daily" => Typed::Daily(x.value),
                    "hourly" => Typed::Hourly(x.value),
                    other => panic!("unknown typed {other}"),
                };
                let wd: Decimal = x.working_days.parse().unwrap();
                let got = rates(typed, wd, x.day_minutes);
                assert_eq!(
                    (got.monthly, got.daily, got.hourly),
                    (x.monthly, x.daily, x.hourly),
                    "{x:?}"
                );
            }
            for x in &v.first_pay {
                let got = first_pay(x.monthly, d(&x.hire_date), x.start_day);
                assert_eq!(
                    (
                        got.from.to_string(),
                        got.to.to_string(),
                        got.days,
                        got.period_days,
                        got.piastres
                    ),
                    (
                        x.from.clone(),
                        x.to.clone(),
                        x.days,
                        x.period_days,
                        x.piastres
                    ),
                    "{x:?}"
                );
            }
            for x in &v.prorated {
                let history: Vec<(NaiveDate, i64)> =
                    x.history.iter().map(|(s, p)| (d(s), *p)).collect();
                let got = prorated_base(
                    &history,
                    x.fallback,
                    d(&x.window.0),
                    d(&x.window.1),
                    d(&x.paid.0),
                    d(&x.paid.1),
                );
                assert_eq!(got, x.piastres, "{}", x.note);
            }
        }
    }
}
