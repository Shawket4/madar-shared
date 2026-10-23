//! Who a dashboard message goes to, and the segments of the members table.
//!
//! An audience is every member, or the members matching ALL of its rules.
//! The server turns a valid audience into SQL; the dashboard builds and checks
//! it with the same code. Members who opted out of marketing are never in a
//! message's audience, whatever the rules say (the server applies that, not
//! the rules: it is not optional).

use serde::{Deserialize, Serialize};

use crate::validate::Problem;

/// At most this many rules in one audience.
pub const MAX_RULES: usize = 6;
/// Day counts run from 1 to a year.
pub const MAX_DAYS: u32 = 365;
/// A balance rule's figure.
pub const MAX_BALANCE: i32 = 1_000_000;

/// A wallet a member may hold the card in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Wallet {
    Apple,
    Google,
    /// In neither wallet (reachable by WhatsApp only).
    None,
}

/// One rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Rule {
    /// The track's balance is at least `value`.
    BalanceAtLeast { track: u8, value: i32 },
    /// The track's balance is at most `value`.
    BalanceAtMost { track: u8, value: i32 },
    /// The track has a reward earned and not yet taken.
    RewardReady { track: u8 },
    /// Seen (any stamp, point or reward) within the last `days`.
    VisitedWithin { days: u32 },
    /// Not seen for at least `days` (never seen counts from joining).
    NotVisitedFor { days: u32 },
    /// Joined within the last `days`.
    JoinedWithin { days: u32 },
    /// Born in `month` (1–12).
    BirthdayMonth { month: u8 },
    /// Holds the card in this wallet.
    HasWallet { wallet: Wallet },
}

/// Every member (no rules), or those matching all of `rules`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Audience {
    #[serde(default)]
    pub rules: Vec<Rule>,
}

/// The problems with `audience` for a shop running `track_count` tracks.
pub fn validate_audience(audience: &Audience, track_count: u8) -> Vec<Problem> {
    let mut out = Vec::new();
    if audience.rules.len() > MAX_RULES {
        out.push(Problem::error("too_many_rules", "rules"));
    }
    let days_ok = |d: u32| (1..=MAX_DAYS).contains(&d);
    for (i, r) in audience.rules.iter().enumerate() {
        let path = format!("rules[{i}]");
        match *r {
            Rule::BalanceAtLeast { track, value } | Rule::BalanceAtMost { track, value } => {
                if track == 0 || track > track_count {
                    out.push(Problem::error("track_unknown", &path));
                }
                if !(0..=MAX_BALANCE).contains(&value) {
                    out.push(Problem::error("balance_range", &path));
                }
            }
            Rule::RewardReady { track } => {
                if track == 0 || track > track_count {
                    out.push(Problem::error("track_unknown", &path));
                }
            }
            Rule::VisitedWithin { days }
            | Rule::NotVisitedFor { days }
            | Rule::JoinedWithin { days } => {
                if !days_ok(days) {
                    out.push(Problem::error("days_range", &path));
                }
            }
            Rule::BirthdayMonth { month } => {
                if !(1..=12).contains(&month) {
                    out.push(Problem::error("month_range", &path));
                }
            }
            Rule::HasWallet { .. } => {}
        }
        // Two rules that can never both hold leave nobody to send to.
        for other in audience.rules.iter().take(i) {
            let clash = match (*r, *other) {
                (Rule::VisitedWithin { days: a }, Rule::NotVisitedFor { days: b })
                | (Rule::NotVisitedFor { days: b }, Rule::VisitedWithin { days: a }) => a <= b,
                (
                    Rule::BalanceAtLeast {
                        track: t1,
                        value: lo,
                    },
                    Rule::BalanceAtMost {
                        track: t2,
                        value: hi,
                    },
                )
                | (
                    Rule::BalanceAtMost {
                        track: t2,
                        value: hi,
                    },
                    Rule::BalanceAtLeast {
                        track: t1,
                        value: lo,
                    },
                ) => t1 == t2 && lo > hi,
                (Rule::BirthdayMonth { month: a }, Rule::BirthdayMonth { month: b }) => a != b,
                (Rule::HasWallet { wallet: a }, Rule::HasWallet { wallet: b }) => {
                    a != b && (a == Wallet::None || b == Wallet::None)
                }
                _ => false,
            };
            if clash {
                out.push(Problem::error("rules_contradict", &path));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules() {
        let codes = |rules: Vec<Rule>, n: u8| {
            validate_audience(&Audience { rules }, n)
                .into_iter()
                .map(|p| p.code)
                .collect::<Vec<_>>()
        };
        assert!(codes(vec![], 1).is_empty());
        assert_eq!(
            codes(vec![Rule::RewardReady { track: 2 }], 1),
            ["track_unknown"]
        );
        assert_eq!(
            codes(vec![Rule::NotVisitedFor { days: 0 }], 1),
            ["days_range"]
        );
        assert_eq!(
            codes(
                vec![
                    Rule::VisitedWithin { days: 7 },
                    Rule::NotVisitedFor { days: 30 }
                ],
                1
            ),
            ["rules_contradict"]
        );
        assert!(codes(
            vec![
                Rule::VisitedWithin { days: 60 },
                Rule::NotVisitedFor { days: 30 }
            ],
            1
        )
        .is_empty());
        assert_eq!(
            codes(vec![Rule::BirthdayMonth { month: 13 }], 1),
            ["month_range"]
        );
    }
}
