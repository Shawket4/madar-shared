//! What one sale line comes to, before any reward, staff comp or discount.
//!
//! `(unit_price + Σ add-on price × add-on qty + Σ optional price) × quantity`:
//! the server's `ResolvedItem::charged_subtotal` (MadarRust
//! `orders/handlers.rs`) and the till's (madar-core `pricing.rs` / `cart.rs`)
//! are this one copy.
//!
//! Combos (bundle lines, whose fixed price plus per-component extras this
//! module also priced) were removed on 2026-09-25; a new combos module will
//! be designed from scratch.
//!
//! Pinned by `vectors/line_total_vectors.json`.

use serde::{Deserialize, Serialize};

use crate::tax::Minor;

/// A selected add-on: its CHARGED delta per unit (already resolved at
/// selection time — swap families clamp upstream) and how many.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Addon {
    pub price_modifier: Minor,
    pub quantity: Minor,
}

/// A sale line as both sides state it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineShape {
    pub quantity: Minor,
    /// The size-resolved unit price.
    pub unit_price: Minor,
    pub addons: Vec<Addon>,
    /// Optional-field prices.
    pub optionals: Vec<Minor>,
}

/// The extras on ONE unit: every add-on's delta × its quantity, plus every
/// optional's price. (The server's `charged_addon_per_unit +
/// optional_per_unit`.)
pub fn extras_per_unit(addons: &[Addon], optionals: &[Minor]) -> Minor {
    addons
        .iter()
        .map(|a| a.price_modifier * a.quantity)
        .sum::<Minor>()
        + optionals.iter().sum::<Minor>()
}

/// A line as charged, before any reward: the charged price of one unit ×
/// quantity. The server's `ResolvedItem::charged_subtotal`.
pub fn charged_subtotal(charged_per_unit: Minor, quantity: Minor) -> Minor {
    charged_per_unit * quantity
}

/// What `line` comes to, before any reward, staff comp or discount.
pub fn line_total(line: &LineShape) -> Minor {
    charged_subtotal(
        line.unit_price + extras_per_unit(&line.addons, &line.optionals),
        line.quantity,
    )
}

pub mod vectors {
    //! Vectors for [`line_total`](super::line_total).
    //!
    //! Generated from the rule as it moved here (the server's arithmetic;
    //! the till's was made identical by fix M3); the combo (bundle) cases
    //! left with combos on 2026-09-25. Regenerate deliberately:
    //! `MADAR_REGENERATE_LINE_VECTORS=1 cargo test -p madar-money line_total_vectors`.

    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use super::{line_total, Addon, LineShape, Minor};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct LineVector {
        pub name: String,
        pub line: LineShape,
        pub total: Minor,
    }

    pub fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/line_total_vectors.json")
    }

    fn case(name: &str, line: LineShape) -> LineVector {
        LineVector {
            name: name.to_string(),
            total: line_total(&line),
            line,
        }
    }

    pub fn generate() -> Vec<LineVector> {
        let mut out = Vec::new();
        // Normal lines: a grid over quantity, price, add-ons and optionals.
        let addon_sets: [&[Addon]; 5] = [
            &[],
            &[Addon {
                price_modifier: 500,
                quantity: 1,
            }],
            &[
                Addon {
                    price_modifier: 250,
                    quantity: 2,
                },
                Addon {
                    price_modifier: 1000,
                    quantity: 1,
                },
            ],
            // A swap priced as the base: charged at zero.
            &[Addon {
                price_modifier: 0,
                quantity: 3,
            }],
            // A replayed modifier priced below nothing (the server refuses the
            // line later; the total itself is still plain arithmetic).
            &[Addon {
                price_modifier: -700,
                quantity: 1,
            }],
        ];
        let optional_sets: [&[Minor]; 3] = [&[], &[300], &[150, 0, 45]];
        for &qty in &[0, 1, 2, 7] {
            for &unit in &[0, 1, 2500, 5000] {
                for (ai, addons) in addon_sets.iter().enumerate() {
                    for (oi, optionals) in optional_sets.iter().enumerate() {
                        out.push(case(
                            &format!("line q{qty} u{unit} a{ai} o{oi}"),
                            LineShape {
                                quantity: qty,
                                unit_price: unit,
                                addons: addons.to_vec(),
                                optionals: optionals.to_vec(),
                            },
                        ));
                    }
                }
            }
        }
        out
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn line_total_vectors() {
            let generated = generate();
            if std::env::var("MADAR_REGENERATE_LINE_VECTORS").is_ok() {
                std::fs::write(
                    fixture_path(),
                    serde_json::to_string_pretty(&generated).unwrap() + "\n",
                )
                .unwrap();
                return;
            }
            let expected: Vec<LineVector> =
                serde_json::from_str(crate::vectors::LINE_TOTAL).unwrap();
            assert_eq!(generated, expected, "line_total drifted from its vectors");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_normal_line_multiplies_every_extra_by_the_quantity() {
        let l = LineShape {
            quantity: 3,
            unit_price: 1500,
            addons: vec![
                Addon {
                    price_modifier: 500,
                    quantity: 1,
                },
                Addon {
                    price_modifier: 250,
                    quantity: 2,
                },
            ],
            optionals: vec![300],
        };
        assert_eq!(line_total(&l), (1500 + 500 + 500 + 300) * 3);
    }
}
