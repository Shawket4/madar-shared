//! Recipe cost: what a recipe line costs, a recipe's total, its margin and its
//! food-cost band.
//!
//! The server's rule, MadarRust `costing/service.rs`: `round_piastres` (half
//! away from zero) and the margin of `sku_costs_impl` / `org_addon_costs`
//! (`(price − cost) / price` over the cost already rounded to piastres). The
//! server sums the lines in SQL (`SUM(quantity_used × cost_per_unit)` over
//! `numeric`, exact) and rounds once; [`recipe_cost`] is that sum.
//!
//! Owner's choices (SHARED_RULES_PLAN.md, Step 2 contract): a cost of 0 is
//! KNOWN (free); only a missing cost makes a recipe incomplete, and the known
//! lines still sum. The food-cost band is good under 30 %, fair up to and
//! including 40 %, poor above.
//!
//! Quantities and costs arrive as `f64` and are read as the decimal they print
//! as (`Decimal::from_f64`: 0.1 is 0.1, not its binary expansion), like the
//! `numeric` columns they are stored in.
//!
//! Pinned by `vectors/cost_vectors.json` (hand-computed).

use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

/// Fractional piastres to whole piastres, half away from zero (MadarRust
/// `costing::service::round_piastres`). Beyond `i64` it is 0, as there.
pub fn round_piastres(piastres: Decimal) -> i64 {
    piastres
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i64()
        .unwrap_or(0)
}

/// `qty × cost_per_unit`, exact, in fractional piastres. `None` for a
/// non-finite input or a product beyond `Decimal`.
fn product(qty: f64, cost_per_unit: f64) -> Option<Decimal> {
    Decimal::from_f64(qty)?.checked_mul(Decimal::from_f64(cost_per_unit)?)
}

/// One line's cost in whole piastres: `qty` (in the ingredient's base unit)
/// × `cost_per_unit` (piastres per base unit, may be a fraction), exact, then
/// rounded half away from zero. A non-finite input counts as 0.
pub fn line_cost(qty: f64, cost_per_unit: f64) -> i64 {
    product(qty, cost_per_unit).map_or(0, round_piastres)
}

/// A recipe line: its quantity in the ingredient's base unit and its cost per
/// base unit in piastres, `None` when the ingredient has no cost (or the line
/// links no ingredient).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CostLine {
    pub qty: f64,
    pub cost_per_unit: Option<f64>,
}

/// A recipe's cost: the known lines' exact sum, rounded once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeCost {
    pub piastres: i64,
    /// `false` when any line has no cost: `piastres` is then a partial sum,
    /// and the server shows no margin for it.
    pub complete: bool,
}

/// `Σ qty × cost` over the lines whose cost is known, exact, rounded once
/// (not per line). No lines is 0 and complete. The server returns `null`
/// instead of 0 when no line is costed; `complete` is false then. A line with
/// a non-finite input counts as 0, as in [`line_cost`].
pub fn recipe_cost(lines: &[CostLine]) -> RecipeCost {
    let sum = lines
        .iter()
        .filter_map(|l| product(l.qty, l.cost_per_unit?))
        .try_fold(Decimal::ZERO, Decimal::checked_add);
    RecipeCost {
        piastres: sum.map_or(0, round_piastres),
        complete: lines.iter().all(|l| l.cost_per_unit.is_some()),
    }
}

/// `(price − cost) / price`, both in piastres (the cost already rounded, as
/// the server does). `None` unless `price > 0`.
pub fn margin(price: i64, cost: i64) -> Option<f64> {
    (price > 0).then(|| price.saturating_sub(cost) as f64 / price as f64)
}

/// Where a food cost (cost ÷ price) sits; the margin badge uses the same
/// cut-offs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Band {
    /// Under 30 %.
    Good,
    /// 30 % up to and including 40 %.
    Fair,
    /// Above 40 %.
    Poor,
}

/// The food-cost band of `cost` against `price` (piastres), compared in
/// integers: good when `cost × 100 < 30 × price`, fair when `≤ 40 × price`,
/// else poor. `None` unless `price > 0`.
pub fn food_cost_band(cost: i64, price: i64) -> Option<Band> {
    if price <= 0 {
        return None;
    }
    let (c, p) = (i128::from(cost) * 100, i128::from(price));
    Some(if c < 30 * p {
        Band::Good
    } else if c <= 40 * p {
        Band::Fair
    } else {
        Band::Poor
    })
}

#[cfg(test)]
mod vectors {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct LineCase {
        name: String,
        qty: f64,
        cost_per_unit: f64,
        expected: i64,
    }

    #[derive(Deserialize)]
    struct RecipeCase {
        name: String,
        lines: Vec<CostLine>,
        expected: RecipeCost,
    }

    #[derive(Deserialize)]
    struct MarginCase {
        name: String,
        price: i64,
        cost: i64,
        expected: Option<f64>,
    }

    #[derive(Deserialize)]
    struct BandCase {
        name: String,
        cost: i64,
        price: i64,
        expected: Option<Band>,
    }

    #[derive(Deserialize)]
    struct Vectors {
        line_cost: Vec<LineCase>,
        recipe_cost: Vec<RecipeCase>,
        margin: Vec<MarginCase>,
        food_cost_band: Vec<BandCase>,
    }

    #[test]
    fn cost_vectors() {
        let v: Vectors = serde_json::from_str(crate::vectors::COST).unwrap();
        for c in &v.line_cost {
            assert_eq!(line_cost(c.qty, c.cost_per_unit), c.expected, "{}", c.name);
        }
        for c in &v.recipe_cost {
            assert_eq!(recipe_cost(&c.lines), c.expected, "{}", c.name);
        }
        for c in &v.margin {
            assert_eq!(margin(c.price, c.cost), c.expected, "{}", c.name);
        }
        for c in &v.food_cost_band {
            assert_eq!(food_cost_band(c.cost, c.price), c.expected, "{}", c.name);
        }
    }

    #[test]
    fn decimal_not_binary_expansion() {
        // 0.35 is 0.34999999999999997779… in binary: × 10 would round to 3.
        assert_eq!(Decimal::from_f64(0.1), Some(Decimal::new(1, 1)));
        assert_eq!(line_cost(0.35, 10.0), 4);
        let binary = Decimal::from_f64_retain(0.35).unwrap() * Decimal::TEN;
        assert_eq!(round_piastres(binary), 3);
        // The f64 product falls below the midpoint too (the vector's `why`).
        assert_eq!((1.015f64 * 100.0).round(), 101.0);
        assert_eq!(line_cost(1.015, 100.0), 102);
    }

    #[test]
    fn non_finite_and_overflow_are_zero_not_a_panic() {
        assert_eq!(line_cost(f64::NAN, 1.0), 0);
        assert_eq!(line_cost(1e20, 1e20), 0);
        let big = CostLine {
            qty: 1e20,
            cost_per_unit: Some(1e20),
        };
        assert_eq!(recipe_cost(&[big]).piastres, 0);
    }
}
