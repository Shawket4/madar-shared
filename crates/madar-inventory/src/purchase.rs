//! Purchase-order money: a line's cost and unit cost, what one delivery cost,
//! the order dialog's estimate, and the unit cost a typed total implies.
//!
//! [`line_costs`] and [`delivery_cost`] are MadarRust
//! `purchasing/handlers.rs`'s, branch for branch and message for message, with
//! one fix (SHARED_RULES_PLAN.md D7): quantities round to 3 decimals HALF AWAY
//! FROM ZERO, as the `numeric(12,3)` column stores them. The backend's
//! `quantity_dec` rounded half to even, so an order of 0.0625 was costed on
//! 0.062 while the column held 0.063.
//!
//! Costs are piastres; a unit cost may be a fraction (8 dp). Quantities arrive
//! as `f64` and are read as the decimal they print as (`Decimal::from_f64`),
//! as Postgres reads a float into `numeric`.
//!
//! Pinned by `vectors/purchase_vectors.json` (hand-computed).

use core::fmt;

use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

/// Piastres per stock unit can be a fraction; a line total is whole piastres.
pub const LINE_COST_DP: u32 = 8;

/// A quantity as `numeric(12,3)` holds it: 3 decimals, half away from zero.
/// A non-finite quantity is 0.
pub fn quantity_dec(q: f64) -> Decimal {
    Decimal::from_f64(q)
        .unwrap_or(Decimal::ZERO)
        .round_dp_with_strategy(3, RoundingStrategy::MidpointAwayFromZero)
}

/// A quantity in whole thousandths, as `numeric(12,3)` stores it:
/// `quantity_dec(q) × 1000` (non-finite or beyond `i64` is 0). On the stored
/// grain it equals [`crate::milli`]; on a typed half thousandth that `f64`
/// cannot hold exactly (0.5005) this gives the column's 501 where
/// `crate::milli`'s float product gives 500. Transfers keep `crate::milli`.
pub fn milli(q: f64) -> i64 {
    quantity_dec(q)
        .checked_mul(Decimal::ONE_THOUSAND)
        .and_then(|m| m.to_i64())
        .unwrap_or(0)
}

/// Fractional piastres to whole piastres, half away from zero (MadarRust
/// `costing::service::round_piastres`, also `madar_money::cost`).
fn round_piastres(piastres: Decimal) -> i64 {
    piastres
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i64()
        .unwrap_or(0)
}

/// Why the server refuses a purchase line's cost (a 400).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PurchaseError {
    NegativeLineCost,
    /// A line total was given for a quantity that is 0 at 3 dp.
    QuantityNotPositive,
    NegativeUnitCost,
    /// Neither a line total nor a unit cost.
    NoCost,
}

impl fmt::Display for PurchaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PurchaseError::NegativeLineCost => "line_cost cannot be negative",
            PurchaseError::QuantityNotPositive => "quantity_ordered must be greater than 0",
            PurchaseError::NegativeUnitCost => "unit_cost cannot be negative",
            PurchaseError::NoCost => {
                "each line needs its cost: line_cost (the invoice total for the line)"
            }
        })
    }
}

impl std::error::Error for PurchaseError {}

/// A purchase line's `(line_cost, unit_cost_exact)` from what the client sent.
/// The invoice total is the truth: given, the unit cost is derived from it
/// exactly (8 dp); only an older client's per-unit price is multiplied out
/// instead (rounded to whole piastres).
pub fn line_costs(
    quantity_ordered: f64,
    line_cost: Option<i64>,
    unit_cost: Option<i64>,
) -> Result<(i64, Decimal), PurchaseError> {
    let qty = quantity_dec(quantity_ordered);
    match (line_cost, unit_cost) {
        (Some(lc), _) if lc < 0 => Err(PurchaseError::NegativeLineCost),
        (Some(lc), _) if qty > Decimal::ZERO => {
            Ok((lc, (Decimal::from(lc) / qty).round_dp(LINE_COST_DP)))
        }
        (Some(_), _) => Err(PurchaseError::QuantityNotPositive),
        (None, Some(uc)) if uc < 0 => Err(PurchaseError::NegativeUnitCost),
        (None, Some(uc)) => Ok((round_piastres(Decimal::from(uc) * qty), Decimal::from(uc))),
        (None, None) => Err(PurchaseError::NoCost),
    }
}

/// Piastres one delivery cost: the actual invoice total if given, else an
/// older client's actual per-unit price × the quantity, else the ORDERED line
/// total pro rata to the quantity received (so receiving all of it costs
/// exactly what was ordered). `quantity_ordered` is the stored column. The
/// result is not rounded.
pub fn delivery_cost(
    quantity_received: f64,
    line_cost: Option<i64>,
    unit_cost: Option<i64>,
    ordered_line_cost: i64,
    quantity_ordered: Decimal,
) -> Result<Decimal, PurchaseError> {
    let qty = quantity_dec(quantity_received);
    match (line_cost, unit_cost) {
        (Some(lc), _) if lc < 0 => Err(PurchaseError::NegativeLineCost),
        (Some(lc), _) => Ok(Decimal::from(lc)),
        (None, Some(uc)) if uc < 0 => Err(PurchaseError::NegativeUnitCost),
        (None, Some(uc)) => Ok(Decimal::from(uc) * qty),
        (None, None) if quantity_ordered > Decimal::ZERO => {
            Ok(Decimal::from(ordered_line_cost) * qty / quantity_ordered)
        }
        (None, None) => Ok(Decimal::ZERO),
    }
}

