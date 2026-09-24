//! The staff comp's input, built from the catalogue: which sizes and which
//! required choice groups a staff drink's base configuration is judged on.
//!
//! The rule itself is madar-money's (`staff_comp::comp`); this is the one
//! copy of its INPUT builder, moved from MadarRust `staff_pool/order_line.rs`
//! (`comp_input`, which read it with SQL). The POS core built the same input
//! from its menu mirror (`cart.rs`, `StaffCompLine::comp_input`) with rules of
//! its own; it now builds it here from the same view.
//!
//! - **sizes**: only when the line names one (a sizeless line rings at the
//!   item's "from" price, and that price is the free amount): every size the
//!   catalogue has, in display order, with the branch's price beside it — a
//!   label only the branch prices is not a size of the item;
//! - **groups**: the attached groups whose effective minimum is at least one
//!   (a group flagged required with a lower minimum reads as one), never a
//!   swap group (`effect = 'swaps'`, or a legacy milk / coffee type: a swap
//!   pick already rings as the difference over the recipe's own choice, so it
//!   is an extra by construction); options are the attachment's allow-list,
//!   each at its catalogue price with the branch's price beside it, its
//!   default flag and whether it is on; a group left with no option is
//!   dropped;
//! - **picks**, the optionals and the quantity: as the line rang them.

use madar_money::staff_comp::{CompGroup, CompInput, CompOption, CompPick, CompSize};

use crate::view::{GroupView, ItemView};

/// A staff line as it rang, before the catalogue is read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StaffLine {
    pub size_label: Option<String>,
    /// The pool decision allowed the line.
    pub eligible: bool,
    /// What one unit of the chosen size rang at, before any pick.
    pub unit_price: i32,
    pub picks: Vec<CompPick>,
    /// Priced optional fields per unit (never free).
    pub optionals_per_unit: i32,
    pub quantity: i32,
}

/// A catalogue figure as the rule's `i32`, saturating (the server's figures
/// are `integer` columns, so this is the identity there).
fn m(v: i64) -> i32 {
    i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX })
}

/// Legacy add-on types that are swap families (milk, beans).
pub const SWAP_LEGACY_TYPES: [&str; 2] = ["milk_type", "coffee_type"];

/// Whether a group can be the base of a staff drink: not a swap group.
pub fn is_comp_group(g: &GroupView) -> bool {
    g.effect != "swaps"
        && !g
            .legacy_type
            .as_deref()
            .is_some_and(|t| SWAP_LEGACY_TYPES.contains(&t))
}

/// The minimum a group's picks must reach: the effective minimum, or one for
/// a group flagged required with a lower one.
pub fn required_min(g: &GroupView) -> i64 {
    if g.min >= 1 {
        g.min
    } else {
        i64::from(g.is_required)
    }
}

/// The comp groups of `groups`, as the rule reads them.
pub fn comp_groups(groups: &[GroupView]) -> Vec<CompGroup> {
    groups
        .iter()
        .filter(|g| is_comp_group(g))
        .filter_map(|g| {
            let min = required_min(g);
            if min < 1 {
                return None;
            }
            let options: Vec<CompOption> = g
                .options
                .iter()
                .filter(|o| g.included.as_ref().is_none_or(|ids| ids.contains(&o.id)))
                .map(|o| CompOption {
                    id: o.id.clone(),
                    price: m(o.price),
                    branch_price: o.branch_price.map(m),
                    is_default: o.is_default,
                    is_active: o.is_active,
                })
                .collect();
            (!options.is_empty()).then(|| CompGroup {
                id: g.id.clone(),
                required_min: m(min),
                options,
            })
        })
        .collect()
}

/// The comp rule's input for `line` of `item`.
pub fn comp_input(item: &ItemView, line: &StaffLine) -> CompInput {
    let sizes = if line.size_label.is_some() {
        item.sizes
            .iter()
            .filter_map(|z| {
                Some(CompSize {
                    label: z.label.clone(),
                    price: m(z.price?),
                    is_active: z.is_active,
                    branch_price: z.branch_price.map(m),
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    CompInput {
        eligible: line.eligible,
        unit_price: line.unit_price,
        sizes,
        groups: comp_groups(&item.groups),
        picks: line.picks.clone(),
        optionals_per_unit: line.optionals_per_unit,
        quantity: line.quantity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::{GroupOption, SizeView};

    fn opt(id: &str, price: i64, default: bool) -> GroupOption {
        GroupOption {
            id: id.into(),
            price,
            branch_price: None,
            is_default: default,
            is_active: true,
        }
    }

    fn group(id: &str, min: i64, required: bool, effect: &str) -> GroupView {
        GroupView {
            id: id.into(),
            min,
            is_required: required,
            effect: effect.into(),
            legacy_type: None,
            included: None,
            options: vec![
                opt(&format!("{id}-a"), 500, true),
                opt(&format!("{id}-b"), 300, false),
            ],
        }
    }

    fn item() -> ItemView {
        ItemView {
            id: "i".into(),
            sizes: vec![
                SizeView {
                    label: "S".into(),
                    price: Some(4000),
                    is_active: true,
                    branch_price: Some(3800),
                },
                SizeView {
                    label: "Branch only".into(),
                    price: None,
                    is_active: false,
                    branch_price: Some(9000),
                },
            ],
            groups: vec![
                group("required", 1, false, "adds"),
                group("flagged", 0, true, "none"),
                group("optional", 0, false, "adds"),
                group("swap", 1, true, "swaps"),
                GroupView {
                    legacy_type: Some("milk_type".into()),
                    ..group("legacy-milk", 1, true, "adds")
                },
                GroupView {
                    included: Some(vec!["listed-b".into()]),
                    ..group("listed", 2, true, "adds")
                },
                GroupView {
                    included: Some(vec![]),
                    ..group("emptied", 1, true, "adds")
                },
            ],
            ..Default::default()
        }
    }

    fn line(size: Option<&str>) -> StaffLine {
        StaffLine {
            size_label: size.map(str::to_string),
            eligible: true,
            unit_price: 3800,
            picks: vec![],
            optionals_per_unit: 0,
            quantity: 1,
        }
    }

    #[test]
    fn required_non_swap_groups_with_their_allow_lists() {
        let input = comp_input(&item(), &line(Some("S")));
        let ids: Vec<&str> = input.groups.iter().map(|g| g.id.as_str()).collect();
        assert_eq!(ids, ["required", "flagged", "listed"]);
        assert_eq!(input.groups[1].required_min, 1);
        assert_eq!(input.groups[2].required_min, 2);
        let listed: Vec<&str> = input.groups[2]
            .options
            .iter()
            .map(|o| o.id.as_str())
            .collect();
        assert_eq!(listed, ["listed-b"]);
    }

    #[test]
    fn sizes_only_for_a_sized_line_and_only_catalogue_sizes() {
        let sized = comp_input(&item(), &line(Some("S")));
        assert_eq!(sized.sizes.len(), 1);
        assert_eq!(sized.sizes[0].branch_price, Some(3800));
        assert!(comp_input(&item(), &line(None)).sizes.is_empty());
    }
}
