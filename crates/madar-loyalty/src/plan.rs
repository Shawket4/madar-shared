//! The redemption planner: which of the asked rewards a sale takes.
//!
//! Moved from MadarRust `loyalty/redeem.rs` (`plan_strict`, the SERVER mode,
//! and `structural_lines`, the replay) and madar-core `loyalty.rs`
//! (`reward_board`'s trimming loop and `unit_cost_for`, the TILL mode). The
//! two agreed on every case the discovery tried; the rules below are each
//! side's own, written once:
//!
//! - a line is named by its index; one reward per line;
//! - a line with no menu item is never a reward; on the till a
//!   staff drink is not either (the server refuses that pairing in the order
//!   path, with its own message, so its planner does not judge it);
//! - the unit cost is the programme's first catalogue entry for the item,
//!   else — when the programme rewards any item — its default cost; a cost of
//!   zero or less is refused (server) or not on offer (till), never free;
//! - units: at least one, at most the line's quantity;
//! - the shop's ceiling counts UNITS across the sale (a cap of zero or less
//!   is no cap; the schema refuses one);
//! - the whole plan's cost must fit the member's balance.

use serde::{Deserialize, Serialize};

/// One line of the sale, as the planner reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    /// The line's menu item; `None` for a line without one.
    #[serde(default)]
    pub menu_item_id: Option<String>,
    pub quantity: i64,
    /// Put on the staff pool (the till only; see the module doc).
    #[serde(default)]
    pub is_staff_drink: bool,
}

/// A catalogue reward: the item and what one unit costs.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reward {
    pub menu_item_id: String,
    pub cost: i64,
}

/// The member's programme at this branch, and their balance in its currency.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Programme {
    /// The catalogue, in the server's order (the first entry for an item
    /// wins).
    #[serde(default)]
    pub rewards: Vec<Reward>,
    /// "Collect five, get anything": an item the catalogue does not list costs
    /// `any_item_cost`.
    #[serde(default)]
    pub any_item: bool,
    #[serde(default)]
    pub any_item_cost: i64,
    /// Units one sale may claim.
    #[serde(default)]
    pub max_per_order: Option<i64>,
    pub balance: i64,
}

/// A reward the sale asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    /// The line's index; `None` when the request named none.
    #[serde(default)]
    pub line: Option<usize>,
    /// Units; `None` reads as one.
    #[serde(default)]
    pub units: Option<i64>,
}

/// Strict or trimming.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// The till: keep what may be taken, trim the rest and name it.
    Till,
    /// The server: refuse the sale when any ask may not be taken as asked.
    Server,
}

/// One reward the plan takes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Planned {
    pub line: usize,
    pub menu_item_id: String,
    pub units: i64,
    /// One unit's cost.
    pub unit_cost: i64,
    /// `unit_cost × units`.
    pub cost: i64,
}

/// Why the till took something off (the FIRST reason only).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trim {
    /// An ask named a line that is not in the sale.
    LineGone,
    /// An ask named a line that is not a reward (a line with no menu item, a
    /// staff drink, an item not on offer, an unpriced reward).
    NotOnOffer,
    /// An ask wanted more units than the line holds.
    LineShrank,
    /// The shop's per-order ceiling.
    OverCap,
    /// The balance.
    BalanceShort,
}

/// Why the server refuses the sale's rewards.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "refusal")]
pub enum Refusal {
    /// An ask named no line.
    NoLineNamed,
    /// An ask named a line past the end.
    NoSuchLine { line: usize },
    /// Two asks named one line.
    TwiceOnOneLine,
    /// An ask for fewer than one unit.
    BelowOneUnit,
    /// An ask for more units than the line holds.
    MoreUnitsThanLine { have: i64, asked: i64 },
    /// The line has no menu item to be a reward of. (Until combos were
    /// removed on 2026-09-25 this was `Bundle`: combo lines were the lines
    /// without one.)
    NoMenuItem,
    /// The item is not a reward at this branch.
    NotOnOffer,
    /// The catalogue prices this reward at nothing.
    NoPrice,
    /// More units than the shop's ceiling.
    OverCap { max: i64, claimed: i64 },
    /// The balance does not cover the plan.
    BalanceShort { balance: i64, spent: i64 },
}

