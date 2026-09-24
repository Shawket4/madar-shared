//! The facts an act on ONE sale is judged on (discovery A2).
//!
//! A void is judged on whose sale it is and how old it is at the moment it was
//! voided (`at` — now live, the queued op's own timestamp on replay, so a
//! queue drained hours later is not judged by the drain time). "Own" is the
//! ORDER's teller — the person who rang it — never the till's opener: the
//! till used the opener until fix A2 and waved through a teller voiding a
//! manager's sale that the server then refused.
//!
//! Moved from MadarRust `authz/acts.rs` (`VoidAsk`, the arithmetic of
//! `void_ask`); madar-core `approvals.rs` `order_facts` computes the same
//! facts from its ledger with this.

use chrono::{DateTime, Utc};

use crate::{Cap, Request};

/// Whose sale, and how old in whole minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoidAsk {
    pub own: bool,
    pub age_minutes: i64,
}

impl VoidAsk {
    /// The request `decide` answers for this void.
    pub fn request(&self) -> Request {
        Request::of(Cap::OrdersVoid)
            .own(self.own)
            .age_minutes(self.age_minutes)
    }
}

/// The void facts: `own` when the order's teller is the actor; the age is the
/// whole minutes from the sale to `at`, truncated, and a clock that puts the
/// sale in the future reads as age 0 rather than a negative age.
pub fn void_facts<T: PartialEq + ?Sized>(
    order_teller: &T,
    actor: &T,
    created_at: DateTime<Utc>,
    at: DateTime<Utc>,
) -> VoidAsk {
    VoidAsk {
        own: order_teller == actor,
        age_minutes: age_minutes(created_at, at),
    }
}

/// Whole minutes from `created_at` to `at`, never negative.
pub fn age_minutes(created_at: DateTime<Utc>, at: DateTime<Utc>) -> i64 {
    (at - created_at).num_minutes().max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    #[test]
    fn own_is_the_orders_teller_and_age_truncates() {
        let a = void_facts(
            "teller-a",
            "teller-a",
            t("2026-09-24T10:00:00.900Z"),
            t("2026-09-24T10:05:00.100Z"),
        );
        assert_eq!(
            a,
            VoidAsk {
                own: true,
                age_minutes: 4
            },
            "4 min 59.2 s is 4"
        );
        let b = void_facts(
            "m",
            "teller-a",
            t("2026-09-24T10:00:00Z"),
            t("2026-09-24T09:00:00Z"),
        );
        assert_eq!(
            b,
            VoidAsk {
                own: false,
                age_minutes: 0
            },
            "a future sale is age 0"
        );
    }
}
