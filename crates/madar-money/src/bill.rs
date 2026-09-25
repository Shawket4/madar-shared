//! Bill assembly: from priced lines to what the customer pays, and the
//! tender that settles it.
//!
//! The order of operations is the server's (MadarRust `create_order_inner`),
//! which the till mirrors (madar-core `pricing::price_cart`):
//!
//! 1. per line, the STAFF COMP comes off first, then a REWARD covers whole
//!    units of what is left ([`net_line`]); no line may end below zero;
//! 2. the subtotal is the sum of the net lines;
//! 3. the DISCOUNT is resolved against that reduced subtotal — a rule
//!    (percentage / fixed) or an amount a person stated — and clamped to
//!    `[0, subtotal]` ([`discount_on`]);
//! 4. the service charge and tax are the shared engine's
//!    ([`crate::tax::compute`]), under the sale's channel policy.
//!
//! [`price_bill_on`] is the same assembly over a subtotal the till STATED
//! (the server records a till's word on what was sold unless it priced the
//! bill itself); [`price_subtotal`] is its last two steps, for a caller that
//! already holds the subtotal; [`rule_of`] reads a stored discount rule.
//!
//! [`price_open_bill`] is a table bill's preview (the server's
//! `price_bill_under`, which the till's `reprice_with` re-runs after a void or
//! a discount). The tender helpers are the server's split/change rules and the
//! till's change-due rule.
//!
//! Pinned by `vectors/bill_vectors.json` (bills, open bills, tenders).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::loyalty::covered_minor;
use crate::tax::{
    compute, discount_amount, negative_part, Breakdown, Discount, Minor, NegativePart, TaxPolicy,
};

/// One line of a bill as priced (see [`crate::line`]), with what may come off
/// it before the discount.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BillLine {
    /// The line as charged, before any reward ([`crate::line::line_total`]).
    pub charged: Minor,
    /// What ONE unit is charged, modifiers included (the reward covers whole
    /// units at this price).
    pub per_unit: Minor,
    /// Units a loyalty reward covers.
    pub reward_units: Minor,
    /// What the staff pool comps on the WHOLE line, as the caller settled it.
    pub staff_comp: Minor,
}

/// A line after the staff comp and the reward.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetLine {
    pub staff_comp: Minor,
    /// What the reward took off (never more than the line after the comp).
    pub covered: Minor,
    /// What the line adds to the subtotal. Negative only for a line priced
    /// below nothing, which [`refusal`] refuses.
    pub net: Minor,
}

/// The staff comp comes off first, then the reward covers whole units of what
/// is left, never more than that. The server's per-line order.
pub fn net_line(l: &BillLine) -> NetLine {
    let covered = covered_minor(l.per_unit, l.charged, l.reward_units);
    let covered = covered.min((l.charged - l.staff_comp).max(0));
    NetLine {
        staff_comp: l.staff_comp,
        covered,
        net: l.charged - l.staff_comp - covered,
    }
}

/// How the bill is discounted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BillDiscount {
    /// A rule the engine applies to the (reduced) subtotal.
    Rule(Discount),
    /// An amount a person stated (a manager-approved figure no rule
    /// expresses). Clamped to the bill.
    Stated(Minor),
}

/// The amount `discount` takes off `subtotal`, clamped to `[0, subtotal]`.
pub fn discount_on(subtotal: Minor, discount: BillDiscount) -> Minor {
    match discount {
        BillDiscount::Rule(d) => discount_amount(subtotal, d),
        BillDiscount::Stated(a) => a.clamp(0, subtotal.max(0)),
    }
}

/// A stored discount rule as the schema spells it (`discount_type`,
/// `discount_value`): `percentage` is a FRACTION (`0.10` is 10% off), `fixed`
/// an amount in minor units; any other type, or none, is no discount. The
/// server's `discounts::calc_discount` and its table-bill preview, and the
/// till's bill preview, all read a rule this way.
pub fn rule_of(discount_type: Option<&str>, value: Decimal) -> Discount {
    match discount_type {
        Some("percentage") => Discount::Percentage(value),
        Some("fixed") => Discount::Fixed(value),
        _ => Discount::None,
    }
}

