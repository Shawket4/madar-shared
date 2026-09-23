//! Vectors for [`refund_split`]: the tax and service charge a refund takes
//! back, pinned for every consumer of the rule.
//!
//! The split exists THREE times: here (the server's and the till's Rust,
//! formerly two identical copies) and in SQL — `refund_share()`, which the
//! `order_refunds_before_insert` trigger calls and which is the authoritative
//! copy for the books. These vectors were generated from the Rust as it stood
//! when it moved here, and the backend runs `refund_share()` over the same
//! file (MadarRust `tests/refund_split_sql_vectors_tests.rs`), so the SQL and
//! the Rust can no longer drift apart unnoticed.
//!
//! The SQL is only ever handed what the trigger hands it: an `already`
//! refunded sum and an `amount` that are both zero or more. Cases outside that
//! domain (negative inputs, where the Rust clamps) carry `"sql": false`.
//!
//! Regenerate deliberately:
//! `MADAR_REGENERATE_REFUND_VECTORS=1 cargo test -p madar-money refund_split_vectors`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{refund_split, Minor};

/// One refund against one order, and what it takes back.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RefundVector {
    pub order_total: Minor,
    pub order_tax: Minor,
    pub order_service_charge: Minor,
    /// The sum of the order's earlier refunds.
    pub refunded_before: Minor,
    pub amount: Minor,
    /// Whether the SQL `refund_share()` is defined on these inputs (see the
    /// module note): the trigger never passes a negative sum or amount.
    pub sql: bool,
    // Expected:
    pub tax: Minor,
    pub service_charge: Minor,
}

pub fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/refund_split_vectors.json")
}

fn case(total: Minor, tax: Minor, sc: Minor, before: Minor, amount: Minor) -> RefundVector {
    let (t, s) = refund_split(total, tax, sc, before, amount);
    RefundVector {
        order_total: total,
        order_tax: tax,
        order_service_charge: sc,
        refunded_before: before,
        amount,
        sql: before >= 0 && amount >= 0,
        tax: t,
        service_charge: s,
    }
}

/// Every case worth pinning: the table both unit suites carried, a grid over
/// order shapes and refund positions (where the cumulative rounding lands on a
/// half), whole sequences that empty an order, and the clamps.
pub fn generate() -> Vec<RefundVector> {
    let mut out = Vec::new();
    // The unit table the server and the till both carried.
    for &(total, tax, sc, before, amount) in &[
        (11400, 1400, 0, 0, 5700),
        (11400, 1400, 0, 5700, 5700),
        (12768, 1568, 1200, 0, 1000),
        (12768, 1568, 1200, 1000, 11768),
        (12768, 1568, 1200, 0, 12768),
        (12768, 1568, 1200, 12000, 5000),
        (333, 41, 0, 0, 1),
        (333, 41, 0, 1, 1),
        (333, 41, 0, 2, 331),
        (0, 0, 0, 0, 100),
    ] {
        out.push(case(total, tax, sc, before, amount));
    }
    // The grid. Orders: tax only, tax + a service charge, odd totals whose
    // shares land on halves, one tiny and one large bill.
    let orders: &[(Minor, Minor, Minor)] = &[
        (1, 0, 0),
        (7, 1, 0),
        (333, 41, 0),
        (5700, 700, 0),
        (5701, 700, 1),
        (6384, 784, 600),
        (6300, 700, 600),
        (12768, 1568, 1200),
        (98765, 12129, 8888),
        (1_000_000, 122_807, 100_000),
    ];
    for &(total, tax, sc) in orders {
        let positions = [0, 1, total / 3, total / 2, total - 1, total];
        let amounts = [1, 2, total / 7, total / 2, total - 1, total, total + 5];
        for &before in &positions {
            for &amount in &amounts {
                if before < 0 || amount <= 0 {
                    continue;
                }
                out.push(case(total, tax, sc, before, amount));
            }
        }
    }
    // Sequences that empty the order: each refund is a case, and together they
    // take back exactly the order's tax and service charge.
    for &(total, tax, sc, steps) in &[
        (98765, 12129, 8888, &[1, 333, 4999, 12345, 40000, 41087][..]),
        (333, 41, 0, &[1, 1, 1, 330][..]),
        (5701, 700, 1, &[1900, 1900, 1901][..]),
        (12768, 1568, 1200, &[3, 5, 7, 12753][..]),
    ] {
        let mut before = 0;
        for &amount in steps {
            out.push(case(total, tax, sc, before, amount));
            before += amount;
        }
    }
    // The clamps: no order, a negative order, figures below zero, a negative
    // earlier sum or amount, an over-refund.
    for &(total, tax, sc, before, amount) in &[
        (0, 100, 100, 0, 50),
        (-100, 14, 0, 0, 50),
        (5700, -700, 0, 0, 5700),
        (5700, 700, -10, 0, 5700),
        (5700, 700, 0, -5, 100),
        (5700, 700, 0, 100, -3),
        (5700, 700, 0, 6000, 100),
        (5700, 700, 0, 5000, 10_000),
    ] {
        out.push(case(total, tax, sc, before, amount));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refund_split_vectors() {
        let generated = generate();
        if std::env::var("MADAR_REGENERATE_REFUND_VECTORS").is_ok() {
            std::fs::write(
                fixture_path(),
                serde_json::to_string_pretty(&generated).unwrap() + "\n",
            )
            .unwrap();
            return;
        }
        let expected: Vec<RefundVector> =
            serde_json::from_str(crate::vectors::REFUND_SPLIT).unwrap();
        assert_eq!(
            generated, expected,
            "refund_split drifted from its vectors — and from the SQL refund_share() pinned to them"
        );
    }
}
