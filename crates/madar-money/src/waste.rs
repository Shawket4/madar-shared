//! What a waste is worth, and which waste inputs may be recorded at all.
//!
//! The value feeds the `max_value` approval ceiling of
//! `inventory.waste.record`, so the till's offline figure and the server's must
//! be the same number. [`value_of`] moved from MadarRust
//! `inventory/waste.rs` (madar-core `waste.rs` carried an identical copy).
//!
//! The input rules are the SERVER's (discovery T6): the till used to accept a
//! quantity the server refuses at replay — above 1 000 000, or one that
//! converts to nothing in the ingredient's unit (0.0004 g against a `kg`
//! ingredient) — and record it with a value of 0, under every ceiling.
//!
//! Pinned by `vectors/waste_vectors.json`.

/// A waste whose worth is not a real, non-negative amount of money, so it must
/// not be recorded at all. Stock is destroyed, never created: a negative
/// quantity or unit cost is bad data, and letting it through would also make
/// the total compare as under every `max_value` ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadValue;

/// `Σ qty × unit cost`, rounded once; partial when a line has no cost.
/// `lines` are `(qty in the ingredient's base unit, exact unit cost in
/// piastres)`: the exact cost, or sub-piastre costs (a ml of milk) would count
/// as 0.
pub fn value_of(lines: &[(f64, Option<f64>)]) -> Result<(Option<i64>, bool), BadValue> {
    let mut known: Vec<f64> = Vec::with_capacity(lines.len());
    for (q, c) in lines {
        if !q.is_finite() || *q < 0.0 {
            return Err(BadValue);
        }
        if let Some(c) = c {
            if !c.is_finite() || *c < 0.0 {
                return Err(BadValue);
            }
            known.push(q * c);
        }
    }
    let partial = known.len() < lines.len();
    if known.is_empty() {
        return Ok((None, partial));
    }
    let total = known.iter().sum::<f64>().round();
    if !total.is_finite() || total < 0.0 || total > i64::MAX as f64 {
        return Err(BadValue);
    }
    Ok((Some(total as i64), partial))
}

/// The most of anything one waste may record.
pub const MAX_QUANTITY: f64 = 1_000_000.0;

/// Why a waste input is refused before anything is looked up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasteInputError {
    /// Not a number, zero or less, above [`MAX_QUANTITY`], or — for an
    /// ingredient — nothing once converted to its unit. The server says
    /// "quantity must be greater than 0".
    Quantity,
    /// A menu item is wasted in whole units.
    WholeUnits,
    /// A menu item is wasted in `pcs`.
    MenuItemUnit,
}

/// The quantity as typed: a finite number above zero and at most
/// [`MAX_QUANTITY`].
pub fn check_quantity(quantity: f64) -> Result<(), WasteInputError> {
    if !quantity.is_finite() || quantity <= 0.0 || quantity > MAX_QUANTITY {
        return Err(WasteInputError::Quantity);
    }
    Ok(())
}

/// An ingredient's quantity once converted to its base unit (rounded to 3
/// decimals): nothing left is nothing to waste.
pub fn check_converted(base_quantity: f64) -> Result<(), WasteInputError> {
    if base_quantity <= 0.0 {
        return Err(WasteInputError::Quantity);
    }
    Ok(())
}

/// A menu item: in `pcs` (when a unit is named) and in whole units.
pub fn check_menu_item(unit: Option<&str>, quantity: f64) -> Result<(), WasteInputError> {
    if unit.is_some_and(|u| u != "pcs") {
        return Err(WasteInputError::MenuItemUnit);
    }
    if quantity.fract() != 0.0 {
        return Err(WasteInputError::WholeUnits);
    }
    Ok(())
}

pub mod vectors {
    //! Waste vectors. Regenerate deliberately:
    //! `MADAR_REGENERATE_WASTE_VECTORS=1 cargo test -p madar-money waste_vectors`.

    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct ValueVector {
        pub lines: Vec<(f64, Option<f64>)>,
        /// `null` with `bad: true` for a [`BadValue`].
        pub value_minor: Option<i64>,
        pub partial: bool,
        pub bad: bool,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct InputVector {
        pub subject_kind: String,
        pub quantity: f64,
        pub unit: Option<String>,
        /// For an ingredient: the quantity in its base unit after conversion.
        pub base_quantity: Option<f64>,
        /// `null` when accepted, else `quantity` / `whole_units` / `menu_item_unit`.
        pub refused: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct Vectors {
        pub values: Vec<ValueVector>,
        pub inputs: Vec<InputVector>,
    }

    pub fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/waste_vectors.json")
    }

    /// Accept or refuse one input the way the server's `plan_waste` does,
    /// before any lookup.
    pub fn refusal(
        kind: &str,
        quantity: f64,
        unit: Option<&str>,
        base_quantity: Option<f64>,
    ) -> Option<WasteInputError> {
        check_quantity(quantity).err().or_else(|| match kind {
            "ingredient" => base_quantity.and_then(|q| check_converted(q).err()),
            _ => check_menu_item(unit, quantity).err(),
        })
    }

    pub fn word(e: WasteInputError) -> &'static str {
        match e {
            WasteInputError::Quantity => "quantity",
            WasteInputError::WholeUnits => "whole_units",
            WasteInputError::MenuItemUnit => "menu_item_unit",
        }
    }

