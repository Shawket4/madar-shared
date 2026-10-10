//! How much a warehouse should send a branch (WAREHOUSE_DESIGN.md §6).
//!
//! A branch needs an ingredient once it is at or under its low-stock level
//! (`par_min > 0 && on_hand <= par_min`, the low-stock report's rule). It
//! needs enough to reach `par_max` (or `par_min` when no max is set), less
//! what is already coming: in transit to it, and open requests or drafts
//! addressed to it. The warehouse offers what it has that no open draft has
//! claimed yet. The suggestion is the smaller of the two. Nothing is sent
//! automatically; a person turns suggestions into a draft and dispatches it.

use serde::{Deserialize, Serialize};

use crate::{from_milli, milli};

/// One ingredient, one branch, one warehouse. All in the base stock unit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "tsify",
    derive(tsify::Tsify),
    tsify(missing_as_null, rename = "ReplenishInput")
)]
pub struct Input {
    pub on_hand: f64,
    pub par_min: f64,
    pub par_max: Option<f64>,
    /// Σ `qty_sent` of `dispatched` transfers to this branch.
    pub in_transit: f64,
    /// Σ line quantities of `requested`/`draft` transfers to this branch.
    pub open_inbound: f64,
    /// The warehouse's on hand.
    pub warehouse_on_hand: f64,
    /// Σ line quantities of the warehouse's `draft` transfers to anyone.
    pub warehouse_drafted_out: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "tsify",
    derive(tsify::Tsify),
    tsify(missing_as_null, rename = "ReplenishSuggestion")
)]
pub struct Suggestion {
    /// What the branch is short of after what is already coming.
    pub need: f64,
    /// What the warehouse can still promise.
    pub available: f64,
    /// `min(need, available)`.
    pub suggested: f64,
}

pub fn suggest(i: &Input) -> Suggestion {
    let on_hand = milli(i.on_hand);
    let par_min = milli(i.par_min);
    let need = if par_min > 0 && on_hand <= par_min {
        let target = i.par_max.map(milli).unwrap_or(par_min).max(par_min);
        (target - on_hand - milli(i.in_transit) - milli(i.open_inbound)).max(0)
    } else {
        0
    };
    let available = (milli(i.warehouse_on_hand) - milli(i.warehouse_drafted_out)).max(0);
    Suggestion {
        need: from_milli(need),
        available: from_milli(available),
        suggested: from_milli(need.min(available)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(on_hand: f64, par_min: f64, par_max: Option<f64>, wh: f64) -> Input {
        Input {
            on_hand,
            par_min,
            par_max,
            in_transit: 0.0,
            open_inbound: 0.0,
            warehouse_on_hand: wh,
            warehouse_drafted_out: 0.0,
        }
    }

    #[test]
    fn fills_to_max_capped_by_the_warehouse() {
        assert_eq!(suggest(&input(2.0, 5.0, Some(20.0), 100.0)).suggested, 18.0);
        assert_eq!(suggest(&input(2.0, 5.0, Some(20.0), 7.5)).suggested, 7.5);
        assert_eq!(suggest(&input(2.0, 5.0, None, 100.0)).suggested, 3.0);
    }

    #[test]
    fn above_par_or_no_par_needs_nothing() {
        assert_eq!(suggest(&input(6.0, 5.0, Some(20.0), 100.0)).need, 0.0);
        assert_eq!(suggest(&input(-3.0, 0.0, Some(20.0), 100.0)).need, 0.0);
    }

    #[test]
    fn what_is_coming_counts() {
        let mut i = input(2.0, 5.0, Some(20.0), 100.0);
        i.in_transit = 10.0;
        i.open_inbound = 5.0;
        assert_eq!(suggest(&i).suggested, 3.0);
        i.warehouse_drafted_out = 99.0;
        assert_eq!(suggest(&i).suggested, 1.0);
    }
}
