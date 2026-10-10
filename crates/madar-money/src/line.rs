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
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct Addon {
    pub price_modifier: Minor,
    pub quantity: Minor,
}

/// A sale line as both sides state it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
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

// The TypeScript name of `Minor`, which the declarations below refer to.
#[cfg(feature = "tsify")]
#[wasm_bindgen::prelude::wasm_bindgen(typescript_custom_section)]
const MINOR_TS: &str = "/** Whole piastres. */\nexport type Minor = number;";

/// One pick of a combo line, per ONE combo unit: madar-catalog's
/// `combo::PartQuote` figures the line is charged by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct PickShape {
    /// Units of the pick per combo unit.
    pub quantity: Minor,
    /// What one unit of it adds to the combo's price: its choice's surcharge
    /// plus a bigger size's extra (`PartQuote::surcharge_unit`).
    pub surcharge_unit: Minor,
    /// Its add-ons and optional fields, per unit (`PartQuote::extras_unit`);
    /// 0 for a pick without any.
    #[serde(default)]
    pub extras_unit: Minor,
}

/// A combo line: its price P per combo unit and its picks.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct ComboShape {
    pub quantity: Minor,
    /// P: the combo's own price.
    pub price: Minor,
    pub picks: Vec<PickShape>,
}

/// One line of a cart: a menu item (`{ "item": … }`) or a combo
/// (`{ "combo": … }`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(rename_all = "snake_case")]
pub enum CartLineShape {
    Item(LineShape),
    Combo(ComboShape),
}

/// What a combo's picks add to ONE combo unit: Σ pick quantity × (its
/// surcharge + its extras). The server's `ComboQuote::unit_total` less P.
pub fn combo_extras(picks: &[PickShape]) -> Minor {
    picks
        .iter()
        .map(|p| p.quantity * (p.surcharge_unit + p.extras_unit))
        .sum()
}

/// One unit of a cart line: an item's unit price and extras
/// ([`extras_per_unit`]), or a combo's P and [`combo_extras`]
/// (`ComboQuote::unit_total`).
pub fn cart_line_unit(line: &CartLineShape) -> Minor {
    match line {
        CartLineShape::Item(l) => l.unit_price + extras_per_unit(&l.addons, &l.optionals),
        CartLineShape::Combo(c) => c.price + combo_extras(&c.picks),
    }
}

/// What a cart line is charged: one unit × quantity ([`line_total`] for an
/// item, `ComboQuote::line_total` for a combo).
pub fn cart_line_total(line: &CartLineShape) -> Minor {
    let quantity = match line {
        CartLineShape::Item(l) => l.quantity,
        CartLineShape::Combo(c) => c.quantity,
    };
    charged_subtotal(cart_line_unit(line), quantity)
}

/// A cart's items total: Σ [`cart_line_total`], before any deal, discount,
/// tax or fee (the server's `CartQuote::items_total`).
pub fn cart_subtotal(lines: &[CartLineShape]) -> Minor {
    lines.iter().map(cart_line_total).sum()
}

#[cfg(test)]
mod cart_vectors {
    //! `cart_vectors.json` (hand-computed): [`cart_line_unit`],
    //! [`cart_line_total`], [`cart_subtotal`] and [`combo_extras`]. Never
    //! regenerated; a change is a deliberate edit with its working in `why`.

    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct Vectors {
        lines: Vec<LineCase>,
        carts: Vec<CartCase>,
        combo_extras: Vec<ExtrasCase>,
    }

    #[derive(Deserialize)]
    struct LineCase {
        name: String,
        line: CartLineShape,
        unit: Minor,
        total: Minor,
    }

    #[derive(Deserialize)]
    struct CartCase {
        name: String,
        lines: Vec<CartLineShape>,
        subtotal: Minor,
    }

    #[derive(Deserialize)]
    struct ExtrasCase {
        name: String,
        picks: Vec<PickShape>,
        expected: Minor,
    }

    #[test]
    fn cart_vectors() {
        let v: Vectors = serde_json::from_str(crate::vectors::CART).unwrap();
        assert!(v.lines.len() >= 10 && !v.carts.is_empty() && !v.combo_extras.is_empty());
        for c in &v.lines {
            assert_eq!(cart_line_unit(&c.line), c.unit, "{}: unit", c.name);
            assert_eq!(cart_line_total(&c.line), c.total, "{}: total", c.name);
        }
        for c in &v.carts {
            assert_eq!(cart_subtotal(&c.lines), c.subtotal, "{}", c.name);
        }
        for c in &v.combo_extras {
            assert_eq!(combo_extras(&c.picks), c.expected, "{}", c.name);
        }
    }

    #[test]
    fn an_item_line_is_line_total() {
        let lines: Vec<vectors::LineVector> =
            serde_json::from_str(crate::vectors::LINE_TOTAL).unwrap();
        for v in lines {
            let item = CartLineShape::Item(v.line.clone());
            assert_eq!(cart_line_total(&item), v.total, "{}", v.name);
        }
    }
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
