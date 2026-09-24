//! What the pricing rule reads: one menu item as a branch sells it, and the
//! options a line may pick.
//!
//! The server builds a [`CatalogView`] from SQL (a few batched queries per
//! order); the POS core builds the same view from its menu mirror, where the
//! server ships the two halves ready-made — [`ItemView`] as the `pricing`
//! field of every `/menu-items?full=true` row and [`OptionView`] as the
//! `pricing` field of every add-on row (`/addon-items` and the `addon_item`
//! feed rows). See [`crate::feed`].
//!
//! Every list is in the order the server's SQL returns it. Where that order
//! is a text collation (a name, a size label) the server computed it; the
//! rule never re-sorts by name, so the view carries the order rather than a
//! Rust approximation of Postgres' `en_US` collation.

use serde::{Deserialize, Serialize};

/// A menu item and the options a line of it may pick.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogView {
    pub item: ItemView,
    /// The options (add-on items) a selection may name. Extra entries are
    /// harmless; a picked id that is not here is [`crate::PriceError::UnknownOption`].
    #[serde(default)]
    pub options: Vec<OptionView>,
}

impl CatalogView {
    /// The option with this id.
    pub fn option(&self, id: &str) -> Option<&OptionView> {
        self.options.iter().find(|o| o.id == id)
    }
}

/// One menu item, priced for one branch.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemView {
    pub id: String,
    /// The branch's item-level price override (`branch_menu_overrides`):
    /// what a line with no size costs, and the fallback for a size the item
    /// no longer sells.
    #[serde(default)]
    pub branch_price: Option<i64>,
    /// Every size the item has, and every size label the branch overrides,
    /// in the item's display order (sort, then label).
    #[serde(default)]
    pub sizes: Vec<SizeView>,
    /// The recipe size a line with no size is made from: the item's first
    /// size (active first, then sort, then label) among the recipe's own
    /// labels. `None` = no recipe at all (the server then reads `one_size`).
    #[serde(default)]
    pub default_recipe_size: Option<String>,
    /// The recipe's lines, per size, in the server's order (size label, then
    /// ingredient name). Only what the rule reads: the ingredient and its
    /// category slug.
    #[serde(default)]
    pub recipe: Vec<RecipeLine>,
    /// For each ingredient of the recipe: the options that carry it (as an
    /// ingredient line, or as the ingredient they replace), in the server's
    /// order (active first, then sort — missing last — then name, then id).
    /// A swap is charged over the first candidate of the chosen option's
    /// family, the chosen option's own group first.
    #[serde(default)]
    pub bases: Vec<BaseCandidates>,
    /// The item's active optional fields.
    #[serde(default)]
    pub optionals: Vec<OptionalView>,
    /// The choice groups attached to the item (active groups only), in the
    /// server's order (attachment sort, then group name, then id; options by
    /// sort, then name, then id). Read by the staff comp's input builder
    /// ([`crate::staff`]), not by the pricing rule. Empty from a server older
    /// than v0.4.0, which did not send it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<GroupView>,
}

/// A choice group attached to an item (`menu_item_modifier_groups` over an
/// active `modifier_groups` row), with the attachment's overrides resolved.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupView {
    pub id: String,
    /// `COALESCE(min_override, min_selections)`.
    #[serde(default)]
    pub min: i64,
    /// `COALESCE(is_required_override, is_required)`.
    #[serde(default)]
    pub is_required: bool,
    /// `none` | `adds` | `swaps`.
    #[serde(default)]
    pub effect: String,
    /// The legacy add-on type the group was made from (`milk_type`, …).
    #[serde(default)]
    pub legacy_type: Option<String>,
    /// The attachment's allow-list; `None` offers every option.
    #[serde(default)]
    pub included: Option<Vec<String>>,
    #[serde(default)]
    pub options: Vec<GroupOption>,
}

/// One option of a choice group, priced for the branch.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupOption {
    pub id: String,
    /// The catalogue price (`addon_items.default_price`).
    pub price: i64,
    /// The branch's price (`branch_addon_overrides.price_override`).
    #[serde(default)]
    pub branch_price: Option<i64>,
    #[serde(default)]
    pub is_default: bool,
    /// The option, its add-on item and the branch's availability all on.
    #[serde(default)]
    pub is_active: bool,
}

/// A size and what it costs at the branch.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SizeView {
    pub label: String,
    /// The catalogue price (`menu_item_sizes.price`); `None` when only a
    /// branch override names this label.
    #[serde(default)]
    pub price: Option<i64>,
    #[serde(default)]
    pub is_active: bool,
    /// The branch's price for this size (`branch_menu_size_overrides`).
    #[serde(default)]
    pub branch_price: Option<i64>,
}

/// One recipe line, as far as the swap rule reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeLine {
    pub size_label: String,
    /// The ingredient's category slug (`milk`, `coffee_bean`, …); `None`
    /// for a line with no ingredient.
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub ingredient_id: Option<String>,
}

/// The options carrying one recipe ingredient, in the server's order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseCandidates {
    pub ingredient_id: String,
    #[serde(default)]
    pub candidates: Vec<BaseCandidate>,
}

/// An option a swap may be charged over.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseCandidate {
    pub option_id: String,
    pub name: String,
    /// The add-on type (`milk_type`, `coffee_type`, `extra`, …).
    pub kind: String,
    /// Its branch-effective price.
    pub price: i64,
    #[serde(default)]
    pub group_id: Option<String>,
    /// Its group's swap category (whatever the group's effect).
    #[serde(default)]
    pub swap_category_id: Option<String>,
}

/// An option (add-on item) as the rule reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionView {
    pub id: String,
    pub name: String,
    /// The add-on type (`milk_type`, `coffee_type`, `extra`, …).
    pub kind: String,
    /// The branch-effective price.
    pub price: i64,
    #[serde(default)]
    pub group_id: Option<String>,
    /// The group's effect (`none` | `adds` | `swaps`); `None` when the
    /// option belongs to no group.
    #[serde(default)]
    pub effect: Option<String>,
    #[serde(default)]
    pub swap_category_id: Option<String>,
    #[serde(default)]
    pub swap_category_slug: Option<String>,
    /// The ingredient an explicit swap option names as its replacement.
    #[serde(default)]
    pub replaces: Option<IngredientRef>,
    /// Its ingredient lines, in the server's order (ingredient name, then id).
    #[serde(default)]
    pub ingredients: Vec<IngredientLine>,
    /// Per-size ingredient lines (menu modeling B9).
    #[serde(default)]
    pub sized: Vec<SizedLine>,
}

/// An ingredient an option names.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngredientRef {
    pub id: String,
    pub name: String,
    pub unit: String,
}

/// One ingredient line of an option (`addon_item_ingredients`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngredientLine {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub unit: String,
}

/// One per-size ingredient line of an option (`recipe_lines`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SizedLine {
    pub size_label: String,
    pub id: String,
    pub name: String,
    pub unit: String,
}

/// An optional field of the item.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionalView {
    pub id: String,
    pub price: i64,
    /// Offered on this size only.
    #[serde(default)]
    pub size_label: Option<String>,
}
