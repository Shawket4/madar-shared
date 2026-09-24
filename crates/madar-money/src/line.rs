//! What one sale line comes to, before any reward, staff comp or discount.
//!
//! The server prices a line as `charged_per_unit × quantity + component
//! surcharge` (MadarRust `orders/handlers.rs`, `ResolvedItem::charged_subtotal`,
//! with the surcharge summed per component as `(add-ons + optionals) ×
//! component quantity × line quantity`). The till priced it as `(unit price +
//! extras) × quantity` (madar-core `pricing.rs` / `cart.rs`). Since fix M3
//! (2026-09-23, the server's rule: a bundle component's extras are charged per
//! component unit) the two are the same arithmetic; this is its one copy.
//!
//! - A normal line: `(unit_price + Σ add-on price × add-on qty + Σ optional
//!   price) × quantity`.
//! - A bundle line: `unit_price` is the bundle's fixed price, which already
//!   covers the components; only each component's extras add money, per
//!   component unit and per bundle. The line's own add-ons and optionals are
//!   not charged.
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

/// One configured component inside a bundle line.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleComponent {
    /// Units of this component in ONE bundle (`bundle_components.quantity`).
    pub quantity: Minor,
    pub addons: Vec<Addon>,
    /// Optional-field prices (absolute; `0` is free).
    pub optionals: Vec<Minor>,
}

/// A sale line as both sides state it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineShape {
    pub quantity: Minor,
    /// The size-resolved unit price; for a bundle, its fixed price.
    pub unit_price: Minor,
    pub is_bundle: bool,
    pub addons: Vec<Addon>,
    /// Optional-field prices.
    pub optionals: Vec<Minor>,
    pub bundle_components: Vec<BundleComponent>,
}

/// The extras on ONE unit: every add-on's delta × its quantity, plus every
/// optional's price. (The server's `charged_addon_per_unit +
/// optional_per_unit`, or a component's `addon_line + optional_line`.)
pub fn extras_per_unit(addons: &[Addon], optionals: &[Minor]) -> Minor {
    addons
        .iter()
        .map(|a| a.price_modifier * a.quantity)
        .sum::<Minor>()
        + optionals.iter().sum::<Minor>()
}

/// What one bundle component adds to its line: its extras per component
/// unit × the component's quantity × the line's quantity. The server's
/// `component_surcharge +=` term.
pub fn component_surcharge(
    extras_per_component_unit: Minor,
    component_quantity: Minor,
    line_quantity: Minor,
) -> Minor {
    extras_per_component_unit * component_quantity * line_quantity
}

/// A line as charged, before any reward: the charged price of one unit ×
/// quantity, plus the bundle surcharge (`0` on a normal line). The server's
/// `ResolvedItem::charged_subtotal`.
pub fn charged_subtotal(charged_per_unit: Minor, quantity: Minor, surcharge: Minor) -> Minor {
    charged_per_unit * quantity + surcharge
}

/// What `line` comes to, before any reward, staff comp or discount.
pub fn line_total(line: &LineShape) -> Minor {
    if line.is_bundle {
        let surcharge = line
            .bundle_components
            .iter()
            .map(|c| {
                component_surcharge(
                    extras_per_unit(&c.addons, &c.optionals),
                    c.quantity,
                    line.quantity,
                )
            })
            .sum();
        charged_subtotal(line.unit_price, line.quantity, surcharge)
    } else {
        charged_subtotal(
            line.unit_price + extras_per_unit(&line.addons, &line.optionals),
            line.quantity,
            0,
        )
    }
}

pub mod vectors {
    //! Vectors for [`line_total`](super::line_total).
    //!
    //! Generated from the rule as it moved here (the server's arithmetic;
    //! the till's was made identical by fix M3). Regenerate deliberately:
    //! `MADAR_REGENERATE_LINE_VECTORS=1 cargo test -p madar-money line_total_vectors`.

    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use super::{line_total, Addon, BundleComponent, LineShape, Minor};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct LineVector {
        pub name: String,
        pub line: LineShape,
        pub total: Minor,
    }

    pub fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/line_total_vectors.json")
    }

    fn addon(price_modifier: Minor, quantity: Minor) -> Addon {
        Addon {
            price_modifier,
            quantity,
        }
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
                                is_bundle: false,
                                addons: addons.to_vec(),
                                optionals: optionals.to_vec(),
                                bundle_components: vec![],
                            },
                        ));
                    }
                }
            }
        }
        // Bundles: the fixed price plus component extras × component qty ×
        // bundle qty. The line's own add-ons/optionals are never charged.
        let components: [&[BundleComponent]; 4] = [
            &[],
            &[BundleComponent {
                quantity: 1,
                addons: vec![],
                optionals: vec![],
            }],
            &[BundleComponent {
                quantity: 2,
                addons: vec![Addon {
                    price_modifier: 500,
                    quantity: 1,
                }],
                optionals: vec![],
            }],
            &[
                BundleComponent {
                    quantity: 1,
                    addons: vec![Addon {
                        price_modifier: 200,
                        quantity: 1,
                    }],
                    optionals: vec![150],
                },
                BundleComponent {
                    quantity: 3,
                    addons: vec![Addon {
                        price_modifier: 250,
                        quantity: 2,
                    }],
                    optionals: vec![100],
                },
            ],
        ];
        for &qty in &[0, 1, 2, 3] {
            for &unit in &[4000, 5000, 12000] {
                for (ci, comps) in components.iter().enumerate() {
                    out.push(case(
                        &format!("bundle q{qty} u{unit} c{ci}"),
                        LineShape {
                            quantity: qty,
                            unit_price: unit,
                            is_bundle: true,
                            addons: vec![addon(9999, 5)],
                            optionals: vec![8888],
                            bundle_components: comps.to_vec(),
                        },
                    ));
                }
            }
        }
        // The discovery's M3 case: 5000 bundle, component qty 2, +500 oat milk.
        out.push(case(
            "M3: bundle 5000, component x2 with +500",
            LineShape {
                quantity: 1,
                unit_price: 5000,
                is_bundle: true,
                addons: vec![],
                optionals: vec![],
                bundle_components: vec![BundleComponent {
                    quantity: 2,
                    addons: vec![addon(500, 1)],
                    optionals: vec![],
                }],
            },
        ));
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

        #[test]
        fn the_m3_bundle_is_the_servers_6000() {
            let v = generate();
            let m3 = v.iter().find(|v| v.name.starts_with("M3")).unwrap();
            assert_eq!(m3.total, 6000);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bundle_ignores_its_own_addons_and_charges_components_per_unit() {
        let l = LineShape {
            quantity: 2,
            unit_price: 5000,
            is_bundle: true,
            addons: vec![Addon {
                price_modifier: 9999,
                quantity: 5,
            }],
            optionals: vec![8888],
            bundle_components: vec![BundleComponent {
                quantity: 2,
                addons: vec![Addon {
                    price_modifier: 500,
                    quantity: 1,
                }],
                optionals: vec![100],
            }],
        };
        // 5000 × 2 + (500 + 100) × 2 × 2
        assert_eq!(line_total(&l), 12_400);
    }

    #[test]
    fn a_normal_line_multiplies_every_extra_by_the_quantity() {
        let l = LineShape {
            quantity: 3,
            unit_price: 1500,
            is_bundle: false,
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
            bundle_components: vec![],
        };
        assert_eq!(line_total(&l), (1500 + 500 + 500 + 300) * 3);
    }
}