/// A plan: the rewards taken, what they cost and how many units they claim.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub lines: Vec<Planned>,
    pub cost: i64,
    pub units: i64,
    /// The till's first trim; always `None` in server mode.
    #[serde(default)]
    pub trimmed: Option<Trim>,
}

impl Plan {
    /// Units of `line` the plan covers.
    pub fn units_for(&self, line: usize) -> i64 {
        self.lines
            .iter()
            .find(|p| p.line == line)
            .map_or(0, |p| p.units)
    }
}

/// The catalogue's cost for one unit of `item`, before the "no price" check:
/// the first catalogue entry, else the any-item cost, else not a reward.
fn listed_cost(item: &str, programme: &Programme) -> Option<i64> {
    match programme.rewards.iter().find(|r| r.menu_item_id == item) {
        Some(r) => Some(r.cost),
        None if programme.any_item => Some(programme.any_item_cost),
        None => None,
    }
}

/// What one unit of `line` costs, if the TILL may offer it as a reward: not a
/// line without a menu item, not a staff drink, on offer, priced above zero.
pub fn unit_cost(line: &Line, programme: &Programme) -> Option<i64> {
    if line.is_staff_drink {
        return None;
    }
    let item = line.menu_item_id.as_deref()?;
    listed_cost(item, programme).filter(|c| *c > 0)
}

fn cap_of(programme: &Programme) -> Option<i64> {
    programme.max_per_order.filter(|c| *c > 0)
}

/// Plan the asked rewards over `lines`.
///
/// [`Mode::Server`] returns the first [`Refusal`], in the server's order: each
/// ask in turn (named line, the line itself, one per line, at least one unit,
/// no more than the line, a menu item, on offer, priced), then the ceiling,
/// then the balance. [`Mode::Till`] never refuses: asks are honoured in the
/// order given and trimmed to what may be taken.
pub fn plan(
    lines: &[Line],
    programme: &Programme,
    asks: &[Ask],
    mode: Mode,
) -> Result<Plan, Refusal> {
    // Nothing asked is nothing to judge, whatever the balance (the server's
    // `plan` returns before it looks at one).
    if asks.is_empty() {
        return Ok(Plan::default());
    }
    match mode {
        Mode::Server => plan_strict(lines, programme, asks),
        Mode::Till => Ok(plan_trimmed(lines, programme, asks)),
    }
}

fn plan_strict(lines: &[Line], programme: &Programme, asks: &[Ask]) -> Result<Plan, Refusal> {
    let mut out: Vec<Planned> = Vec::new();
    let mut spent = 0i64;
    for a in asks {
        let index = a.line.ok_or(Refusal::NoLineNamed)?;
        let line = lines
            .get(index)
            .ok_or(Refusal::NoSuchLine { line: index })?;
        if out.iter().any(|p| p.line == index) {
            return Err(Refusal::TwiceOnOneLine);
        }
        let units = a.units.unwrap_or(1);
        if units < 1 {
            return Err(Refusal::BelowOneUnit);
        }
        if units > line.quantity {
            return Err(Refusal::MoreUnitsThanLine {
                have: line.quantity,
                asked: units,
            });
        }
        let item = line.menu_item_id.as_deref().ok_or(Refusal::NoMenuItem)?;
        let unit = listed_cost(item, programme).ok_or(Refusal::NotOnOffer)?;
        if unit <= 0 {
            return Err(Refusal::NoPrice);
        }
        let cost = unit.saturating_mul(units);
        spent = spent.saturating_add(cost);
        out.push(Planned {
            line: index,
            menu_item_id: item.to_string(),
            units,
            unit_cost: unit,
            cost,
        });
    }
    let claimed: i64 = out.iter().map(|p| p.units).sum();
    if let Some(max) = cap_of(programme) {
        if claimed > max {
            return Err(Refusal::OverCap { max, claimed });
        }
    }
    if spent > programme.balance {
        return Err(Refusal::BalanceShort {
            balance: programme.balance,
            spent,
        });
    }
    Ok(Plan {
        lines: out,
        cost: spent,
        units: claimed,
        trimmed: None,
    })
}

