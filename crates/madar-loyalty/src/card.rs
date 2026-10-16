//! What a member's card shows: the rewards a balance has already earned, the
//! progress towards the next, what that one still needs, and the stamps.
//!
//! The server's rule (MadarRust `loyalty::model::earned_and_progress` and
//! `MemberRow::view`, which the card page and the passes show), ported as it
//! is; the stamp row is the pass's `wallet::google::stepper`, whose cap the
//! web card shares. Pinned by `vectors/loyalty_card_vectors.json`
//! (hand-computed).
//!
//! A card does not stop at full: six stamps against a five-stamp reward is one
//! reward earned and one stamp towards the next.

use serde::{Deserialize, Serialize};

/// Past this many steps a card is drawn as figures, not stamps (the pass's
/// `MAX_STEPS`).
pub const MAX_STEPS: i32 = 12;

/// The card's figures for a balance against the next reward's cost.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    /// Rewards the balance has already earned.
    pub rewards_ready: i32,
    /// Progress towards the next reward, the earned ones set aside.
    pub progress_to_next: i32,
    /// What the next reward still needs: the full cost on an exact multiple.
    pub points_to_next_reward: i32,
    /// At least one reward is earned.
    pub can_redeem: bool,
}

/// `(rewards earned, progress towards the next)`; with no cost, or nothing
/// above zero, none earned and the balance (at least 0) as the progress.
pub fn earned_and_progress(balance: i32, cost: i32) -> (i32, i32) {
    if cost <= 0 || balance <= 0 {
        return (0, balance.max(0));
    }
    (balance / cost, balance % cost)
}

/// The card for `balance` against `next_reward_cost`.
pub fn card(balance: i32, next_reward_cost: i32) -> Card {
    let (rewards_ready, progress_to_next) = earned_and_progress(balance, next_reward_cost);
    Card {
        rewards_ready,
        progress_to_next,
        points_to_next_reward: (next_reward_cost - progress_to_next).max(0),
        can_redeem: rewards_ready > 0,
    }
}

/// How many of a `cost`-step stamp row `earned` fills (clamped to the row),
/// or `None` when the card has no row: a cost of 0 or less, or above
/// [`MAX_STEPS`].
#[must_use]
pub fn stamps(earned: i32, cost: i32) -> Option<i32> {
    if cost <= 0 || cost > MAX_STEPS {
        return None;
    }
    Some(earned.clamp(0, cost))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Vectors {
        cards: Vec<CardCase>,
        stamps: Vec<StampCase>,
    }

    #[derive(Deserialize)]
    struct CardCase {
        name: String,
        balance: i32,
        next_reward_cost: i32,
        expected: Card,
    }

    #[derive(Deserialize)]
    struct StampCase {
        name: String,
        earned: i32,
        cost: i32,
        expected: Option<i32>,
    }

    #[test]
    fn loyalty_card_vectors() {
        let v: Vectors = serde_json::from_str(crate::vectors::CARD).unwrap();
        assert!(v.cards.len() >= 10 && v.stamps.len() >= 6);
        for c in &v.cards {
            assert_eq!(
                card(c.balance, c.next_reward_cost),
                c.expected,
                "{}",
                c.name
            );
        }
        for c in &v.stamps {
            assert_eq!(stamps(c.earned, c.cost), c.expected, "{}", c.name);
        }
    }
}
