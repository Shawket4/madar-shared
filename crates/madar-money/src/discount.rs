//! Who may put a discount on a sale, and how far: which capability a
//! discount act needs and the figures `madar_authz::decide` judges it on.
//!
//! Three capabilities, each with per-person caps (`madar_authz::Limits`):
//!   * `orders.discount.preset`         — a named preset; `max_percent` / `max_amount`
//!   * `orders.discount.manual_amount`  — an amount typed by hand; `max_amount`
//!   * `orders.discount.manual_percent` — a percentage typed by hand; `max_percent`
//!
//! Percent is in basis points (1250 = 12.5%), amount in minor units.
//!
//! [`ask_from`] and [`percent_bps_of`] moved here from MadarRust
//! `orders/discount_authz.rs` (the server judges a sale with them, live and at
//! replay). The till asked the same question with its own copy
//! (madar-core `discounts.rs`: `bps_of_rate`, `figures`, `discount_request`),
//! which rounded half-away-from-zero in `f64` where the server rounds a
//! `Decimal` to even (banker's): an exact-half basis point (0.00025) or a
//! fixed preset of 12.5 came out one apart. This module is the one copy, with
//! the SERVER's rounding, so the till now asks for a manager exactly when the
//! server would. (Owner question deferred: switch both to half-away-from-zero,
//! like `discounts/wire.rs` `legacy_value`?)
//!
//! Pinned by `vectors/discount_vectors.json`.

use madar_authz::{decide, Cap, Decision, EffectiveSet, Request};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

pub const KIND_PRESET: &str = "preset";
pub const KIND_MANUAL_AMOUNT: &str = "manual_amount";
pub const KIND_MANUAL_PERCENT: &str = "manual_percent";

/// The capability for a discount act of `kind`.
pub fn cap_for(kind: &str) -> Option<Cap> {
    match kind {
        KIND_PRESET => Some(Cap::OrdersDiscountPreset),
        KIND_MANUAL_AMOUNT => Some(Cap::OrdersDiscountManualAmount),
        KIND_MANUAL_PERCENT => Some(Cap::OrdersDiscountManualPercent),
        _ => None,
    }
}

/// A stored or sent percentage → basis points, `[0, 10_000]`. Accepts both
/// spellings: a fraction (`0.125`) and the legacy 0-100 integer (`12`).
/// Rounded like `Decimal::round` (half to even).
pub fn percent_bps_of(value: Decimal) -> i64 {
    let frac = if value > Decimal::ONE {
        value / Decimal::ONE_HUNDRED
    } else {
        value
    };
    (frac * Decimal::from(10_000))
        .round()
        .to_i64()
        .unwrap_or(0)
        .clamp(0, 10_000)
}

/// A rate that crossed an FFI or JSON as `f64` → basis points, through its
/// decimal string (so `0.145` is 0.145, not 0.14499999999999999), then
/// [`percent_bps_of`]. Not a number reads as zero.
pub fn bps_of_rate(rate: f64) -> i64 {
    percent_bps_of(decimal_of(rate))
}

/// `f64` → `Decimal` through the shortest decimal string; `0` when it is not
/// a finite number.
pub fn decimal_of(v: f64) -> Decimal {
    use core::str::FromStr;
    if !v.is_finite() {
        return Decimal::ZERO;
    }
    Decimal::from_str(&v.to_string()).unwrap_or_default()
}

/// A fixed discount's value → whole minor units, rounded like
/// `Decimal::round` (half to even). The server's reading of a preset's value.
pub fn fixed_minor(value: Decimal) -> i64 {
    value.round().to_i64().unwrap_or(0)
}

/// The discount act a sale asks for: which capability, and its figures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscountAsk {
    pub cap: Cap,
    pub kind: &'static str,
    /// Minor units taken off, when known.
    pub amount_minor: Option<i64>,
    /// Basis points, when the discount is a percentage.
    pub percent_bps: Option<i64>,
}

impl DiscountAsk {
    pub fn request(&self) -> Request {
        let mut r = Request::of(self.cap);
        r.amount = self.amount_minor;
        r.percent = self.percent_bps;
        r
    }

    pub fn decide(&self, eff: &EffectiveSet) -> Decision {
        decide(eff, &self.request())
    }
}

/// The discount figures of ANY sale-shaped request: a counter sale or a table
/// bill's settle. Once the act is expressed in one vocabulary, the counter
/// gate and the floor gate are the same code.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscountFields<'a> {
    /// Whether the sale names a preset (`discount_id`). The id itself is the
    /// caller's to look up.
    pub has_preset: bool,
    pub discount_type: Option<&'a str>,
    pub discount_value: Option<Decimal>,
    pub discount_amount: Option<i32>,
    pub discount_kind: Option<&'a str>,
    pub discount_percent_bps: Option<i32>,
}

