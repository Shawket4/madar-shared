//! The late-penalty ladder and the absence charge, priced as payroll prices
//! them. The SERVER's arithmetic (MadarRust `staff::rules`), moved here so the
//! Rules pages stop carrying copies (`rules-preview.ts`, `ladder_pricing.dart`):
//! they priced in floating point and came out a piastre short on some day
//! fractions. Pinned by `vectors/ladder_vectors.json`, worked out by hand.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::salary::{round_piastres, PayRates};

/// What a rung costs the employee (MadarRust `staff::rules::LateDeductionKind`;
/// the wire names `minutes`, `piastres`, `day_fraction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LateDeductionKind {
    /// Dock N minutes of pay.
    Minutes,
    /// Dock a flat sum.
    Piastres,
    /// Dock a fraction of a day's pay.
    DayFraction,
}

/// One rung: inclusive at both ends, `to_minutes = None` the open top rung
/// (MadarRust `staff::rules::LateTier`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateTier {
    pub from_minutes: i32,
    pub to_minutes: Option<i32>,
    pub kind: LateDeductionKind,
    pub value: Decimal,
}

/// The FIRST rung `late_minutes` falls on, or `None` (on time, or past a
/// ladder that stops). MadarRust `staff::rules::select_late_tier`.
pub fn select_late_tier(tiers: &[LateTier], late_minutes: i64) -> Option<&LateTier> {
    if late_minutes <= 0 {
        return None;
    }
    tiers.iter().find(|t| {
        let above = late_minutes >= t.from_minutes.max(0) as i64;
        let below = t.to_minutes.is_none_or(|to| late_minutes <= to as i64);
        above && below
    })
}

/// What a rung costs in piastres at `rates` (the salary, working days and the
/// day's rostered minutes): multiplied before divided, rounded once, never
/// below zero. MadarRust `staff::rules::late_deduction_piastres`.
pub fn late_deduction_piastres(tier: &LateTier, rates: &PayRates) -> i64 {
    let raw = match tier.kind {
        LateDeductionKind::Minutes => rates.minutes_piastres(tier.value),
        LateDeductionKind::Piastres => tier.value,
        LateDeductionKind::DayFraction => rates.days_piastres(tier.value),
    };
    round_piastres(raw).max(0)
}

/// What `days_absent` absent days cost at `deduction_days_per_absence` days
/// docked each (the Rules pages' `dayPiastres`). MadarRust
/// `staff::rules::absence_deduction_piastres`.
pub fn absence_deduction_piastres(
    rates: &PayRates,
    days_absent: Decimal,
    deduction_days_per_absence: Decimal,
) -> i64 {
    round_piastres(rates.days_piastres(
        days_absent.max(Decimal::ZERO) * deduction_days_per_absence.max(Decimal::ZERO),
    ))
    .max(0)
}

pub mod vectors {
    //! `vectors/ladder_vectors.json`: every figure worked out by hand (exact
    //! fractions, half away from zero), never generated from this crate. A
    //! case the dashboards' copies get wrong carries `client`: what
    //! `rules-preview.ts` (`tierPiastres`, `dayPiastres`) and
    //! `ladder_pricing.dart` (`rungPiastres`, `dayPiastres`) give instead.

    use serde::{Deserialize, Serialize};

    use super::LateDeductionKind;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct LadderVectors {
        pub ladder: Vec<TierVector>,
        pub select: Vec<SelectVector>,
        pub deductions: Vec<DeductionVector>,
        pub absences: Vec<AbsenceVector>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct TierVector {
        pub from_minutes: i32,
        pub to_minutes: Option<i32>,
        pub kind: LateDeductionKind,
        /// As text (`numeric`).
        pub value: String,
    }

    /// The rung of `ladder` a lateness falls on: its index, or `null`.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct SelectVector {
        pub late_minutes: i64,
        pub tier: Option<usize>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct DeductionVector {
        pub kind: LateDeductionKind,
        pub value: String,
        pub salary: i64,
        pub working_days: String,
        pub day_minutes: i64,
        pub piastres: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub client: Option<i64>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct AbsenceVector {
        pub salary: i64,
        pub working_days: String,
        pub days_absent: String,
        pub deduction_days: String,
        pub piastres: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub client: Option<i64>,
    }

    #[cfg(test)]
    mod tests {
        use rust_decimal::Decimal;

        use super::*;
        use crate::ladder::{
            absence_deduction_piastres, late_deduction_piastres, select_late_tier, LateTier,
        };
        use crate::salary::PayRates;

        fn dec(s: &str) -> Decimal {
            s.parse().unwrap()
        }

        #[test]
        fn ladder_vectors() {
            let v: LadderVectors = serde_json::from_str(crate::vectors::LADDER).unwrap();
            let ladder: Vec<LateTier> = v
                .ladder
                .iter()
                .map(|t| LateTier {
                    from_minutes: t.from_minutes,
                    to_minutes: t.to_minutes,
                    kind: t.kind,
                    value: dec(&t.value),
                })
                .collect();
            for x in &v.select {
                let got = select_late_tier(&ladder, x.late_minutes)
                    .map(|t| ladder.iter().position(|l| std::ptr::eq(l, t)).unwrap());
                assert_eq!(got, x.tier, "{x:?}");
            }
            for x in &v.deductions {
                let tier = LateTier {
                    from_minutes: 0,
                    to_minutes: None,
                    kind: x.kind,
                    value: dec(&x.value),
                };
                let rates = PayRates::from_base(x.salary, dec(&x.working_days), x.day_minutes);
                assert_eq!(late_deduction_piastres(&tier, &rates), x.piastres, "{x:?}");
            }
            for x in &v.absences {
                // The day's minutes do not enter a day's pay.
                let rates = PayRates::from_base(x.salary, dec(&x.working_days), 480);
                assert_eq!(
                    absence_deduction_piastres(&rates, dec(&x.days_absent), dec(&x.deduction_days)),
                    x.piastres,
                    "{x:?}"
                );
            }
        }
    }
}
