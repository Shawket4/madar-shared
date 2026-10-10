//! A recipe line's quantity: what it stores (in the ingredient's base unit,
//! grossed up by yield loss) and the usable amount it stands for.
//!
//! Pinned by `vectors/recipe_qty_vectors.json` (hand-computed).

use crate::{convert_with_density, round3, UnitError};

/// A yield percentage as a factor: `yield_pct / 100`; missing, zero or
/// negative is 1 (no loss).
fn yield_factor(yield_pct: Option<f64>) -> f64 {
    yield_pct
        .map(|y| y / 100.0)
        .filter(|y| *y > 0.0)
        .unwrap_or(1.0)
}

/// What a recipe line stores: `qty` typed in `unit`, converted to the
/// ingredient's `base_unit` ([`convert_with_density`]), grossed up by its yield
/// loss (÷ `yield_pct / 100`: 100 g usable at 80 % consumes 125 g bought),
/// rounded to 3 decimals.
///
/// MadarRust `recipes/handlers.rs` `normalize_recipe_unit`, float for float:
/// stored quantities (and the POS's waste values over them) depend on it. A
/// positive `qty` that comes to 0 is returned as 0; refusing it is the
/// server's job.
pub fn recipe_base_qty(
    qty: f64,
    unit: &str,
    base_unit: &str,
    density_g_per_ml: Option<f64>,
    yield_pct: Option<f64>,
) -> Result<f64, UnitError> {
    let base_q = convert_with_density(qty, unit, base_unit, density_g_per_ml)?;
    Ok(round3(base_q / yield_factor(yield_pct)))
}

/// The usable amount of a stored recipe quantity, in the base unit: `stored ×
/// yield_pct / 100`, rounded to 3 decimals. What the person typed, for an
/// editor to show and send back.
///
/// `usable_qty(recipe_base_qty(x, b, b, None, y), y) == x` when `x` is a whole
/// thousandth of the base unit and `y ≤ 100`: the stored 3 dp step, scaled
/// by the yield, stays under half a thousandth. It does not round-trip for a
/// yield above 100 % (1 g at 300 % stores 0.333, usable 0.999), nor for an
/// amount finer than a thousandth of the base unit (1.5 g into a kg
/// ingredient is 0.002 kg).
pub fn usable_qty(stored: f64, yield_pct: Option<f64>) -> f64 {
    round3(stored * yield_factor(yield_pct))
}

#[cfg(test)]
mod vectors {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct BaseCase {
        name: String,
        qty: f64,
        unit: String,
        base_unit: String,
        density: Option<f64>,
        yield_pct: Option<f64>,
        expected: Option<f64>,
        error: Option<String>,
    }

    #[derive(Deserialize)]
    struct UsableCase {
        name: String,
        stored: f64,
        yield_pct: Option<f64>,
        expected: f64,
    }

    #[derive(Deserialize)]
    struct RoundTrip {
        name: String,
        qty: f64,
        unit: String,
        base_unit: String,
        density: Option<f64>,
        yield_pct: Option<f64>,
        typed_in_base: f64,
        stored: f64,
        usable: f64,
        round_trips: bool,
    }

    #[derive(Deserialize)]
    struct Vectors {
        recipe_base_qty: Vec<BaseCase>,
        usable_qty: Vec<UsableCase>,
        round_trips: Vec<RoundTrip>,
    }

    #[test]
    fn recipe_qty_vectors() {
        let v: Vectors = serde_json::from_str(crate::vectors::RECIPE_QTY).unwrap();
        for c in &v.recipe_base_qty {
            let r = recipe_base_qty(c.qty, &c.unit, &c.base_unit, c.density, c.yield_pct);
            assert_eq!(r.as_ref().ok().copied(), c.expected, "{}", c.name);
            assert_eq!(r.err().map(|e| e.to_string()), c.error, "{}", c.name);
        }
        for c in &v.usable_qty {
            assert_eq!(usable_qty(c.stored, c.yield_pct), c.expected, "{}", c.name);
        }
        for c in &v.round_trips {
            let stored =
                recipe_base_qty(c.qty, &c.unit, &c.base_unit, c.density, c.yield_pct).unwrap();
            assert_eq!(stored, c.stored, "{}", c.name);
            let usable = usable_qty(stored, c.yield_pct);
            assert_eq!(usable, c.usable, "{}", c.name);
            assert_eq!(usable == c.typed_in_base, c.round_trips, "{}", c.name);
        }
    }

    /// The doc's claim: a whole thousandth of the base unit at a yield of
    /// 1–100 % always comes back as typed.
    #[test]
    fn round_trips_at_or_under_100_percent() {
        for y in 1..=100 {
            for m in (1..=200_000).step_by(7) {
                let x = m as f64 / 1000.0;
                let stored = recipe_base_qty(x, "g", "g", None, Some(f64::from(y))).unwrap();
                assert_eq!(usable_qty(stored, Some(f64::from(y))), x, "{x} at {y} %");
            }
        }
    }
}