    pub fn generate() -> Vectors {
        let line_sets: Vec<Vec<(f64, Option<f64>)>> = vec![
            vec![],
            vec![(1.0, Some(100.0))],
            vec![(250.0, Some(0.35))],
            vec![(0.5, Some(1.0))],
            vec![(1.5, Some(1.0))],
            vec![(2.5, Some(1.0))],
            vec![(1.0, None)],
            vec![(1.0, None), (2.0, Some(10.0))],
            vec![(0.0, Some(100.0))],
            vec![(-1.0, Some(100.0))],
            vec![(1.0, Some(-0.01))],
            vec![(f64::INFINITY, Some(1.0))],
            vec![(1.0, Some(f64::NAN))],
            vec![(1e300, Some(1e300))],
            vec![
                (0.3333, Some(3.0)),
                (0.3333, Some(3.0)),
                (0.3334, Some(3.0)),
            ],
        ];
        let values = line_sets
            .into_iter()
            .map(|lines| {
                let r = value_of(&lines);
                ValueVector {
                    value_minor: r.ok().and_then(|(v, _)| v),
                    partial: r.map(|(_, p)| p).unwrap_or(false),
                    bad: r.is_err(),
                    // JSON has no NaN/inf: store them as null quantities.
                    lines: lines
                        .into_iter()
                        .map(|(q, c)| {
                            (
                                if q.is_finite() { q } else { f64::MAX },
                                c.map(|c| if c.is_finite() { c } else { f64::MAX }),
                            )
                        })
                        .collect(),
                }
            })
            .collect();
        let mut inputs = Vec::new();
        // Ingredient: (quantity typed, unit, what it is in the base unit).
        for &(q, unit, base) in &[
            (1.0, "kg", 1.0),
            (0.0004, "g", 0.0),   // against a kg ingredient: nothing left
            (0.0005, "g", 0.001), // half a gram rounds up to a milligram of kg
            (2_000_000.0, "g", 2000.0),
            (1_000_000.0, "g", 1000.0),
            (0.0, "g", 0.0),
            (-3.0, "g", -3.0),
            (250.0, "ml", 250.0),
        ] {
            inputs.push(InputVector {
                subject_kind: "ingredient".into(),
                quantity: q,
                unit: Some(unit.into()),
                base_quantity: Some(base),
                refused: refusal("ingredient", q, Some(unit), Some(base)).map(|e| word(e).into()),
            });
        }
        for &(q, unit) in &[
            (1.0, None),
            (2.0, Some("pcs")),
            (1.5, None),
            (1.0, Some("g")),
            (1_000_001.0, None),
            (0.0, Some("pcs")),
        ] {
            inputs.push(InputVector {
                subject_kind: "menu_item".into(),
                quantity: q,
                unit: unit.map(str::to_string),
                base_quantity: None,
                refused: refusal("menu_item", q, unit, None).map(|e| word(e).into()),
            });
        }
        Vectors { values, inputs }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn waste_vectors() {
            let generated = generate();
            if std::env::var("MADAR_REGENERATE_WASTE_VECTORS").is_ok() {
                std::fs::write(
                    fixture_path(),
                    serde_json::to_string_pretty(&generated).unwrap() + "\n",
                )
                .unwrap();
                return;
            }
            let expected: Vectors = serde_json::from_str(crate::vectors::WASTE).unwrap();
            assert_eq!(generated, expected, "waste value / input rules drifted");
        }

        #[test]
        fn t6_the_servers_limits() {
            assert_eq!(check_quantity(2_000_000.0), Err(WasteInputError::Quantity));
            assert_eq!(check_converted(0.0), Err(WasteInputError::Quantity));
            assert_eq!(check_quantity(1_000_000.0), Ok(()));
        }
    }
}
