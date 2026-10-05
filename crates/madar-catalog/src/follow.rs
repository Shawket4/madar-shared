//! An add-on line follows the drink's own choice.
//!
//! An extra shot on a decaf latte is a decaf shot, and extra milk on an oat
//! latte is oat. An ADDITIVE add-on (`extra`, any option that adds lines
//! rather than swapping one) whose ingredient sits in a swap family takes the
//! ingredient the drink ended up with — the recipe's own, or what a swap put
//! in its place — instead of whatever the catalogue happened to name on the
//! add-on. Without it the sale charges for one thing and deducts another, and
//! a till's recipe card tells the barista to pull the wrong bean.
//!
//! **The server's rule.** Moved from MadarRust `orders/component_resolve.rs`
//! (the "follow the drink's choice" pass after the swaps); the server's
//! results did not change. The POS core had no copy until it ported one into
//! its recipe preview; it now runs this, so the preview, the recipe card and
//! the stock the server deducts are one rule.
//!
//! - Milk and coffee always follow; any other swap family follows only on a
//!   line where one of its choices was made ([`families`]).
//! - The drink's choice for a family is its FIRST non-additive line of that
//!   category, in the caller's order (recipe lines, swapped or not).
//! - Lines are matched by ingredient id only, as the server always has: an
//!   add-on line already naming the chosen ingredient is left alone.
//! - The add-on's quantity is converted into the chosen ingredient's unit
//!   (`madar_units::convert`, rounded to 3 decimals). Across unit families
//!   (pieces vs grams) the line is left as authored and reported.
//!
//! Pure: lines in, lines changed in place plus what happened to each.

use serde::{Deserialize, Serialize};

use crate::price::PricedOptions;

/// The families that always follow, whatever was picked on the line.
pub const ALWAYS: [&str; 2] = ["milk", "coffee_bean"];

/// One ingredient line of a drink as resolved so far: the recipe's lines
/// (with any swap applied) and the additive options' lines.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DrinkLine {
    #[serde(default)]
    pub ingredient_id: Option<String>,
    pub name: String,
    pub unit: String,
    pub quantity: f64,
    /// The ingredient's category slug (`milk`, `coffee_bean`, …); `general`
    /// (or anything that is no family) never follows.
    pub category: String,
    /// An additive option's line — the only kind that follows.
    pub additive: bool,
}

/// What [`follow_the_drink`] did to one line (by its index in the slice).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Followed {
    /// The line now names the drink's choice (`to`), its quantity converted.
    Followed {
        line: usize,
        from: String,
        to: String,
    },
    /// The line should follow `to_name` but its unit cannot become `to_unit`:
    /// left as authored.
    Unconvertible {
        line: usize,
        to_name: String,
        to_unit: String,
    },
}

/// The families that follow on a line priced as `priced`: milk and coffee,
/// then the family of every swap choice made on it (in pick order, once each)
/// — whether or not the choice changed the cup.
pub fn families(priced: &PricedOptions) -> Vec<String> {
    let mut out: Vec<String> = ALWAYS.iter().map(|s| s.to_string()).collect();
    for p in &priced.options {
        if let Some(t) = &p.target {
            if !out.contains(&t.slug) {
                out.push(t.slug.clone());
            }
        }
    }
    out
}

/// Make every additive line of a following family name the drink's choice.
/// `families` is [`families`]'s answer (or any list in the same shape).
pub fn follow_the_drink(lines: &mut [DrinkLine], families: &[String]) -> Vec<Followed> {
    let mut out = Vec::new();
    for cat in families {
        let Some((id, name, unit)) = lines
            .iter()
            .find(|l| &l.category == cat && !l.additive)
            .map(|l| (l.ingredient_id.clone(), l.name.clone(), l.unit.clone()))
        else {
            continue;
        };
        for (i, l) in lines.iter_mut().enumerate() {
            if !l.additive || &l.category != cat || l.ingredient_id == id {
                continue;
            }
            match madar_units::convert(l.quantity, &l.unit, &unit) {
                Ok(q) => {
                    let from = std::mem::replace(&mut l.name, name.clone());
                    l.quantity = q;
                    l.ingredient_id = id.clone();
                    l.unit = unit.clone();
                    out.push(Followed::Followed {
                        line: i,
                        from,
                        to: name.clone(),
                    });
                }
                Err(_) => out.push(Followed::Unconvertible {
                    line: i,
                    to_name: name.clone(),
                    to_unit: unit.clone(),
                }),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectors::{FollowVector, FOLLOW};

    #[test]
    fn the_follow_vectors_hold() {
        let cases: Vec<FollowVector> = serde_json::from_str(FOLLOW).expect("follow_vectors.json");
        assert!(!cases.is_empty());
        for c in cases {
            let mut lines = c.lines.clone();
            let got = follow_the_drink(&mut lines, &c.families);
            assert_eq!(lines, c.expect_lines, "{}: lines", c.name);
            assert_eq!(got, c.expect_followed, "{}: report", c.name);
        }
    }

    #[test]
    fn families_are_milk_and_coffee_then_each_choice_once() {
        use crate::price::{PricedOption, SwapTarget};
        let opt = |slug: &str| PricedOption {
            target: Some(SwapTarget {
                slug: slug.into(),
                category_id: None,
                family: slug.into(),
            }),
            ..Default::default()
        };
        let priced = PricedOptions {
            options: vec![
                opt("coffee_bean"),
                opt("syrup_base"),
                PricedOption::default(),
                opt("syrup_base"),
            ],
            ..Default::default()
        };
        assert_eq!(families(&priced), ["milk", "coffee_bean", "syrup_base"]);
        assert_eq!(families(&PricedOptions::default()), ["milk", "coffee_bean"]);
    }
}
