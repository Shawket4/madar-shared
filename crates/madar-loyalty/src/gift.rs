//! The birthday gift: which line of a sale may take it (Madar
//! `BIRTHDAY_GIFT_DESIGN.md`).
//!
//! A gift covers ONE unit of one line, whatever that item costs, and spends
//! no balance. Moved from MadarRust `loyalty::gifts::plan` (the server's claim
//! check) and madar-core's `gift_eligible` (the till's board), which must
//! never disagree:
//!
//! - the line exists, has a menu item and at least one unit;
//! - it is not a staff drink, not inside a combo, and not a line a points
//!   reward already covers ([`GiftRefusal::Line`]);
//! - the item is one of the programme's rewards, or any item when the
//!   programme rewards any item ([`GiftRefusal::NotAReward`]);
//! - the member holds an open gift ([`GiftRefusal::None`]).
//!
//! The codes are the server's 409 codes ([`GiftRefusal::code`]). Pinned by
//! `vectors/loyalty_gift_vectors.json`.

use serde::{Deserialize, Serialize};

use crate::plan::Line;

/// What the gift may be taken as at this branch, and whether there is one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GiftProgramme {
    /// The programme's reward items (the catalogue, whatever each costs).
    #[serde(default)]
    pub reward_items: Vec<String>,
    /// "Any item" is a reward here: every line with a menu item qualifies.
    #[serde(default)]
    pub any_item: bool,
    /// The member holds an open, unexpired gift.
    #[serde(default)]
    pub open: bool,
}

/// Why the chosen line cannot take the gift.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GiftRefusal {
    /// No such line, no menu item, no unit, a staff drink, a combo, or a line
    /// a points reward covers.
    Line,
    /// The item is not one of the rewards and "any item" is off.
    NotAReward,
    /// No open gift: none granted, already claimed, or expired.
    None,
}

impl GiftRefusal {
    /// The server's refusal code.
    pub fn code(self) -> &'static str {
        match self {
            GiftRefusal::Line => "LOYALTY_GIFT_LINE",
            GiftRefusal::NotAReward => "LOYALTY_GIFT_NOT_A_REWARD",
            GiftRefusal::None => "LOYALTY_GIFT_NONE",
        }
    }
}

/// The gift a sale takes: one unit of `line`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GiftCover {
    pub line: usize,
    pub menu_item_id: String,
    pub units: i64,
}

/// Whether `line` could be the gift, line rules and item rule only (no open
/// gift needed): the till's board and the lookup's list.
pub fn gift_eligible(line: &Line, rewarded: bool, programme: &GiftProgramme) -> bool {
    line_refusal(line, rewarded, programme).is_none()
}

fn line_refusal(line: &Line, rewarded: bool, programme: &GiftProgramme) -> Option<GiftRefusal> {
    let Some(item) = line.menu_item_id.as_deref() else {
        return Some(GiftRefusal::Line);
    };
    if line.quantity < 1 || line.is_staff_drink || line.in_combo || rewarded {
        return Some(GiftRefusal::Line);
    }
    if !programme.any_item && !programme.reward_items.iter().any(|r| r == item) {
        return Some(GiftRefusal::NotAReward);
    }
    None
}

/// The gift on `chosen` (a line index), or why not. `rewarded` lists the line
/// indices a points reward covers. Checked in the server's order: the line,
/// then the item, then the gift itself.
pub fn plan_gift(
    lines: &[Line],
    programme: &GiftProgramme,
    chosen: Option<usize>,
    rewarded: &[usize],
) -> Result<GiftCover, GiftRefusal> {
    let i = chosen.ok_or(GiftRefusal::Line)?;
    let line = lines.get(i).ok_or(GiftRefusal::Line)?;
    if let Some(r) = line_refusal(line, rewarded.contains(&i), programme) {
        return Err(r);
    }
    if !programme.open {
        return Err(GiftRefusal::None);
    }
    Ok(GiftCover {
        line: i,
        menu_item_id: line.menu_item_id.clone().unwrap_or_default(),
        units: 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `loyalty_gift_vectors.json`: hand-written carts and answers. Rerun with
    /// `MADAR_REGENERATE_GIFT_VECTORS=1` only to re-serialise; every answer
    /// was written by hand against its `why`.
    #[test]
    fn gift_vectors() {
        let original: serde_json::Value = serde_json::from_str(crate::vectors::GIFT).unwrap();
        let mut got = original.clone();
        for c in got["cases"].as_array_mut().unwrap() {
            let lines: Vec<Line> = serde_json::from_value(c["lines"].clone()).unwrap();
            let p: GiftProgramme = serde_json::from_value(c["programme"].clone()).unwrap();
            let chosen: Option<usize> = serde_json::from_value(c["chosen"].clone()).unwrap();
            let rewarded: Vec<usize> = serde_json::from_value(c["rewarded"].clone()).unwrap();
            c["result"] = match plan_gift(&lines, &p, chosen, &rewarded) {
                Ok(cover) => serde_json::json!({ "ok": cover }),
                Err(r) => serde_json::json!({ "refused": r.code() }),
            };
            c["eligible"] = serde_json::to_value(
                lines
                    .iter()
                    .enumerate()
                    .map(|(i, l)| gift_eligible(l, rewarded.contains(&i), &p))
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        }
        if std::env::var("MADAR_REGENERATE_GIFT_VECTORS").is_ok() {
            let path = format!(
                "{}/vectors/loyalty_gift_vectors.json",
                env!("CARGO_MANIFEST_DIR")
            );
            std::fs::write(path, serde_json::to_string_pretty(&got).unwrap() + "\n").unwrap();
        }
        assert_eq!(got, original, "a gift vector's answer changed");
    }

    #[test]
    fn the_codes_are_the_servers() {
        assert_eq!(GiftRefusal::None.code(), "LOYALTY_GIFT_NONE");
        assert_eq!(GiftRefusal::NotAReward.code(), "LOYALTY_GIFT_NOT_A_REWARD");
        assert_eq!(GiftRefusal::Line.code(), "LOYALTY_GIFT_LINE");
    }
}