fn plan_trimmed(lines: &[Line], programme: &Programme, asks: &[Ask]) -> Plan {
    let balance = programme.balance.max(0);
    let cap = cap_of(programme);
    let mut out: Vec<Planned> = Vec::new();
    let mut cost = 0i64;
    let mut claimed = 0i64;
    let mut trimmed: Option<Trim> = None;
    for a in asks {
        let Some((index, line)) = a.line.and_then(|i| lines.get(i).map(|l| (i, l))) else {
            trimmed.get_or_insert(Trim::LineGone);
            continue;
        };
        if out.iter().any(|p| p.line == index) {
            continue;
        }
        let Some(unit) = unit_cost(line, programme) else {
            trimmed.get_or_insert(Trim::NotOnOffer);
            continue;
        };
        let asked = a.units.unwrap_or(1);
        let mut units = asked.clamp(0, line.quantity.max(0));
        if units < asked {
            trimmed.get_or_insert(Trim::LineShrank);
        }
        if let Some(c) = cap {
            let room = (c - claimed).max(0);
            if units > room {
                units = room;
                trimmed.get_or_insert(Trim::OverCap);
            }
        }
        let affordable = (balance - cost) / unit;
        if units > affordable {
            units = affordable.max(0);
            trimmed.get_or_insert(Trim::BalanceShort);
        }
        if units <= 0 {
            continue;
        }
        cost += unit * units;
        claimed += units;
        out.push(Planned {
            line: index,
            menu_item_id: line.menu_item_id.clone().unwrap_or_default(),
            units,
            unit_cost: unit,
            cost: unit * units,
        });
    }
    Plan {
        lines: out,
        cost,
        units: claimed,
        trimmed,
    }
}

/// The lines a REPLAYED sale covered when the strict plan refused it: the
/// sale already happened, so what the till took off stands, as far as it can
/// be priced at all. An ask naming no line, a line past the end, a line with
/// no menu item or a line already covered is dropped; units are clamped to the line and an
/// ask left with none is dropped. No cost: no points move.
pub fn replay_lines(lines: &[Line], asks: &[Ask]) -> Vec<Planned> {
    let mut out: Vec<Planned> = Vec::new();
    for a in asks {
        let Some(index) = a.line else { continue };
        let Some(line) = lines.get(index) else {
            continue;
        };
        let Some(item) = line.menu_item_id.as_deref() else {
            continue;
        };
        if out.iter().any(|p| p.line == index) {
            continue;
        }
        let units = a.units.unwrap_or(1).clamp(0, line.quantity.max(0));
        if units < 1 {
            continue;
        }
        out.push(Planned {
            line: index,
            menu_item_id: item.to_string(),
            units,
            unit_cost: 0,
            cost: 0,
        });
    }
    out
}

pub mod vectors {
    //! Cases for [`plan`](super::plan) in both modes and for
    //! [`replay_lines`](super::replay_lines), generated from the rules as they
    //! moved here. Regenerate deliberately:
    //! `MADAR_REGENERATE_LOYALTY_PLAN_VECTORS=1 cargo test -p madar-loyalty plan_vectors`.

    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct PlanVector {
        pub name: String,
        pub lines: Vec<Line>,
        pub programme: Programme,
        pub asks: Vec<Ask>,
        /// [`Mode::Till`]'s plan.
        pub till: Plan,
        /// [`Mode::Server`]'s verdict.
        pub server: Result<Plan, Refusal>,
        pub replay: Vec<Planned>,
    }

