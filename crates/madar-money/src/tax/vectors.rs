//! The tax engine's vectors: every bill worth pinning, and what it must come to.
//!
//! These used to be the contract between two copies of the engine (the
//! backend's and the till's), committed to both repos by hand. There is one
//! engine now, but the vectors stay: old tablets in the field run the copy
//! they shipped with, and `tax_vectors.json` is what that copy was pinned to.
//! A change that alters any of these figures is a change to what an old till
//! and the server disagree on, and must be deliberate.
//!
//! To change the maths deliberately: change it in `tax`, run
//! `MADAR_REGENERATE_TAX_VECTORS=1 cargo test -p madar-money tax::vectors`, and
//! release a tag.

use std::path::PathBuf;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::{compute, discount_amount, Discount, SaleChannel, TaxPolicy};

/// One priced bill: the inputs, and every figure they must produce.
///
/// The discount is stated the way the policy states it — a kind and a value —
/// rather than as the amount it comes to. The amount is an OUTPUT, because
/// deriving it is a rounding point, and a fixture that carried it ready-made
/// let the two engines derive it differently (one in `f64`, one in `Decimal`)
/// while both conformance tests stayed green.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Vector {
    pub subtotal: i64,
    /// `"none"`, `"percentage"` or `"fixed"` — the `discount_type` column's words.
    pub discount_kind: String,
    /// A fraction for `percentage`, minor units for `fixed`, `"0"` for none.
    pub discount_value: String,
    pub tax_rate: String,
    pub tax_inclusive: bool,
    pub service_charge_rate: String,
    pub service_charge_taxable: bool,
    /// Where the sale happened — `"dine_in"`, `"takeaway"`, `"delivery"` or
    /// `"online"`. Only dine-in carries a service charge (owner ruling 2), and
    /// that rule is part of the maths both engines must agree on.
    pub channel: String,
    /// Someone holding `orders:waive_service` removed the service charge.
    pub service_waived: bool,
    // Expected:
    pub discount: i64,
    pub service_charge: i64,
    pub tax: i64,
    pub total: i64,
    pub net: i64,
}

/// The fixture's words for a discount, as the engine's type. Mirrored in the
/// till's conformance test; an unknown kind is a fixture bug, not a bill.
pub fn discount_from_wire(kind: &str, value: &str) -> Discount {
    let value = value
        .parse::<Decimal>()
        .unwrap_or_else(|e| panic!("discount_value {value:?} is not a decimal: {e}"));
    match kind {
        "none" => Discount::None,
        "percentage" => Discount::Percentage(value),
        "fixed" => Discount::Fixed(value),
        other => panic!("unknown discount_kind {other:?} in tax_vectors.json"),
    }
}

/// The fixture's word for a channel. An unknown word is a fixture bug.
pub fn channel_from_wire(word: &str) -> SaleChannel {
    SaleChannel::from_wire(word)
        .unwrap_or_else(|| panic!("unknown channel {word:?} in tax_vectors.json"))
}

pub fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/tax_vectors.json")
}