/// The order dialog's line estimate in piastres: `cost_per_stock_unit × qty`,
/// `qty` typed in `purchase_unit` and converted to `stock_unit` by the unit
/// factors, all exact and rounded once, half away from zero (a kg ingredient
/// at 50 000/kg, 1.5 g ordered: 75).
///
/// `None` when there is no cost, `qty` is not a number above 0, a unit is
/// unknown, or the units are of different families.
pub fn estimate_line_total(
    cost_per_stock_unit: Option<f64>,
    qty: f64,
    purchase_unit: &str,
    stock_unit: &str,
) -> Option<i64> {
    if !qty.is_finite() || qty <= 0.0 {
        return None;
    }
    let (pf, pk) = madar_units::unit_spec(purchase_unit)?;
    let (sf, sk) = madar_units::unit_spec(stock_unit)?;
    if pf != sf {
        return None;
    }
    let d = Decimal::from_f64;
    let total = d(cost_per_stock_unit?)?
        .checked_mul(d(qty)?)?
        .checked_mul(d(pk)?)?
        .checked_div(d(sk)?)?;
    Some(round_piastres(total))
}

/// The unit cost a typed line total implies: `line / quantity_dec(qty)` to 8
/// dp, as [`line_costs`] stores it. `None` unless `line ≥ 0` and the 3 dp
/// quantity is above 0.
pub fn unit_cost_from_total(line: i64, qty: f64) -> Option<f64> {
    line_costs(qty, Some(line), None).ok()?.1.to_f64()
}

#[cfg(test)]
mod vectors {
    use core::str::FromStr;

    use serde::Deserialize;

    use super::*;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[derive(Deserialize)]
    struct QtyCase {
        name: String,
        q: f64,
        quantity_dec: String,
        milli: i64,
    }

    #[derive(Deserialize)]
    struct LineCase {
        name: String,
        quantity_ordered: f64,
        line_cost: Option<i64>,
        unit_cost: Option<i64>,
        expected_line_cost: Option<i64>,
        expected_unit_cost: Option<String>,
        error: Option<String>,
    }

    #[derive(Deserialize)]
    struct DeliveryCase {
        name: String,
        quantity_received: f64,
        line_cost: Option<i64>,
        unit_cost: Option<i64>,
        ordered_line_cost: i64,
        quantity_ordered: String,
        expected: Option<String>,
        error: Option<String>,
    }

    #[derive(Deserialize)]
    struct EstimateCase {
        name: String,
        cost_per_stock_unit: Option<f64>,
        qty: f64,
        purchase_unit: String,
        stock_unit: String,
        expected: Option<i64>,
    }

    #[derive(Deserialize)]
    struct UnitCostCase {
        name: String,
        line: i64,
        qty: f64,
        expected: Option<f64>,
    }

    #[derive(Deserialize)]
    struct Vectors {
        quantity: Vec<QtyCase>,
        line_costs: Vec<LineCase>,
        delivery_cost: Vec<DeliveryCase>,
        estimate_line_total: Vec<EstimateCase>,
        unit_cost_from_total: Vec<UnitCostCase>,
    }

    #[test]
    fn purchase_vectors() {
        let v: Vectors = serde_json::from_str(crate::vectors::PURCHASE).unwrap();
        for c in &v.quantity {
            assert_eq!(quantity_dec(c.q), dec(&c.quantity_dec), "{}", c.name);
            assert_eq!(milli(c.q), c.milli, "{}", c.name);
        }
        for c in &v.line_costs {
            let r = line_costs(c.quantity_ordered, c.line_cost, c.unit_cost);
            assert_eq!(r.ok().map(|(l, _)| l), c.expected_line_cost, "{}", c.name);
            assert_eq!(
                r.ok().map(|(_, u)| u),
                c.expected_unit_cost.as_deref().map(dec),
                "{}",
                c.name
            );
            assert_eq!(r.err().map(|e| e.to_string()), c.error, "{}", c.name);
        }
        for c in &v.delivery_cost {
            let r = delivery_cost(
                c.quantity_received,
                c.line_cost,
                c.unit_cost,
                c.ordered_line_cost,
                dec(&c.quantity_ordered),
            );
            assert_eq!(r.ok(), c.expected.as_deref().map(dec), "{}", c.name);
            assert_eq!(r.err().map(|e| e.to_string()), c.error, "{}", c.name);
        }
        for c in &v.estimate_line_total {
            let r = estimate_line_total(
                c.cost_per_stock_unit,
                c.qty,
                &c.purchase_unit,
                &c.stock_unit,
            );
            assert_eq!(r, c.expected, "{}", c.name);
        }
        for c in &v.unit_cost_from_total {
            assert_eq!(
                unit_cost_from_total(c.line, c.qty),
                c.expected,
                "{}",
                c.name
            );
        }
    }

    #[test]
    fn non_finite_inputs() {
        assert_eq!(quantity_dec(f64::NAN), Decimal::ZERO);
        assert_eq!(estimate_line_total(Some(1.0), f64::NAN, "g", "g"), None);
        assert_eq!(
            estimate_line_total(Some(1.0), f64::INFINITY, "g", "g"),
            None
        );
        assert_eq!(estimate_line_total(Some(f64::NAN), 1.0, "g", "g"), None);
    }

    /// On the stored grain (whole thousandths) both millis agree; they part
    /// only on a typed half thousandth the f64 product lands under.
    #[test]
    fn milli_matches_the_crates_on_the_stored_grain() {
        for n in 0..200_000i64 {
            let q = n as f64 / 1000.0;
            assert_eq!(milli(q), crate::milli(q), "{q}");
            assert_eq!(milli(-q), crate::milli(-q), "{q}");
        }
        assert_eq!((milli(0.5005), crate::milli(0.5005)), (501, 500));
    }
}