/// Price a subtotal: the discount resolved against it ([`discount_on`]), then
/// the service charge and tax. The last two steps of [`price_bill`], for a
/// caller that already holds the subtotal (a delivery order, the server's
/// expected bill).
pub fn price_subtotal(subtotal: Minor, discount: BillDiscount, policy: &TaxPolicy) -> Breakdown {
    compute(subtotal, discount_on(subtotal, discount), policy)
}

/// A priced bill.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bill {
    pub lines: Vec<NetLine>,
    /// What rewards took off the lines.
    pub reward_covered: Minor,
    /// What the staff pool took off the lines.
    pub staff_comp: Minor,
    /// The engine's figures over the net subtotal (`breakdown.subtotal`).
    pub breakdown: Breakdown,
}

/// Assemble a bill: net lines → subtotal → discount → service charge and tax.
/// Never refuses; [`refusal`] says whether the result may be booked.
pub fn price_bill(lines: &[BillLine], discount: BillDiscount, policy: &TaxPolicy) -> Bill {
    price_bill_on(lines, None, discount, policy)
}

/// [`price_bill`], over the subtotal a till STATED when it is given one.
///
/// The server records what a till says was sold (line prices are the till's
/// word; a sale that happened offline was charged at the till's figures), so
/// on a bill the server did not price itself — no reward, no staff comp priced
/// here — the subtotal is the till's and only the arithmetic on it is the
/// server's. The lines are still netted, so a line below zero is still
/// [`refusal`]'s to name; a stated subtotal below zero is refused as the
/// bill's `subtotal` part.
pub fn price_bill_on(
    lines: &[BillLine],
    stated_subtotal: Option<Minor>,
    discount: BillDiscount,
    policy: &TaxPolicy,
) -> Bill {
    let nets: Vec<NetLine> = lines.iter().map(net_line).collect();
    let subtotal: Minor = stated_subtotal.unwrap_or_else(|| nets.iter().map(|n| n.net).sum());
    Bill {
        reward_covered: nets.iter().map(|n| n.covered).sum(),
        staff_comp: nets.iter().map(|n| n.staff_comp).sum(),
        lines: nets,
        breakdown: price_subtotal(subtotal, discount, policy),
    }
}

/// Why a priced bill may not be booked: a line below zero (first one, by
/// index), else the first negative figure of the bill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BillRefusal {
    Line { index: usize, net: Minor },
    Part(NegativePart),
}

pub fn refusal(bill: &Bill) -> Option<BillRefusal> {
    if let Some((index, l)) = bill.lines.iter().enumerate().find(|(_, l)| l.net < 0) {
        return Some(BillRefusal::Line { index, net: l.net });
    }
    negative_part(&bill.breakdown).map(BillRefusal::Part)
}

/// A table bill's preview (the server's `price_bill_under`): a negative
/// subtotal (a void that took more off than the rounds put on) is floored to
/// zero — the settle refuses it with its own message — then the discount rule
/// is resolved against it and the engine prices the rest.
pub fn price_open_bill(subtotal: Minor, discount: Discount, policy: &TaxPolicy) -> Breakdown {
    price_subtotal(subtotal.max(0), BillDiscount::Rule(discount), policy)
}

// ── Tender ──────────────────────────────────────────────────────────────────

/// The till's change ceiling (`clamp(0, 999_999)`).
pub const CHANGE_CAP: Minor = 999_999;

/// The change a cash sale hands back: what was tendered, less the bill, less
/// the cash part of a tip (so the change the teller sees is the change that is
/// recorded), never below zero nor above [`CHANGE_CAP`]. No tender, no change.
pub fn change_due(tendered: Option<Minor>, total: Minor, cash_tip: Minor) -> Minor {
    match tendered {
        None => 0,
        Some(t) => (t - total - cash_tip).clamp(0, CHANGE_CAP),
    }
}

/// One leg of a split payment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leg {
    pub amount: Minor,
    pub is_cash: bool,
}

/// The first leg below zero, if any. A leg of -500 beside one of +500 would
/// otherwise reconcile perfectly.
pub fn negative_leg(legs: &[Leg]) -> Option<Leg> {
    legs.iter().copied().find(|l| l.amount < 0)
}

