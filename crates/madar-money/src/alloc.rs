//! The one allocator: split an amount over weights so the shares sum to the
//! amount EXACTLY.
//!
//! Used for a combo's price over its parts (by their menu prices) and for a
//! deal's discount over the units of a chunk (by their prices). Cumulative
//! pro-rata — the `refund_split` pattern: each share is what the running
//! weight should have taken so far, minus what the earlier shares took, so
//! the rounding never adds up to a piastre more or less than the total.
//!
//! Pinned by `vectors/alloc_vectors.json` (hand-computed).

/// `num / den` rounded half away from zero, for `num ≥ 0` and `den > 0`:
/// `(2·num + den) div (2·den)`. A `den` of 0 gives 0.
pub fn round_half_away(num: i128, den: i128) -> i128 {
    if den <= 0 {
        return 0;
    }
    let num = num.max(0);
    (2 * num + den) / (2 * den)
}

/// `total` over `weights`: one share per weight, in order.
///
/// - No weights → no shares.
/// - A negative total or weight counts as 0.
/// - When every weight is 0 the split is equal (every weight 1).
/// - Σ shares == total (after the clamp), and every share ≥ 0.
pub fn split(total: i64, weights: &[i64]) -> Vec<i64> {
    if weights.is_empty() {
        return Vec::new();
    }
    let total = i128::from(total.max(0));
    let mut w: Vec<i128> = weights.iter().map(|&x| i128::from(x.max(0))).collect();
    if w.iter().all(|&x| x == 0) {
        w.iter_mut().for_each(|x| *x = 1);
    }
    let big_w: i128 = w.iter().sum();
    let mut cum: i128 = 0;
    let mut prev: i128 = 0;
    let mut out = Vec::with_capacity(w.len());
    for wi in w {
        cum += wi;
        let upto = round_half_away(total * cum, big_w);
        out.push((upto - prev) as i64);
        prev = upto;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Vectors {
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Case {
        name: String,
        total: i64,
        weights: Vec<i64>,
        expected: Vec<i64>,
    }

    #[test]
    fn alloc_vectors() {
        let v: Vectors = serde_json::from_str(crate::vectors::ALLOC).unwrap();
        assert!(v.cases.len() >= 12);
        for c in &v.cases {
            assert_eq!(split(c.total, &c.weights), c.expected, "{}", c.name);
        }
    }

    #[test]
    fn the_sum_is_always_exact_and_no_share_is_negative() {
        // A small deterministic sweep (no randomness in this crate).
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..5000 {
            let n = (next() % 7) as usize + 1;
            let total = (next() % 1_000_000) as i64;
            let weights: Vec<i64> = (0..n).map(|_| (next() % 30_000) as i64).collect();
            let s = split(total, &weights);
            assert_eq!(s.len(), n);
            assert_eq!(s.iter().sum::<i64>(), total, "{total} over {weights:?}");
            assert!(s.iter().all(|&x| x >= 0), "{total} over {weights:?}: {s:?}");
        }
    }

    #[test]
    fn rounding_is_half_away() {
        assert_eq!(round_half_away(5, 2), 3);
        assert_eq!(round_half_away(3, 2), 2);
        assert_eq!(round_half_away(1, 3), 0);
        assert_eq!(round_half_away(2, 3), 1);
        assert_eq!(round_half_away(7, 0), 0);
    }
}
