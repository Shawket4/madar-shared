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

/// A percentage of a salary, in minor units: `salary × percent ÷ 100`, rounded
/// half away from zero (never banker's rounding), a negative salary or
/// percentage reading as zero. The SERVER's rule (DW3): MadarRust
/// `staff::pricing::percent_of_salary` (a bonus or deduction line, the
/// salary-advance ask) and its SQL twins — `dawam_advance_cap`
/// (`round(salary::numeric * cap% / 100)`) and an adjustment's
/// `value_piastres` — pinned to this by `vectors/percent_vectors.json`
/// (MadarRust runs every case through Postgres). The staff app computed it in
/// `f64` first: 33.3 % of 1500 came to 499 there and 500 here.
pub fn percent_of_salary(base_salary_minor: i64, percent: rust_decimal::Decimal) -> i64 {
    use rust_decimal::prelude::ToPrimitive;
    use rust_decimal::{Decimal, RoundingStrategy};
    (Decimal::from(base_salary_minor.max(0)) * percent.max(Decimal::ZERO) / Decimal::from(100))
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i64()
        .unwrap_or(0)
        .max(0)
}

pub mod percent_vectors {
    //! Salaries and percentages, with the server's figure. Regenerate
    //! deliberately: `MADAR_REGENERATE_PERCENT_VECTORS=1 cargo test -p
    //! madar-dawam percent_vectors` — then MadarRust's
    //! `tests/dawam_shared_rules_tests.rs` checks every case against Postgres's
    //! `round(numeric)`.

    use std::path::PathBuf;

    use rust_decimal::Decimal;
    use serde::{Deserialize, Serialize};

    use super::percent_of_salary;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct PercentVector {
        pub salary: i64,
        /// As text, exactly as stored (`numeric`).
        pub percent: String,
        pub value: i64,
    }

    pub fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/percent_vectors.json")
    }

    pub fn generate() -> Vec<PercentVector> {
        let salaries = [0, 1, 100, 501, 1_500, 999_999, 1_200_000, 3_333_333, -500];
        let percents = [
            "0", "0.5", "1", "12.5", "33.3", "33.33", "50", "66.67", "99.99", "100", "150", "-10",
        ];
        let mut out = Vec::new();
        for salary in salaries {
            for p in percents {
                let percent: Decimal = p.parse().unwrap();
                out.push(PercentVector {
                    salary,
                    percent: p.to_string(),
                    value: percent_of_salary(salary, percent),
                });
            }
        }
        out
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn percent_vectors() {
            let generated = generate();
            if std::env::var("MADAR_REGENERATE_PERCENT_VECTORS").is_ok() {
                std::fs::write(
                    fixture_path(),
                    serde_json::to_string_pretty(&generated).unwrap() + "\n",
                )
                .unwrap();
                return;
            }
            let expected: Vec<PercentVector> =
                serde_json::from_str(crate::vectors::PERCENT).unwrap();
            assert_eq!(generated, expected, "the percent rule drifted");
        }

        /// DW3 as the discovery found it, and the server's own unit cases.
        #[test]
        fn the_servers_rounding() {
            use rust_decimal_macros::dec;
            assert_eq!(percent_of_salary(1_500, dec!(33.3)), 500);
            assert_eq!(percent_of_salary(100, dec!(0.5)), 1);
            assert_eq!(percent_of_salary(1_200_000, dec!(20)), 240_000);
            assert_eq!(percent_of_salary(501, dec!(12.5)), 63);
            assert_eq!(percent_of_salary(-1_500, dec!(33.3)), 0);
        }
    }
}