/// Split legs must cover the bill exactly. `Err` carries the legs' sum.
pub fn legs_cover(legs: &[Leg], total: Minor) -> Result<(), Minor> {
    let sum: Minor = legs.iter().map(|l| l.amount).sum();
    if sum == total {
        Ok(())
    } else {
        Err(sum)
    }
}

/// The tender the server RECORDS for a sale, as `(amount_tendered,
/// change_given)`.
///
/// - A split: the notes handed over cover its CASH legs, so the change is what
///   they exceed those by; a split that names no usable tender (no cash leg,
///   or a tender below the cash legs) records none.
/// - Otherwise the till's figures, with the change falling back to `tendered −
///   total` (never below zero) when the till sent none.
pub fn recorded_tender(
    legs: &[Leg],
    tendered: Option<Minor>,
    change_given: Option<Minor>,
    total: Minor,
) -> (Option<Minor>, Option<Minor>) {
    if legs.is_empty() {
        return (
            tendered,
            change_given.or_else(|| tendered.map(|t| (t - total).max(0))),
        );
    }
    let cash_legs: Minor = legs.iter().filter(|l| l.is_cash).map(|l| l.amount).sum();
    match tendered.filter(|t| cash_legs > 0 && *t >= cash_legs) {
        Some(t) => (Some(t), Some(t - cash_legs)),
        None => (None, None),
    }
}

pub mod vectors {
    //! Vectors for bill assembly and tender, generated from the rules as they
    //! moved here (the server's order of operations). Regenerate deliberately:
    //! `MADAR_REGENERATE_BILL_VECTORS=1 cargo test -p madar-money bill_vectors`.

    use std::path::PathBuf;
    use std::str::FromStr;

    use rust_decimal::Decimal;
    use serde::{Deserialize, Serialize};

    use super::*;