/// What discount act `body` performs, if any. `preset` is the preset's
/// `(type, value)` as the caller looked it up (the server reads its
/// `discounts` row, active or not), else `None` and the sale's own type and
/// value are read. `None` when the sale carries no discount.
pub fn ask_from(body: &DiscountFields<'_>, preset: Option<(&str, Decimal)>) -> Option<DiscountAsk> {
    let (dtype, value) = match preset {
        Some((t, v)) => (Some(t), v),
        None => (
            body.discount_type,
            body.discount_value.unwrap_or(Decimal::ZERO),
        ),
    };
    let amount = body.discount_amount.filter(|a| *a > 0).map(i64::from);
    let is_percent = dtype == Some("percentage");
    let is_fixed = dtype == Some("fixed");
    let has_discount =
        body.has_preset || amount.is_some() || ((is_percent || is_fixed) && value > Decimal::ZERO);
    if !has_discount {
        return None;
    }
    let percent_bps = body
        .discount_percent_bps
        .map(i64::from)
        .or_else(|| is_percent.then(|| percent_bps_of(value)));
    let fixed_amount =
        || amount.or_else(|| Some(fixed_minor(value)).filter(|v| is_fixed && *v > 0));
    // An explicit kind wins; otherwise a preset id says preset, and an ad-hoc
    // discount is manual of its type (what an older client's ad-hoc one was).
    let kind = match body.discount_kind {
        Some(KIND_PRESET) => KIND_PRESET,
        Some(KIND_MANUAL_AMOUNT) => KIND_MANUAL_AMOUNT,
        Some(KIND_MANUAL_PERCENT) => KIND_MANUAL_PERCENT,
        _ if body.has_preset => KIND_PRESET,
        _ if is_percent => KIND_MANUAL_PERCENT,
        _ => KIND_MANUAL_AMOUNT,
    };
    Some(match kind {
        KIND_PRESET => DiscountAsk {
            cap: Cap::OrdersDiscountPreset,
            kind,
            amount_minor: if is_percent { amount } else { fixed_amount() },
            percent_bps: if is_percent { percent_bps } else { None },
        },
        KIND_MANUAL_PERCENT => DiscountAsk {
            cap: Cap::OrdersDiscountManualPercent,
            kind,
            amount_minor: None,
            percent_bps: percent_bps.or(Some(0)),
        },
        _ => DiscountAsk {
            cap: Cap::OrdersDiscountManualAmount,
            kind,
            amount_minor: fixed_amount().or(Some(0)),
            percent_bps: None,
        },
    })
}

/// The request `decide` answers for a discount of `kind` with these figures
/// (the till's `discount_request`): a manual amount is judged on its amount
/// (zero when unknown), a manual percentage on its basis points, a preset on
/// whichever figures it has.
pub fn request_for(
    kind: &str,
    amount_minor: Option<i64>,
    percent_bps: Option<i64>,
) -> Option<Request> {
    let cap = cap_for(kind)?;
    let mut r = Request::of(cap);
    match kind {
        KIND_MANUAL_AMOUNT => r.amount = Some(amount_minor.unwrap_or(0)),
        KIND_MANUAL_PERCENT => r.percent = Some(percent_bps.unwrap_or(0)),
        _ => {
            r.amount = amount_minor;
            r.percent = percent_bps;
        }
    }
    Some(r)
}

/// The figures a discount would have on a cart whose pre-discount subtotal is
/// `subtotal`: `(amount off, percent bps)`. `preset` is the preset's `(type,
/// value)`, a percentage as a fraction. Amounts are clamped to the cart; a
/// percentage's amount is rounded half-away-from-zero like the bill's own
/// discount line.
pub fn figures(
    kind: &str,
    preset: Option<(&str, Decimal)>,
    amount_minor: Option<i64>,
    percent_bps: Option<i64>,
    subtotal: i64,
) -> (Option<i64>, Option<i64>) {
    let pct_off = |bps: i64| {
        (Decimal::from(subtotal) * Decimal::from(bps) / Decimal::from(10_000))
            .round_dp_with_strategy(0, rust_decimal::RoundingStrategy::MidpointAwayFromZero)
            .to_i64()
            .unwrap_or(0)
    };
    match kind {
        KIND_MANUAL_AMOUNT => (
            Some(amount_minor.unwrap_or(0).clamp(0, subtotal.max(0))),
            None,
        ),
        KIND_MANUAL_PERCENT => {
            let bps = percent_bps.unwrap_or(0).clamp(0, 10_000);
            (Some(pct_off(bps)), Some(bps))
        }
        _ => match preset {
            Some(("percentage", rate)) => {
                let bps = percent_bps_of(rate);
                (Some(pct_off(bps)), Some(bps))
            }
            Some((_, value)) => (Some(fixed_minor(value).clamp(0, subtotal.max(0))), None),
            None => (None, None),
        },
    }
}

