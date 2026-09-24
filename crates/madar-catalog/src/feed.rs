//! The POS core's side: the view from the menu mirror.
//!
//! The server ships both halves of a [`CatalogView`] ready-made, so the core
//! never re-derives an order or a join: every `/menu-items?full=true` row
//! carries its [`ItemView`] as `pricing`, and every add-on row (`/addon-items`,
//! the `addon_item` feed rows) its [`OptionView`] as `pricing`. A row from a
//! server that predates the field has none; the caller decides what to do
//! then (the core falls back to its legacy projection).

use serde_json::Value;

use crate::view::{CatalogView, ItemView, OptionView};

/// The `pricing` of one `/menu-items?full=true` row.
pub fn item_of(row: &Value) -> Option<ItemView> {
    serde_json::from_value(row.get("pricing")?.clone()).ok()
}

/// The `pricing` of one add-on row.
pub fn option_of(row: &Value) -> Option<OptionView> {
    serde_json::from_value(row.get("pricing")?.clone()).ok()
}

/// The view for `item_row` over every add-on row that carries its `pricing`.
/// `None` when the item row has none (an older server).
pub fn view_of(item_row: &Value, addon_rows: &[Value]) -> Option<CatalogView> {
    let options: Vec<OptionView> = addon_rows.iter().filter_map(option_of).collect();
    Some(CatalogView {
        item: with_fresh_candidates(item_of(item_row)?, &options),
        options,
    })
}

/// A swap's base candidates as the add-on rows state them now.
///
/// The item row names each candidate with its price, but a till refreshes its
/// menu rows only when the catalogue's revision moves, and a branch's add-on
/// price does not move it — the add-on's own feed row does. So a candidate
/// the till also holds as an add-on row takes that row's figures; one it does
/// not hold (switched off at the branch) keeps the item row's.
pub fn with_fresh_candidates(mut item: ItemView, options: &[OptionView]) -> ItemView {
    for c in item.bases.iter_mut().flat_map(|b| b.candidates.iter_mut()) {
        if let Some(o) = options.iter().find(|o| o.id == c.option_id) {
            c.name = o.name.clone();
            c.kind = o.kind.clone();
            c.price = o.price;
            c.group_id = o.group_id.clone();
            c.swap_category_id = o.swap_category_id.clone();
        }
    }
    item
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectors::Vectors;

    #[test]
    fn a_candidate_takes_the_add_on_rows_price() {
        // The branch re-priced the recipe's whole milk after the till last
        // fetched its menu rows: the add-on row (the feed) wins.
        let v = Vectors::load();
        let mut rows = v.addon_items.clone();
        let whole = "c0de0000-0000-4000-8000-000000000050";
        let row = rows.iter_mut().find(|r| r["id"] == whole).unwrap();
        row["pricing"]["price"] = serde_json::json!(120);
        let view = view_of(&v.item("latte").menu_item, &rows).unwrap();
        let milk = view
            .item
            .bases
            .iter()
            .flat_map(|b| &b.candidates)
            .find(|c| c.option_id == whole)
            .unwrap();
        assert_eq!(milk.price, 120);
        let oat =
            crate::option_charge(&view, Some("Small"), "c0de0000-0000-4000-8000-000000000052");
        assert_eq!(oat, Some(800 - 120));
    }

    #[test]
    fn a_row_without_pricing_is_an_older_server() {
        let v = Vectors::load();
        let mut row = v.item("latte").menu_item.clone();
        row.as_object_mut().unwrap().remove("pricing");
        assert!(view_of(&row, &v.addon_items).is_none());
    }
}
