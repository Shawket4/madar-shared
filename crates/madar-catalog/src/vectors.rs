//! The vector file this crate is pinned by, as bytes, and its shape.
//!
//! `catalog_vectors.json` is written by MadarRust's
//! `tests/catalog_pricing_tests.rs` from the SERVER's behaviour: a fixture
//! catalogue seeded in Postgres, every case priced by the server's order path
//! BEFORE the pricing moved here (the backend keeps that capture as its own
//! pin), the [`CatalogView`] its loader builds, and the feed rows the server
//! serves a till for the same catalogue. The POS core reads the same bytes
//! through its menu mirror.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::price::{PriceError, PricedLine, Selection};
use crate::view::{CatalogView, ItemView, OptionView};

/// `catalog_vectors.json`.
pub const CATALOG: &str = include_str!("../vectors/catalog_vectors.json");

/// `staff_input_vectors.json`: `staff::vectors::StaffInputVector`s.
pub const STAFF_INPUT: &str = include_str!("../vectors/staff_input_vectors.json");

/// `combo_vectors.json` (hand-computed): [`crate::combo::quote`],
/// [`crate::combo::available`], [`crate::combo::resolve_sell`].
pub const COMBO: &str = include_str!("../vectors/combo_vectors.json");

/// `deal_vectors.json` (hand-computed): [`crate::deal`].
pub const DEAL: &str = include_str!("../vectors/deal_vectors.json");

/// `sale_window_vectors.json` (hand-authored): [`crate::sale_window`].
pub const SALE_WINDOW: &str = include_str!("../vectors/sale_window_vectors.json");

/// The vector file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vectors {
    pub about: String,
    /// Every option of the fixture catalogue, as the server's loader builds it.
    pub options: Vec<OptionView>,
    /// The same options as a till receives them: the `addon_item` feed rows.
    pub addon_items: Vec<Value>,
    pub items: Vec<ItemFixture>,
    pub cases: Vec<Case>,
}

/// One fixture item: its view as the server's loader builds it, and its
/// `/menu-items?full=true` row as a till receives it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemFixture {
    pub key: String,
    pub view: ItemView,
    pub menu_item: Value,
}

/// One priced line.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Case {
    pub name: String,
    /// [`ItemFixture::key`].
    pub item: String,
    /// `line` = [`crate::price_line`], the only part since combos were removed
    /// (2026-09-25; `component` priced a combo's component with
    /// [`crate::price_options`]). The field stays so no case's shape moves.
    pub part: String,
    pub selection: Selection,
    pub expected: Expected,
}

/// What the server answered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Expected {
    Line(PricedLine),
    Error(PriceError),
}

impl Vectors {
    pub fn load() -> Self {
        serde_json::from_str(CATALOG).expect("catalog_vectors.json parses")
    }

    pub fn item(&self, key: &str) -> &ItemFixture {
        self.items
            .iter()
            .find(|i| i.key == key)
            .unwrap_or_else(|| panic!("no fixture item {key}"))
    }

    /// The loader's view of the item `key`, with every option.
    pub fn view(&self, key: &str) -> CatalogView {
        CatalogView {
            item: self.item(key).view.clone(),
            options: self.options.clone(),
        }
    }
}

/// What the rule answers for `case` over `view`, in the vector's shape.
pub fn run(view: &CatalogView, case: &Case) -> Expected {
    crate::price_line(view, &case.selection)
        .map(Expected::Line)
        .unwrap_or_else(Expected::Error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_case_prices_as_the_server_did() {
        let v = Vectors::load();
        assert!(v.cases.len() >= 40, "{} cases", v.cases.len());
        for case in &v.cases {
            assert_eq!(
                run(&v.view(&case.item), case),
                case.expected,
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn the_feed_rows_rebuild_the_loaders_view() {
        let v = Vectors::load();
        for item in &v.items {
            let from_feed = crate::feed::view_of(&item.menu_item, &v.addon_items)
                .unwrap_or_else(|| panic!("{}: the feed row has no pricing", item.key));
            assert_eq!(from_feed, v.view(&item.key), "{}", item.key);
        }
    }

    #[test]
    fn the_drift_cases_price_as_the_server_does() {
        // Discovery M4 / M5, as the vectors state them (the till used to
        // charge the figure in the comment).
        let v = Vectors::load();
        let line = |name: &str| match &v.cases.iter().find(|c| c.name == name).unwrap().expected {
            Expected::Line(l) => l.clone(),
            other => panic!("{name}: {other:?}"),
        };
        // M4a: an explicit swap group — green over the recipe's black (was 700).
        assert_eq!(line("tea_cup_green").options.option_total, 400);
        // ... and the recipe's own black costs nothing (was 300).
        assert_eq!(line("tea_cup_black_is_the_recipe").options.option_total, 0);
        // M4b: an option sharing the recipe's milk is the recipe's (was 300).
        assert_eq!(
            line("latte_barista_whole_shares_the_recipe_milk")
                .options
                .option_total,
            0
        );
        // M4c: a swap group picked twice keeps the last pick, once (was 650).
        let c = line("vlatte_two_vanilla_and_a_caramel");
        assert_eq!((c.options.options.len(), c.options.option_total), (1, 50));
        // M5: a sizeless line is the branch's item price (was the lowest size).
        assert_eq!(
            line("latte_no_size_is_the_branch_item_price").unit_price,
            3500
        );
    }
}
