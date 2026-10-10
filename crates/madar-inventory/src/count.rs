//! Stock counts: which counted row is "suspicious" and needs a reason before
//! the count is finalized.
//!
//! The server's rule is MadarRust `stocktakes/handlers.rs`
//! `is_variance_flagged` (finalize) and the report's SQL. Finalize compared
//! floats, so book 1 counted 0.9 at 10 % (a difference of 9.999…%) was not
//! flagged while the report, in exact `numeric`, flagged it (SHARED_RULES_PLAN
//! D1). Here it is exact: quantities in whole thousandths (the
//! `numeric(12,3)` grain), the threshold in thousandths of a percent.
//!
//! Pinned by `vectors/count_vectors.json` (hand-computed).

use crate::purchase::milli;

/// True when `counted` differs from `book` by at least `pct` percent of the
/// book quantity, or when stock appears from zero (book 0, anything counted).
///
/// In integers ([`milli`], the column's thousandths): `b = milli(book)`,
/// `c = milli(counted)`, `p = milli(pct)` (thousandths of a percent); book 0
/// flags any `c ≠ 0`, else `|c − b| × 100 000 ≥ |b| × p`. A threshold of 0
/// flags every row of a non-zero book, an exact count included.
pub fn is_variance_flagged(book: f64, counted: f64, pct: f64) -> bool {
    let (b, c, p) = (
        i128::from(milli(book)),
        i128::from(milli(counted)),
        i128::from(milli(pct)),
    );
    if b == 0 {
        return c != 0;
    }
    (c - b).abs() * 100_000 >= b.abs() * p
}

#[cfg(test)]
mod vectors {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct Case {
        name: String,
        book: f64,
        counted: f64,
        pct: f64,
        expected: bool,
    }

    #[derive(Deserialize)]
    struct Vectors {
        cases: Vec<Case>,
    }

    #[test]
    fn count_vectors() {
        let v: Vectors = serde_json::from_str(crate::vectors::COUNT).unwrap();
        for c in &v.cases {
            assert_eq!(
                is_variance_flagged(c.book, c.counted, c.pct),
                c.expected,
                "{}",
                c.name
            );
        }
    }

    #[test]
    fn d1_the_float_rule_missed_it() {
        // MadarRust's float comparison, for the record.
        let float =
            |book: f64, counted: f64, pct: f64| (counted - book).abs() / book.abs() * 100.0 >= pct;
        assert!(!float(1.0, 0.9, 10.0));
        assert!(is_variance_flagged(1.0, 0.9, 10.0));
    }
}