/// Every combination worth pinning: both modes, both service-charge
/// treatments, the rates a shop plausibly sets (including 14.5%, where f64 and
/// decimal rounding part company), and bills that exercise the rounding
/// boundaries rather than only round numbers — with discounts stated as the
/// policy states them, so the derivation is pinned as well as the tax on it.
pub fn generate() -> Vec<Vector> {
    let rates = ["0", "0.05", "0.10", "0.14", "0.145", "0.20", "0.255", "1"];
    let charges = ["0", "0.10", "0.125"];
    let bills: &[(i64, &str, &str)] = &[
        // Plain bills.
        (0, "none", "0"),
        (1, "none", "0"),
        (7, "none", "0"),
        (100, "none", "0"),
        (333, "none", "0"),
        (999, "none", "0"),
        (1000, "none", "0"),
        (1500, "none", "0"),
        (4999, "none", "0"),
        (5000, "none", "0"),
        (5700, "none", "0"),
        (12_345, "none", "0"),
        (99_999, "none", "0"),
        (1_000_000, "none", "0"),
        // Fixed amounts off, including ones that swallow the bill, and one
        // with a fraction of a piastre — the column is NUMERIC, so it can arrive.
        (5000, "fixed", "1"),
        (5000, "fixed", "500"),
        (5000, "fixed", "250.5"),
        (5000, "fixed", "4999"),
        (5000, "fixed", "5000"),
        (5000, "fixed", "99999"),
        (1, "fixed", "1"),
        // Percentages. Every one of these lands on or near a half-piastre, which
        // is where a derivation in binary floating point parts company with one
        // in decimal — 100 at 14.5% is the case that actually bit.
        (100, "percentage", "0.145"),
        (5, "percentage", "0.10"),
        (25, "percentage", "0.10"),
        (105, "percentage", "0.10"),
        (1000, "percentage", "0.125"),
        (333, "percentage", "0.333"),
        (12_345, "percentage", "0.075"),
        (5000, "percentage", "0.145"),
        (1_000_000, "percentage", "0.145"),
        // An inclusive shop's gross with a discount on it: 5700 is 5000 at 14%.
        (5700, "percentage", "0.10"),
        // Discounts that swallow the bill, or would take more than it.
        (1, "percentage", "0.5"),
        (1, "percentage", "0.145"),
        (0, "percentage", "0.10"),
        (5000, "percentage", "1"),
        (5000, "percentage", "1.5"),
    ];

    let mut out = Vec::new();
    for r in rates {
        for c in charges {
            for &taxable in &[true, false] {
                for &inclusive in &[true, false] {
                    for &(subtotal, kind, value) in bills {
                        let policy = TaxPolicy {
                            tax_rate: r.parse::<Decimal>().unwrap(),
                            tax_inclusive: inclusive,
                            service_charge_rate: c.parse::<Decimal>().unwrap(),
                            service_charge_taxable: taxable,
                        };
                        out.push(priced(
                            subtotal,
                            kind,
                            value,
                            policy,
                            SaleChannel::DineIn,
                            false,
                        ));
                    }
                }
            }
        }
    }

    // The channel rule and the waiver. Every channel, waived and not, under the
    // charges and both tax treatments, over bills with and without a discount —
    // a takeaway priced with a service charge is the refused sale this pins.
    let channel_bills: &[(i64, &str, &str)] = &[
        (5000, "none", "0"),
        (5700, "percentage", "0.10"),
        (100, "percentage", "0.145"),
        (12_345, "fixed", "500"),
        (1, "none", "0"),
    ];
    let channels = [
        SaleChannel::DineIn,
        SaleChannel::Takeaway,
        SaleChannel::Delivery,
        SaleChannel::Online,
    ];
    for channel in channels {
        for &waived in &[false, true] {
            for r in ["0", "0.14"] {
                for c in ["0.10", "0.125"] {
                    for &taxable in &[true, false] {
                        for &inclusive in &[true, false] {
                            for &(subtotal, kind, value) in channel_bills {
                                let policy = TaxPolicy {
                                    tax_rate: r.parse::<Decimal>().unwrap(),
                                    tax_inclusive: inclusive,
                                    service_charge_rate: c.parse::<Decimal>().unwrap(),
                                    service_charge_taxable: taxable,
                                };
                                out.push(priced(subtotal, kind, value, policy, channel, waived));
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

/// One vector: the branch's policy as configured, priced for `channel`.
fn priced(
    subtotal: i64,
    kind: &str,
    value: &str,
    policy: TaxPolicy,
    channel: SaleChannel,
    service_waived: bool,
) -> Vector {
    let discount = discount_amount(subtotal, discount_from_wire(kind, value));
    let b = compute(
        subtotal,
        discount,
        &policy.for_sale(channel, service_waived),
    );
    Vector {
        subtotal,
        discount_kind: kind.to_string(),
        discount_value: value.to_string(),
        tax_rate: policy.tax_rate.to_string(),
        tax_inclusive: policy.tax_inclusive,
        service_charge_rate: policy.service_charge_rate.to_string(),
        service_charge_taxable: policy.service_charge_taxable,
        channel: channel.as_str().to_string(),
        service_waived,
        discount: b.discount,
        service_charge: b.service_charge,
        tax: b.tax,
        total: b.total,
        net: b.net,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_engine_still_agrees_with_the_shared_vectors() {
        let generated = generate();

        // Deliberate change to the maths: regenerate, and release a tag.
        if std::env::var("MADAR_REGENERATE_TAX_VECTORS").is_ok() {
            std::fs::write(
                fixture_path(),
                serde_json::to_string_pretty(&generated).unwrap() + "\n",
            )
            .unwrap();
            eprintln!(
                "regenerated {} vectors at {}",
                generated.len(),
                fixture_path().display()
            );
            return;
        }

        let raw = std::fs::read_to_string(fixture_path()).expect(
            "tax_vectors.json is missing — regenerate with \
             MADAR_REGENERATE_TAX_VECTORS=1 cargo test -p madar-money tax::vectors",
        );
        let expected: Vec<Vector> = serde_json::from_str(&raw).unwrap();

        assert_eq!(
            generated.len(),
            expected.len(),
            "the vector set itself changed; regenerate deliberately"
        );
        let mut drift = Vec::new();
        for (got, want) in generated.iter().zip(expected.iter()) {
            if got != want {
                drift.push(format!("  got {got:?}\n  want {want:?}"));
            }
        }
        assert!(
            drift.is_empty(),
            "the tax engine no longer matches the shared vectors — the till \
             computes these bills differently and the server would reject its \
             orders:\n{}",
            drift.join("\n")
        );
    }
}

#[cfg(test)]
/// Moved from madar-core `tax::conformance`: the till's side of the same
/// contract, reading the file rather than regenerating it.
mod conformance {
    use super::super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Vector {
        subtotal: i64,
        discount_kind: String,
        discount_value: String,
        tax_rate: String,
        tax_inclusive: bool,
        service_charge_rate: String,
        service_charge_taxable: bool,
        channel: String,
        service_waived: bool,
        discount: i64,
        service_charge: i64,
        tax: i64,
        total: i64,
        net: i64,
    }

    /// The fixture's words for a discount, as the engine's type. Mirrors
    /// `tax::vectors::discount_from_wire` on the server; an unknown kind is a
    /// fixture bug, not a bill.
    fn discount_from_wire(kind: &str, value: &str) -> Discount {
        let value = value
            .parse::<Decimal>()
            .unwrap_or_else(|e| panic!("discount_value {value:?} is not a decimal: {e}"));
        match kind {
            "none" => Discount::None,
            "percentage" => Discount::Percentage(value),
            "fixed" => Discount::Fixed(value),
            other => panic!("unknown discount_kind {other:?} in tax_vectors.json"),
        }
    }

    #[test]
    fn this_till_prices_every_shared_vector_exactly_as_the_server_does() {
        let raw = crate::vectors::TAX;
        let vectors: Vec<Vector> = serde_json::from_str(raw).expect("tax_vectors.json parses");
        assert!(
            vectors.len() > 1000,
            "the shared fixture looks truncated ({} vectors)",
            vectors.len()
        );
        // The fixture must actually exercise the derivation, or this test is
        // back to trusting a ready-made amount.
        assert!(
            vectors
                .iter()
                .any(|v| v.discount_kind == "percentage" && v.discount_value == "0.145"),
            "the shared fixture no longer carries the 14.5% discount that bit"
        );
        // And the channel rule: a takeaway with a service charge configured.
        assert!(
            vectors.iter().any(|v| v.channel == "takeaway"
                && v.service_charge_rate != "0"
                && v.service_charge == 0),
            "the shared fixture no longer pins the dine-in-only service charge"
        );

        let mut drift = Vec::new();
        for v in &vectors {
            let policy = TaxPolicy {
                tax_rate: v.tax_rate.parse().unwrap(),
                tax_inclusive: v.tax_inclusive,
                service_charge_rate: v.service_charge_rate.parse().unwrap(),
                service_charge_taxable: v.service_charge_taxable,
            };
            let discount = discount_amount(
                v.subtotal,
                discount_from_wire(&v.discount_kind, &v.discount_value),
            );
            let channel = SaleChannel::from_wire(&v.channel)
                .unwrap_or_else(|| panic!("unknown channel {:?} in tax_vectors.json", v.channel));
            let got = compute(
                v.subtotal,
                discount,
                &policy.for_sale(channel, v.service_waived),
            );
            if got.discount != v.discount
                || got.service_charge != v.service_charge
                || got.tax != v.tax
                || got.total != v.total
                || got.net != v.net
            {
                drift.push(format!(
                    "  subtotal {} discount {} {} rate {} incl {} sc {} sc_taxable {} channel {} waived {}\n    \
                     got  discount={} sc={} tax={} total={} net={}\n    want discount={} sc={} tax={} total={} net={}",
                    v.subtotal,
                    v.discount_kind,
                    v.discount_value,
                    v.tax_rate,
                    v.tax_inclusive,
                    v.service_charge_rate,
                    v.service_charge_taxable,
                    v.channel,
                    v.service_waived,
                    got.discount,
                    got.service_charge,
                    got.tax,
                    got.total,
                    got.net,
                    v.discount,
                    v.service_charge,
                    v.tax,
                    v.total,
                    v.net
                ));
                if drift.len() >= 10 {
                    break;
                }
            }
        }
        assert!(
            drift.is_empty(),
            "this till would price {} of {} shared bills differently from the server, \
             and the server refuses orders it disagrees with:\n{}",
            drift.len(),
            vectors.len(),
            drift.join("\n")
        );
    }
}