pub mod vectors {
    //! Vectors for the discount ask, generated from the server's rule as it
    //! moved here. Regenerate deliberately:
    //! `MADAR_REGENERATE_DISCOUNT_VECTORS=1 cargo test -p madar-money discount_vectors`.

    use std::path::PathBuf;
    use std::str::FromStr;

    use rust_decimal::Decimal;
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct BpsVector {
        pub value: String,
        pub bps: i64,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct VFields {
        pub has_preset: bool,
        pub discount_type: Option<String>,
        pub discount_value: Option<String>,
        pub discount_amount: Option<i32>,
        pub discount_kind: Option<String>,
        pub discount_percent_bps: Option<i32>,
    }

    impl VFields {
        pub fn fields(&self) -> DiscountFields<'_> {
            DiscountFields {
                has_preset: self.has_preset,
                discount_type: self.discount_type.as_deref(),
                discount_value: self
                    .discount_value
                    .as_deref()
                    .map(|v| Decimal::from_str(v).unwrap()),
                discount_amount: self.discount_amount,
                discount_kind: self.discount_kind.as_deref(),
                discount_percent_bps: self.discount_percent_bps,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct AskVector {
        pub fields: VFields,
        /// The preset row `(type, value)`, when the sale names a live one.
        pub preset: Option<(String, String)>,
        /// The capability key, `null` for no discount.
        pub cap: Option<String>,
        pub amount_minor: Option<i64>,
        pub percent_bps: Option<i64>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct FiguresVector {
        pub kind: String,
        pub preset: Option<(String, String)>,
        pub amount_minor: Option<i64>,
        pub percent_bps: Option<i64>,
        pub subtotal: i64,
        pub out_amount: Option<i64>,
        pub out_bps: Option<i64>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Vectors {
        pub bps: Vec<BpsVector>,
        pub asks: Vec<AskVector>,
        pub figures: Vec<FiguresVector>,
    }

    pub fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/discount_vectors.json")
    }

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    const VALUES: &[&str] = &[
        "0", "0.00005", "0.00015", "0.00025", "0.00035", "0.1", "0.125", "0.12345", "0.145",
        "0.33335", "0.5", "0.99995", "1", "1.5", "12", "12.5", "12.345", "100", "150", "-0.1",
    ];

    pub fn generate() -> Vectors {
        let bps = VALUES
            .iter()
            .map(|v| BpsVector {
                value: v.to_string(),
                bps: percent_bps_of(dec(v)),
            })
            .collect();

        let mut asks = Vec::new();
        let presets: [Option<(&str, &str)>; 5] = [
            None,
            Some(("percentage", "0.15")),
            Some(("percentage", "0.00025")),
            Some(("fixed", "500")),
            Some(("fixed", "12.5")),
        ];
        let types: [Option<&str>; 3] = [None, Some("percentage"), Some("fixed")];
        let values: [Option<&str>; 5] = [None, Some("0"), Some("0.125"), Some("12"), Some("12.5")];
        let amounts: [Option<i32>; 3] = [None, Some(0), Some(150)];
        let kinds: [Option<&str>; 5] = [
            None,
            Some(KIND_PRESET),
            Some(KIND_MANUAL_AMOUNT),
            Some(KIND_MANUAL_PERCENT),
            Some("bogus"),
        ];
        let sent_bps: [Option<i32>; 2] = [None, Some(1250)];
        for preset in presets {
            for has_preset in [false, true] {
                if preset.is_some() && !has_preset {
                    continue;
                }
                for dtype in types {
                    for value in values {
                        for amount in amounts {
                            for kind in kinds {
                                for pbps in sent_bps {
                                    let fields = VFields {
                                        has_preset,
                                        discount_type: dtype.map(str::to_string),
                                        discount_value: value.map(str::to_string),
                                        discount_amount: amount,
                                        discount_kind: kind.map(str::to_string),
                                        discount_percent_bps: pbps,
                                    };
                                    let ask = ask_from(
                                        &fields.fields(),
                                        preset.map(|(t, v)| (t, dec(v))),
                                    );
                                    asks.push(AskVector {
                                        preset: preset.map(|(t, v)| (t.to_string(), v.to_string())),
                                        cap: ask.as_ref().map(|a| a.cap.key().to_string()),
                                        amount_minor: ask.as_ref().and_then(|a| a.amount_minor),
                                        percent_bps: ask.as_ref().and_then(|a| a.percent_bps),
                                        fields,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut figures_v = Vec::new();
        for kind in [
            KIND_PRESET,
            KIND_MANUAL_AMOUNT,
            KIND_MANUAL_PERCENT,
            "bogus",
        ] {
            for preset in presets {
                for amount in [None, Some(0), Some(750), Some(5000), Some(-5)] {
                    for pbps in [None, Some(0), Some(1250), Some(2), Some(20_000), Some(-1)] {
                        for subtotal in [0, 1, 105, 2000, 2010, -50] {
                            let (a, b) = figures(
                                kind,
                                preset.map(|(t, v)| (t, dec(v))),
                                amount,
                                pbps,
                                subtotal,
                            );
                            figures_v.push(FiguresVector {
                                kind: kind.to_string(),
                                preset: preset.map(|(t, v)| (t.to_string(), v.to_string())),
                                amount_minor: amount,
                                percent_bps: pbps,
                                subtotal,
                                out_amount: a,
                                out_bps: b,
                            });
                        }
                    }
                }
            }
        }
        Vectors {
            bps,
            asks,
            figures: figures_v,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn discount_vectors() {
            let generated = generate();
            if std::env::var("MADAR_REGENERATE_DISCOUNT_VECTORS").is_ok() {
                std::fs::write(
                    fixture_path(),
                    serde_json::to_string_pretty(&generated).unwrap() + "\n",
                )
                .unwrap();
                return;
            }
            let expected: Vectors = serde_json::from_str(crate::vectors::DISCOUNT).unwrap();
            assert_eq!(generated.bps, expected.bps, "percent_bps_of drifted");
            assert_eq!(generated.asks.len(), expected.asks.len());
            for (g, e) in generated.asks.iter().zip(&expected.asks) {
                assert_eq!(g, e, "ask_from drifted from its vectors");
            }
            assert_eq!(generated.figures, expected.figures, "figures drifted");
        }

        #[test]
        fn the_exact_half_rounds_to_even_like_the_server() {
            let v = generate();
            let bps = |s: &str| v.bps.iter().find(|b| b.value == s).unwrap().bps;
            assert_eq!(bps("0.00025"), 2);
            assert_eq!(bps("0.00035"), 4);
            assert_eq!(bps("12"), 1200, "the legacy 0-100 spelling");
        }
    }
}

// Moved from MadarRust `orders/discount_authz.rs` with the rule.
#[cfg(test)]
mod tests {
    use super::*;
    use madar_authz::{CapSet, Limits};
    use rust_decimal_macros::dec;

    fn body() -> DiscountFields<'static> {
        DiscountFields::default()
    }

    #[test]
    fn no_discount_is_no_ask() {
        assert_eq!(ask_from(&body(), None), None);
    }

    #[test]
    fn a_preset_percentage_asks_for_its_percent_and_amount() {
        let mut b = body();
        b.has_preset = true;
        b.discount_amount = Some(150);
        let a = ask_from(&b, Some(("percentage", dec!(0.15)))).unwrap();
        assert_eq!(a.cap, Cap::OrdersDiscountPreset);
        assert_eq!(a.percent_bps, Some(1500));
        assert_eq!(a.amount_minor, Some(150));
    }

    #[test]
    fn an_ad_hoc_discount_is_manual_of_its_type_in_either_spelling() {
        let mut b = body();
        b.discount_type = Some("percentage");
        b.discount_value = Some(dec!(12));
        let a = ask_from(&b, None).unwrap();
        assert_eq!(
            (a.cap, a.percent_bps),
            (Cap::OrdersDiscountManualPercent, Some(1200))
        );

        let mut b = body();
        b.discount_type = Some("fixed");
        b.discount_value = Some(dec!(500));
        let a = ask_from(&b, None).unwrap();
        assert_eq!(
            (a.cap, a.amount_minor),
            (Cap::OrdersDiscountManualAmount, Some(500))
        );
    }

    #[test]
    fn over_the_cap_needs_a_manager() {
        let mut eff = EffectiveSet {
            caps: CapSet::from_keys(["orders.discount.manual_amount"]),
            ..Default::default()
        };
        eff.limits.insert(
            Cap::OrdersDiscountManualAmount.id(),
            Limits {
                max_amount: Some(1000),
                ..Default::default()
            },
        );
        let mut b = body();
        b.discount_kind = Some("manual_amount");
        b.discount_type = Some("fixed");
        b.discount_amount = Some(1000);
        assert_eq!(ask_from(&b, None).unwrap().decide(&eff), Decision::Allow);
        b.discount_amount = Some(1001);
        assert!(matches!(
            ask_from(&b, None).unwrap().decide(&eff),
            Decision::NeedsApproval(_)
        ));
    }

    #[test]
    fn a_rate_from_an_ffi_goes_through_its_decimal_string() {
        assert_eq!(bps_of_rate(0.145), 1450);
        assert_eq!(bps_of_rate(0.00025), 2);
        assert_eq!(bps_of_rate(f64::NAN), 0);
        assert_eq!(bps_of_rate(0.1234), 1234);
    }
}