    pub fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors/loyalty_plan_vectors.json")
    }

    const LATTE: &str = "00000000-0000-4000-8000-0000000000a1";
    const MUFFIN: &str = "00000000-0000-4000-8000-0000000000a2";
    const TEA: &str = "00000000-0000-4000-8000-0000000000a3";

    fn item(id: &str, quantity: i64) -> Line {
        Line {
            menu_item_id: Some(id.into()),
            quantity,
            is_staff_drink: false,
        }
    }

    fn no_item(quantity: i64) -> Line {
        Line {
            menu_item_id: None,
            quantity,
            is_staff_drink: false,
        }
    }

    fn staff(id: &str, quantity: i64) -> Line {
        Line {
            is_staff_drink: true,
            ..item(id, quantity)
        }
    }

    fn catalogue(balance: i64) -> Programme {
        Programme {
            rewards: vec![
                Reward {
                    menu_item_id: LATTE.into(),
                    cost: 5,
                },
                Reward {
                    menu_item_id: MUFFIN.into(),
                    cost: 3,
                },
                // A second entry for the latte: the first one wins.
                Reward {
                    menu_item_id: LATTE.into(),
                    cost: 1,
                },
            ],
            any_item: false,
            any_item_cost: 0,
            max_per_order: None,
            balance,
        }
    }

    fn ask(line: usize, units: i64) -> Ask {
        Ask {
            line: Some(line),
            units: Some(units),
        }
    }

    fn case(name: &str, lines: Vec<Line>, programme: Programme, asks: Vec<Ask>) -> PlanVector {
        PlanVector {
            name: name.into(),
            till: plan(&lines, &programme, &asks, Mode::Till).expect("the till never refuses"),
            server: plan(&lines, &programme, &asks, Mode::Server),
            replay: replay_lines(&lines, &asks),
            lines,
            programme,
            asks,
        }
    }

    pub fn generate() -> Vec<PlanVector> {
        let cart = vec![item(LATTE, 3), item(MUFFIN, 2), item(TEA, 1), no_item(1)];
        let any = |balance: i64, cost: i64| Programme {
            any_item: true,
            any_item_cost: cost,
            ..catalogue(balance)
        };
        let capped = |balance: i64, max: i64| Programme {
            max_per_order: Some(max),
            ..catalogue(balance)
        };
        vec![
            case("one_latte", cart.clone(), catalogue(10), vec![ask(0, 1)]),
            case(
                "default_one_unit",
                cart.clone(),
                catalogue(10),
                vec![Ask {
                    line: Some(1),
                    units: None,
                }],
            ),
            case(
                "two_lines_fit",
                cart.clone(),
                catalogue(16),
                vec![ask(0, 2), ask(1, 2)],
            ),
            case(
                "first_catalogue_entry_wins",
                cart.clone(),
                catalogue(5),
                vec![ask(0, 1)],
            ),
            case(
                "balance_short_on_the_second",
                cart.clone(),
                catalogue(12),
                vec![ask(0, 2), ask(1, 2)],
            ),
            case(
                "balance_exactly",
                cart.clone(),
                catalogue(15),
                vec![ask(0, 3)],
            ),
            case(
                "negative_balance",
                cart.clone(),
                catalogue(-4),
                vec![ask(1, 1)],
            ),
            case(
                "more_units_than_the_line",
                cart.clone(),
                catalogue(100),
                vec![ask(1, 3)],
            ),
            case("zero_units", cart.clone(), catalogue(100), vec![ask(0, 0)]),
            case(
                "negative_units",
                cart.clone(),
                catalogue(100),
                vec![ask(0, -2)],
            ),
            case(
                "no_line_named",
                cart.clone(),
                catalogue(100),
                vec![Ask {
                    line: None,
                    units: Some(1),
                }],
            ),
            case(
                "line_past_the_end",
                cart.clone(),
                catalogue(100),
                vec![ask(9, 1), ask(0, 1)],
            ),
            case(
                "twice_on_one_line",
                cart.clone(),
                catalogue(100),
                vec![ask(0, 1), ask(0, 2)],
            ),
            case(
                "a_line_with_no_menu_item",
                cart.clone(),
                catalogue(100),
                vec![ask(3, 1)],
            ),
            case(
                "not_on_offer",
                cart.clone(),
                catalogue(100),
                vec![ask(2, 1), ask(0, 1)],
            ),
            case(
                "any_item_prices_the_tea",
                cart.clone(),
                any(100, 4),
                vec![ask(2, 1)],
            ),
            case(
                "any_item_keeps_the_catalogue_price",
                cart.clone(),
                any(100, 4),
                vec![ask(0, 1)],
            ),
            case(
                "any_item_at_nothing",
                cart.clone(),
                any(100, 0),
                vec![ask(2, 1)],
            ),
            case(
                "listed_at_nothing_is_not_any_item",
                cart.clone(),
                Programme {
                    rewards: vec![Reward {
                        menu_item_id: TEA.into(),
                        cost: 0,
                    }],
                    ..any(100, 4)
                },
                vec![ask(2, 1)],
            ),
            case(
                "cap_counts_units",
                cart.clone(),
                capped(100, 2),
                vec![ask(0, 3)],
            ),
            case(
                "cap_across_lines",
                cart.clone(),
                capped(100, 3),
                vec![ask(0, 2), ask(1, 2)],
            ),
            case(
                "cap_exactly",
                cart.clone(),
                capped(100, 3),
                vec![ask(0, 2), ask(1, 1)],
            ),
            case(
                "cap_zero_is_no_cap",
                cart.clone(),
                capped(100, 0),
                vec![ask(0, 3)],
            ),
            case(
                "staff_drink",
                vec![staff(LATTE, 1), item(MUFFIN, 1)],
                catalogue(100),
                vec![ask(0, 1), ask(1, 1)],
            ),
            case(
                "trim_reasons_keep_the_first",
                cart.clone(),
                capped(9, 2),
                vec![ask(1, 5), ask(0, 2), ask(2, 1)],
            ),
            case("empty", cart.clone(), catalogue(0), vec![]),
            case("empty_cart", vec![], catalogue(10), vec![ask(0, 1)]),
        ]
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn plan_vectors() {
            let generated = generate();
            if std::env::var("MADAR_REGENERATE_LOYALTY_PLAN_VECTORS").is_ok() {
                std::fs::write(
                    fixture_path(),
                    serde_json::to_string_pretty(&generated).unwrap() + "\n",
                )
                .unwrap();
                return;
            }
            let expected: Vec<PlanVector> = serde_json::from_str(crate::vectors::PLAN).unwrap();
            assert_eq!(generated, expected, "the redemption planner drifted");
        }

        /// What the till sends is never refused by the server: the till's
        /// trimmed plan, asked again strictly, passes.
        #[test]
        fn a_trimmed_plan_is_one_the_server_takes() {
            for v in generate() {
                let asks: Vec<Ask> = v
                    .till
                    .lines
                    .iter()
                    .map(|p| Ask {
                        line: Some(p.line),
                        units: Some(p.units),
                    })
                    .collect();
                let lines: Vec<Line> = v
                    .lines
                    .iter()
                    .map(|l| Line {
                        is_staff_drink: false,
                        ..l.clone()
                    })
                    .collect();
                let strict = plan(&lines, &v.programme, &asks, Mode::Server)
                    .unwrap_or_else(|r| panic!("{}: {r:?}", v.name));
                assert_eq!(strict.lines, v.till.lines, "{}", v.name);
                assert_eq!(strict.cost, v.till.cost, "{}", v.name);
            }
        }

        /// A plan the server takes, the till takes unchanged.
        #[test]
        fn a_plan_the_server_takes_is_not_trimmed() {
            for v in generate() {
                if let Ok(strict) = &v.server {
                    if v.lines.iter().any(|l| l.is_staff_drink) {
                        continue;
                    }
                    assert_eq!(v.till.lines, strict.lines, "{}", v.name);
                    assert_eq!(v.till.trimmed, None, "{}", v.name);
                }
            }
        }
    }
}
