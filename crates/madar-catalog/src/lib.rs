//! # madar-catalog
//!
//! How one sale line is priced from the catalogue: the size price, each
//! picked option — a swap charged as the difference over the recipe's own
//! choice (floored at 0; the recipe's own choice costs nothing), or an add-on
//! at its price — and the optional fields.
//!
//! **The server's rule.** Moved from MadarRust `orders/handlers.rs`
//! (`catalog_unit_price`) and `orders/component_resolve.rs` (`swap_target`,
//! `collapse_families`, `merge_sized_option_lines`, the pricing half of
//! `resolve_menu_item_configuration`); the server's results did not change.
//! The POS core priced options with its own copy (two hard-coded swap
//! families, the base found by ingredient alone, the lowest size for a
//! sizeless line) and disagreed with the server on explicit swap groups, two
//! options sharing the base ingredient, a multi-select swap group, a size
//! the item no longer sells and a branch's item price. It now runs this.
//!
//! - [`view`]: what the rule reads ([`CatalogView`]); the server builds it
//!   from SQL, the core from its menu mirror ([`feed`]).
//! - [`price`]: [`price_line`], [`price_options`] (a line's options alone),
//!   [`unit_price`], the display helpers [`option_charge`] and
//!   [`is_recipe_choice`], and the pieces the server's stock deduction shares
//!   ([`swap_target`], [`merge_sized_lines`]).
//! - [`staff`]: the staff comp's input built from the same view (the sizes
//!   and required groups a staff drink's base is judged on).
//!
//! - [`combo`] (v0.5): a combo line — the slots, the picks, the split of the
//!   combo's price over its parts ([`combo::quote`]), availability on a
//!   channel at a branch, and the org/branch channel switches.
//! - [`deal`] (v0.5): deal rules (N for a price, buy X get Y) over a cart —
//!   the till's suggestions, the server's QR/online auto-apply, and the
//!   pricing of an application both sides run.
//! - [`sale_window`] (v0.5): the optional weekday / time / date windows of a
//!   combo or a deal.
//!
//! Pinned by `vectors/catalog_vectors.json`, generated from the server's
//! behaviour (MadarRust `tests/catalog_pricing_tests.rs`); the combo, deal
//! and window rules by hand-computed `vectors/{combo,deal,sale_window}_vectors.json`.

pub mod combo;
pub mod deal;
pub mod feed;
pub mod price;
pub mod sale_window;
pub mod staff;
pub mod vectors;
pub mod view;

pub use price::{
    collapse_families, is_recipe_choice, merge_sized_lines, option_charge, option_lines,
    price_line, price_options, recipe_size, swap_target, target_of, unit_price, Note, Over, Pick,
    PriceError, PricedLine, PricedOption, PricedOptional, PricedOptions, Replacement, Selection,
    SwapTarget,
};
pub use view::{
    BaseCandidate, BaseCandidates, CatalogView, GroupOption, GroupView, IngredientLine,
    IngredientRef, ItemView, OptionView, OptionalView, RecipeLine, SizeView, SizedLine,
};