    /// A discount as the vector file spells it: `none`, `percentage` /
    /// `fixed` with a decimal string, or `stated` with an amount.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct VDiscount {
        pub kind: String,
        pub value: String,
    }

    impl VDiscount {
        pub fn to_bill(&self) -> BillDiscount {
            let d = || Decimal::from_str(&self.value).unwrap();
            match self.kind.as_str() {
                "percentage" => BillDiscount::Rule(Discount::Percentage(d())),
                "fixed" => BillDiscount::Rule(Discount::Fixed(d())),
                "stated" => BillDiscount::Stated(self.value.parse().unwrap()),
                _ => BillDiscount::Rule(Discount::None),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct VPolicy {
        pub tax_rate: String,
        pub tax_inclusive: bool,
        pub service_charge_rate: String,
        pub service_charge_taxable: bool,
    }

    impl VPolicy {
        pub fn to_policy(&self) -> TaxPolicy {
            TaxPolicy {
                tax_rate: Decimal::from_str(&self.tax_rate).unwrap(),
                tax_inclusive: self.tax_inclusive,
                service_charge_rate: Decimal::from_str(&self.service_charge_rate).unwrap(),
                service_charge_taxable: self.service_charge_taxable,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct BillVector {
        pub lines: Vec<BillLine>,
        pub discount: VDiscount,
        pub policy: VPolicy,
        // Expected:
        pub nets: Vec<NetLine>,
        pub reward_covered: Minor,
        pub staff_comp: Minor,
        pub subtotal: Minor,
        pub discount_amount: Minor,
        pub service_charge: Minor,
        pub tax: Minor,
        pub total: Minor,
        /// `line:<index>` or a `NegativePart::as_str`, `null` when bookable.
        pub refusal: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct OpenBillVector {
        pub subtotal: Minor,
        pub discount: VDiscount,
        pub policy: VPolicy,
        pub out_subtotal: Minor,
        pub discount_amount: Minor,
        pub service_charge: Minor,
        pub tax: Minor,
        pub total: Minor,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct TenderVector {
        pub legs: Vec<Leg>,
        pub tendered: Option<Minor>,
        pub change_given: Option<Minor>,
        pub total: Minor,
        pub cash_tip: Minor,
        // Expected:
        pub negative_leg: Option<Minor>,
        /// `null` when the legs cover the total (or there are none), else their sum.
        pub legs_mismatch: Option<Minor>,
        pub recorded_tendered: Option<Minor>,
        pub recorded_change: Option<Minor>,
        pub change_due: Minor,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Vectors {
        pub bills: Vec<BillVector>,
        pub open_bills: Vec<OpenBillVector>,
        pub tenders: Vec<TenderVector>,
    }

    pub fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/bill_vectors.json")
    }

    fn policies() -> Vec<VPolicy> {
        let p = |t: &str, inc: bool, s: &str, st: bool| VPolicy {
            tax_rate: t.into(),
            tax_inclusive: inc,
            service_charge_rate: s.into(),
            service_charge_taxable: st,
        };
        vec![
            p("0", false, "0", true),
            p("0.14", false, "0", true),
            p("0.14", true, "0", true),
            p("0.14", false, "0.12", true),
            p("0.145", false, "0.1", false),
            p("0.25", true, "0.12", true),
        ]
    }

    fn discounts() -> Vec<VDiscount> {
        let d = |k: &str, v: &str| VDiscount {
            kind: k.into(),
            value: v.into(),
        };
        vec![
            d("none", "0"),
            d("percentage", "0.1"),
            d("percentage", "0.145"),
            d("percentage", "1.5"),
            d("percentage", "-0.2"),
            d("fixed", "250"),
            d("fixed", "12.5"),
            d("fixed", "99999"),
            d("stated", "300"),
            d("stated", "-5"),
            d("stated", "99999"),
        ]
    }

    fn line(charged: Minor, per_unit: Minor, reward_units: Minor, staff_comp: Minor) -> BillLine {
        BillLine {
            charged,
            per_unit,
            reward_units,
            staff_comp,
        }
    }

    fn line_sets() -> Vec<Vec<BillLine>> {
        vec![
            vec![],
            vec![line(1000, 1000, 0, 0)],
            vec![line(105, 105, 0, 0)],
            vec![
                line(2000, 1000, 0, 0),
                line(500, 500, 0, 0),
                line(1000, 250, 0, 0),
            ],
            // A reward: one unit of a two-unit line; a reward bigger than the line.
            vec![line(2000, 1000, 1, 0), line(1500, 1500, 0, 0)],
            vec![line(1200, 600, 5, 0)],
            // Size 5000 + a 1000 add-on, qty 2, one reward unit (discovery T3).
            vec![line(12000, 6000, 1, 0)],
            // A staff drink: the comp, a comp larger than the line.
            vec![line(3500, 3500, 0, 3000), line(800, 800, 0, 0)],
            vec![line(700, 700, 0, 900)],
            // Both on one line (a replay only): comp first, then the reward
            // covers what is left.
            vec![line(3000, 1500, 1, 2000)],
            // A line charged more than per unit × quantity (once a combo line's
            // component surcharge; kept as plain arithmetic).
            vec![line(6000, 5000, 0, 0)],
            // A line below zero, and a zero line.
            vec![line(-200, -200, 0, 0), line(1000, 1000, 0, 0)],
            vec![line(0, 0, 0, 0)],
            // Big figures.
            vec![line(50_000_000, 1_000_000, 0, 0)],
        ]
    }

    fn bill_case(lines: Vec<BillLine>, discount: VDiscount, policy: VPolicy) -> BillVector {
        let b = price_bill(&lines, discount.to_bill(), &policy.to_policy());
        let refusal = refusal(&b).map(|r| match r {
            BillRefusal::Line { index, .. } => format!("line:{index}"),
            BillRefusal::Part(p) => p.as_str().to_string(),
        });
        BillVector {
            nets: b.lines.clone(),
            reward_covered: b.reward_covered,
            staff_comp: b.staff_comp,
            subtotal: b.breakdown.subtotal,
            discount_amount: b.breakdown.discount,
            service_charge: b.breakdown.service_charge,
            tax: b.breakdown.tax,
            total: b.breakdown.total,
            refusal,
            lines,
            discount,
            policy,
        }
    }

    fn tender_case(
        legs: Vec<Leg>,
        tendered: Option<Minor>,
        change_given: Option<Minor>,
        total: Minor,
        cash_tip: Minor,
    ) -> TenderVector {
        let (rt, rc) = recorded_tender(&legs, tendered, change_given, total);
        TenderVector {
            negative_leg: negative_leg(&legs).map(|l| l.amount),
            legs_mismatch: if legs.is_empty() {
                None
            } else {
                legs_cover(&legs, total).err()
            },
            recorded_tendered: rt,
            recorded_change: rc,
            change_due: change_due(tendered, total, cash_tip),
            legs,
            tendered,
            change_given,
            total,
            cash_tip,
        }
    }

    pub fn generate() -> Vectors {
        let mut bills = Vec::new();
        for lines in line_sets() {
            for discount in discounts() {
                for policy in policies() {
                    bills.push(bill_case(lines.clone(), discount.clone(), policy));
                }
            }
        }
        let mut open_bills = Vec::new();
        for &subtotal in &[-300, 0, 1, 105, 1000, 5700, 123_457] {
            for discount in discounts().into_iter().filter(|d| d.kind != "stated") {
                for policy in policies() {
                    let b = price_open_bill(
                        subtotal,
                        match discount.to_bill() {
                            BillDiscount::Rule(d) => d,
                            BillDiscount::Stated(_) => unreachable!(),
                        },
                        &policy.to_policy(),
                    );
                    open_bills.push(OpenBillVector {
                        subtotal,
                        discount: discount.clone(),
                        policy,
                        out_subtotal: b.subtotal,
                        discount_amount: b.discount,
                        service_charge: b.service_charge,
                        tax: b.tax,
                        total: b.total,
                    });
                }
            }
        }
        let cash = |amount| Leg {
            amount,
            is_cash: true,
        };
        let card = |amount| Leg {
            amount,
            is_cash: false,
        };
        let mut tenders = Vec::new();
        let leg_sets: Vec<Vec<Leg>> = vec![
            vec![],
            vec![card(2850), cash(2550)],
            vec![card(5400)],
            vec![cash(0), card(5400)],
            vec![cash(3000), cash(2400)],
            vec![cash(-500), cash(500), card(5400)],
            vec![card(5000)],
        ];
        for legs in leg_sets {
            for &tendered in &[
                None,
                Some(0),
                Some(2000),
                Some(2550),
                Some(3000),
                Some(6000),
                Some(10_000_000),
            ] {
                for &change_given in &[None, Some(0), Some(450)] {
                    for &cash_tip in &[0, 200] {
                        tenders.push(tender_case(
                            legs.clone(),
                            tendered,
                            change_given,
                            5400,
                            cash_tip,
                        ));
                    }
                }
            }
        }
        Vectors {
            bills,
            open_bills,
            tenders,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn bill_vectors() {
            let generated = generate();
            if std::env::var("MADAR_REGENERATE_BILL_VECTORS").is_ok() {
                std::fs::write(
                    fixture_path(),
                    serde_json::to_string_pretty(&generated).unwrap() + "\n",
                )
                .unwrap();
                return;
            }
            let expected: Vectors = serde_json::from_str(crate::vectors::BILL).unwrap();
            assert_eq!(generated.bills.len(), expected.bills.len());
            for (g, e) in generated.bills.iter().zip(&expected.bills) {
                assert_eq!(g, e, "bill assembly drifted from its vectors");
            }
            assert_eq!(
                generated.open_bills, expected.open_bills,
                "open bill drifted"
            );
            assert_eq!(generated.tenders, expected.tenders, "tender drifted");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn exclusive(rate: rust_decimal::Decimal) -> TaxPolicy {
        TaxPolicy {
            tax_rate: rate,
            ..TaxPolicy::default()
        }
    }

    #[test]
    fn rewards_first_then_the_discount_on_what_is_left() {
        // Two units at 1000, one covered; 10% off the remaining 1000.
        let b = price_bill(
            &[BillLine {
                charged: 2000,
                per_unit: 1000,
                reward_units: 1,
                staff_comp: 0,
            }],
            BillDiscount::Rule(Discount::Percentage(dec!(0.1))),
            &exclusive(dec!(0.14)),
        );
        assert_eq!(b.reward_covered, 1000);
        assert_eq!(b.breakdown.subtotal, 1000);
        assert_eq!(b.breakdown.discount, 100);
        assert_eq!(b.breakdown.tax, 126);
        assert_eq!(b.breakdown.total, 1026);
        assert_eq!(refusal(&b), None);
    }

    #[test]
    fn the_staff_comp_comes_off_before_the_reward() {
        let n = net_line(&BillLine {
            charged: 3000,
            per_unit: 1500,
            reward_units: 1,
            staff_comp: 2000,
        });
        assert_eq!((n.staff_comp, n.covered, n.net), (2000, 1000, 0));
    }

    #[test]
    fn a_negative_line_is_refused_by_index() {
        let b = price_bill(
            &[
                BillLine {
                    charged: 10,
                    per_unit: 10,
                    ..Default::default()
                },
                BillLine {
                    charged: -1,
                    per_unit: -1,
                    ..Default::default()
                },
            ],
            BillDiscount::Rule(Discount::None),
            &TaxPolicy::default(),
        );
        assert_eq!(refusal(&b), Some(BillRefusal::Line { index: 1, net: -1 }));
    }

    #[test]
    fn a_split_records_the_change_over_its_cash_legs() {
        let legs = [
            Leg {
                amount: 2850,
                is_cash: false,
            },
            Leg {
                amount: 2550,
                is_cash: true,
            },
        ];
        assert_eq!(
            recorded_tender(&legs, Some(3000), None, 5400),
            (Some(3000), Some(450))
        );
        assert_eq!(recorded_tender(&legs, Some(2000), None, 5400), (None, None));
        assert_eq!(legs_cover(&legs, 5400), Ok(()));
    }

    #[test]
    fn a_stored_rule_reads_as_the_schema_spells_it() {
        use rust_decimal_macros::dec;
        assert_eq!(
            rule_of(Some("percentage"), dec!(0.10)),
            Discount::Percentage(dec!(0.10))
        );
        assert_eq!(
            rule_of(Some("fixed"), dec!(300)),
            Discount::Fixed(dec!(300))
        );
        assert_eq!(rule_of(Some("bogus"), dec!(0.5)), Discount::None);
        assert_eq!(rule_of(None, dec!(0.5)), Discount::None);
    }

    #[test]
    fn a_stated_subtotal_is_priced_as_stated_and_the_lines_still_netted() {
        use rust_decimal_macros::dec;
        let policy = TaxPolicy {
            tax_rate: dec!(0.14),
            ..TaxPolicy::default()
        };
        let lines = [BillLine {
            charged: 3000,
            per_unit: 1500,
            reward_units: 0,
            staff_comp: 500,
        }];
        let d = BillDiscount::Rule(Discount::Percentage(dec!(0.10)));
        let own = price_bill(&lines, d, &policy);
        assert_eq!(own, price_bill_on(&lines, None, d, &policy));
        assert_eq!(own.breakdown, price_subtotal(2500, d, &policy));
        let stated = price_bill_on(&lines, Some(4000), d, &policy);
        assert_eq!(stated.lines, own.lines);
        assert_eq!(stated.staff_comp, 500);
        assert_eq!(stated.breakdown, compute(4000, 400, &policy));
        // A stated subtotal below zero is the bill's subtotal part.
        let neg = price_bill_on(&lines, Some(-1), BillDiscount::Stated(50), &policy);
        assert_eq!(neg.breakdown.discount, 0);
        assert_eq!(
            refusal(&neg),
            Some(BillRefusal::Part(NegativePart::Subtotal))
        );
    }

    #[test]
    fn a_stated_discount_is_clamped_to_the_subtotal() {
        let p = TaxPolicy::default();
        assert_eq!(
            price_subtotal(1000, BillDiscount::Stated(5000), &p).discount,
            1000
        );
        assert_eq!(
            price_subtotal(1000, BillDiscount::Stated(-5), &p).discount,
            0
        );
    }

    #[test]
    fn change_subtracts_the_cash_tip_and_is_capped() {
        assert_eq!(change_due(Some(1500), 1000, 200), 300);
        assert_eq!(change_due(Some(900), 1000, 0), 0);
        assert_eq!(change_due(None, 1000, 0), 0);
        assert_eq!(change_due(Some(i64::MAX / 2), 100, 0), CHANGE_CAP);
    }
}
