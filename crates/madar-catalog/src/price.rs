//! The rule: what one line of a menu item costs.
//!
//! Moved from MadarRust — `orders/handlers.rs` `catalog_unit_price` (the size
//! price) and `orders/component_resolve.rs` (`swap_target`,
//! `collapse_families`, `merge_sized_option_lines` and the pricing half of
//! `resolve_menu_item_configuration`). The server's results are the
//! reference; the POS core's own swap families (`SWAP_FAMILIES`,
//! `adjusted_addon_price`, `swap_base_addon`) are gone.

use serde::{Deserialize, Serialize};

use crate::view::{CatalogView, IngredientLine, ItemView, OptionView};

/// A line as the till or the order payload states it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    #[serde(default)]
    pub size_label: Option<String>,
    /// The picked options (add-on items), in the order they were picked.
    #[serde(default)]
    pub options: Vec<Pick>,
    /// The picked optional fields. A repeated id is charged again, as the
    /// server does; the till never sends one twice.
    #[serde(default)]
    pub optionals: Vec<String>,
}

/// One picked option.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pick {
    pub id: String,
    #[serde(default = "one")]
    pub quantity: i64,
}

fn one() -> i64 {
    1
}

/// Why a line cannot be priced. The server answers the first with a 400 and
/// the second with a 404; the till drops an unknown option before it asks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum PriceError {
    /// The item has no active size with a price.
    NoPricedSize,
    /// A picked option is not in the catalogue.
    UnknownOption { id: String },
}

impl core::fmt::Display for PriceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoPricedSize => f.write_str("the item has no priced size"),
            Self::UnknownOption { id } => write!(f, "option {id} not found"),
        }
    }
}

/// A priced line.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricedLine {
    /// One unit of the item at its size, before any option.
    pub unit_price: i64,
    #[serde(flatten)]
    pub options: PricedOptions,
}

impl PricedLine {
    /// One unit with every option and optional field.
    pub fn per_unit(&self) -> i64 {
        self.unit_price + self.options.option_total + self.options.optional_total
    }
}

/// A line's options and optional fields, priced.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricedOptions {
    /// The options as charged: one of each swap family (the last pick), in
    /// the order they were picked.
    pub options: Vec<PricedOption>,
    pub optionals: Vec<PricedOptional>,
    /// Σ option price × quantity, per unit of the line.
    pub option_total: i64,
    /// Σ optional-field price, per unit of the line.
    pub optional_total: i64,
    /// What the rule set aside or dropped, for a log or a preview.
    #[serde(default)]
    pub notes: Vec<Note>,
}

/// One option as charged.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricedOption {
    pub id: String,
    /// The quantity charged (at least 1; a swap-family pick beside another
    /// pick is always 1).
    pub quantity: i64,
    /// The charge per unit of this option.
    pub unit_price: i64,
    /// The option replaced part of the recipe (a swap, or the recipe's own
    /// choice) rather than adding to it.
    pub is_swap: bool,
    /// It is the recipe's own ingredient: charged nothing.
    pub is_base: bool,
    /// An add-on that carries ingredient lines of its own.
    pub has_ingredients: bool,
    /// What it swaps: set for every option of a swap family.
    #[serde(default)]
    pub target: Option<SwapTarget>,
    /// The recipe ingredient it would replace, when the recipe has one.
    #[serde(default)]
    pub base_ingredient: Option<String>,
    /// The ingredient it swaps in.
    #[serde(default)]
    pub replacement: Option<Replacement>,
    /// The option a swap is charged over (the recipe's own choice).
    #[serde(default)]
    pub over: Option<Over>,
}

/// The ingredient a swap puts in the cup.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replacement {
    pub id: Option<String>,
    pub name: String,
    pub unit: String,
}

/// The option a swap is charged over.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Over {
    pub id: String,
    pub name: String,
    pub price: i64,
}

/// One optional field as charged.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricedOptional {
    pub id: String,
    pub price: i64,
}

/// Something the rule set aside.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "note", rename_all = "snake_case")]
pub enum Note {
    /// Two picks of one swap family: the last was kept, at quantity 1.
    CollapsedFamily,
    /// An optional field that is not an active option of this item.
    OptionalNotFound { id: String },
    /// An optional field offered on another size only.
    OptionalSizeMismatch { id: String, size_label: String },
}

/// How one option relates to the drink's recipe.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwapTarget {
    /// Ingredient category slug of the recipe line the choice replaces.
    pub slug: String,
    /// The group's explicit swap category; `None` = inferred from the type.
    #[serde(default)]
    pub category_id: Option<String>,
    /// Family key: two choices with the same key cannot share a line.
    pub family: String,
}

/// Explicit (`effect = 'swaps'` + a swap category) wins; otherwise the
/// inference from the legacy type (`milk_type` → `milk`, `coffee_type` →
/// `coffee_bean`). The magic families keep their type as the key so an
/// explicit milk group and an inferred milk group still collapse together.
pub fn swap_target(
    kind: Option<&str>,
    effect: Option<&str>,
    swap_category_id: Option<&str>,
    swap_category_slug: Option<&str>,
) -> Option<SwapTarget> {
    let family_of = |slug: &str, fallback: String| match slug {
        "milk" => "milk_type".to_string(),
        "coffee_bean" => "coffee_type".to_string(),
        _ => fallback,
    };
    if effect == Some("swaps") {
        if let (Some(cid), Some(slug)) = (swap_category_id, swap_category_slug) {
            return Some(SwapTarget {
                slug: slug.to_string(),
                category_id: Some(cid.to_string()),
                family: family_of(slug, format!("category:{cid}")),
            });
        }
    }
    let slug = match kind {
        Some("milk_type") => "milk",
        Some("coffee_type") => "coffee_bean",
        _ => return None,
    };
    Some(SwapTarget {
        slug: slug.to_string(),
        category_id: None,
        family: family_of(slug, String::new()),
    })
}

/// The swap target of an option.
pub fn target_of(o: &OptionView) -> Option<SwapTarget> {
    swap_target(
        Some(&o.kind),
        o.effect.as_deref(),
        o.swap_category_id.as_deref(),
        o.swap_category_slug.as_deref(),
    )
}

/// A drink has ONE milk and ONE coffee: a swap-family option REPLACES the
/// recipe's ingredient, so two of one family on a line cannot be made,
/// costed or deducted. Tills in the field sent such lines, so the line is not
/// refused — the LAST choice of each family wins. `true` = keep.
pub fn collapse_families(families: &[Option<String>]) -> Vec<bool> {
    let mut keep = vec![true; families.len()];
    for i in 0..families.len() {
        if let Some(f) = &families[i] {
            if families[i + 1..].iter().any(|g| g.as_ref() == Some(f)) {
                keep[i] = false;
            }
        }
    }
    keep
}

/// Per-size option lines (menu modeling B9): a sized line REPLACES the
/// generic line for the same ingredient, and a sized line for an ingredient
/// with no generic line is added. With no sized lines nothing changes;
/// otherwise the result is ordered by name (bytes), then ingredient id (a
/// line without one first) — the server's order, which a multi-line swap
/// option's first line depends on.
pub fn merge_sized_lines<T: Clone, K: Ord>(
    generic: Vec<T>,
    sized: Vec<T>,
    id: impl Fn(&T) -> Option<K>,
    name: impl Fn(&T) -> &str,
) -> Vec<T> {
    if sized.is_empty() {
        return generic;
    }
    let same = |a: &T, b: &T| {
        let ia = id(a);
        ia.is_some() && ia == id(b)
    };
    let mut out: Vec<T> = generic
        .into_iter()
        .map(|g| match sized.iter().find(|s| same(s, &g)) {
            Some(s) => s.clone(),
            None => g,
        })
        .collect();
    for s in sized {
        if !out.iter().any(|o| same(o, &s)) {
            out.push(s);
        }
    }
    out.sort_by(|a, b| name(a).cmp(name(b)).then_with(|| id(a).cmp(&id(b))));
    out
}

/// The option's ingredient lines for a line of `size_label`.
pub fn option_lines(o: &OptionView, size_label: Option<&str>) -> Vec<IngredientLine> {
    let Some(label) = size_label else {
        return o.ingredients.clone();
    };
    let sized: Vec<IngredientLine> = o
        .sized
        .iter()
        .filter(|s| s.size_label == label)
        .map(|s| IngredientLine {
            id: Some(s.id.clone()),
            name: s.name.clone(),
            unit: s.unit.clone(),
        })
        .collect();
    merge_sized_lines(
        o.ingredients.clone(),
        sized,
        |l| l.id.clone(),
        |l| l.name.as_str(),
    )
}

/// The recipe size a line is made from: its own size, else the item's
/// default recipe size, else `one_size`.
pub fn recipe_size<'a>(item: &'a ItemView, size_label: Option<&'a str>) -> &'a str {
    size_label
        .or(item.default_recipe_size.as_deref())
        .unwrap_or("one_size")
}

/// One unit of the item at `size_label`, before any option.
///
/// - The branch's price for that size, else the size's catalogue price when
///   the size is active, else the fallback;
/// - with no size: the fallback — the branch's item price, else the lowest
///   active size price (the "from" price).
///
/// An item without an active priced size cannot be sold, whatever the line
/// names.
pub fn unit_price(item: &ItemView, size_label: Option<&str>) -> Result<i64, PriceError> {
    let lowest = item
        .sizes
        .iter()
        .filter(|s| s.is_active)
        .filter_map(|s| s.price)
        .min()
        .ok_or(PriceError::NoPricedSize)?;
    let fallback = item.branch_price.unwrap_or(lowest);
    Ok(match size_label {
        Some(label) => item
            .sizes
            .iter()
            .find(|s| s.label == label)
            .and_then(|s| s.branch_price)
            .or_else(|| {
                item.sizes
                    .iter()
                    .find(|s| s.label == label && s.is_active)
                    .and_then(|s| s.price)
            })
            .unwrap_or(fallback),
        None => fallback,
    })
}

/// A menu-item line: its size price, then its options and optional fields.
pub fn price_line(view: &CatalogView, selection: &Selection) -> Result<PricedLine, PriceError> {
    let unit_price = unit_price(&view.item, selection.size_label.as_deref())?;
    Ok(PricedLine {
        unit_price,
        options: price_options(view, selection)?,
    })
}

/// The options and optional fields of a line, priced without its size.
pub fn price_options(
    view: &CatalogView,
    selection: &Selection,
) -> Result<PricedOptions, PriceError> {
    let size = selection.size_label.as_deref();
    let mut notes = Vec::new();

    // One choice per swap family: the last, at quantity 1 (only when the line
    // picks more than one option, as the server does).
    let picks: Vec<Pick> = if selection.options.len() < 2 {
        selection.options.clone()
    } else {
        let families: Vec<Option<String>> = selection
            .options
            .iter()
            .map(|p| view.option(&p.id).and_then(target_of).map(|t| t.family))
            .collect();
        let keep = collapse_families(&families);
        if keep.iter().any(|k| !k) {
            notes.push(Note::CollapsedFamily);
        }
        selection
            .options
            .iter()
            .zip(&families)
            .zip(keep)
            .filter(|(_, k)| *k)
            .map(|((p, f), _)| Pick {
                id: p.id.clone(),
                quantity: if f.is_some() { 1 } else { p.quantity },
            })
            .collect()
    };

    let recipe_size = recipe_size(&view.item, size);
    let base_lines: Vec<_> = view
        .item
        .recipe
        .iter()
        .filter(|r| r.size_label == recipe_size)
        .collect();
    // Categories whose recipe lines a swap has already replaced: a later swap
    // of the same category finds no recipe line of its own.
    let mut swapped: Vec<String> = Vec::new();

    let mut options = Vec::with_capacity(picks.len());
    for pick in &picks {
        let o = view
            .option(&pick.id)
            .ok_or_else(|| PriceError::UnknownOption {
                id: pick.id.clone(),
            })?;
        let lines = option_lines(o, size);
        let mut priced = PricedOption {
            id: o.id.clone(),
            quantity: pick.quantity.max(1),
            unit_price: o.price,
            is_swap: false,
            is_base: false,
            has_ingredients: false,
            target: None,
            base_ingredient: None,
            replacement: None,
            over: None,
        };
        let Some(target) = target_of(o) else {
            priced.has_ingredients = !lines.is_empty();
            options.push(priced);
            continue;
        };
        let slug = target.slug.clone();
        let base_ingredient = if swapped.contains(&slug) {
            None
        } else {
            base_lines
                .iter()
                .find(|r| r.category.as_deref() == Some(slug.as_str()))
                .and_then(|r| r.ingredient_id.clone())
        };
        // The replacement: an explicit swap group names it on the option;
        // otherwise the option's first ingredient line.
        let replacement = match (&target.category_id, &o.replaces) {
            (Some(_), Some(ing)) => Some(
                lines
                    .iter()
                    .find(|l| l.id.as_deref() == Some(ing.id.as_str()))
                    .map(|l| Replacement {
                        id: l.id.clone(),
                        name: l.name.clone(),
                        unit: l.unit.clone(),
                    })
                    .unwrap_or_else(|| Replacement {
                        id: Some(ing.id.clone()),
                        name: ing.name.clone(),
                        unit: ing.unit.clone(),
                    }),
            ),
            _ => lines.first().map(|l| Replacement {
                id: l.id.clone(),
                name: l.name.clone(),
                unit: l.unit.clone(),
            }),
        };
        let repl_id = replacement.as_ref().and_then(|r| r.id.clone());
        let is_base = base_ingredient.is_some() && repl_id.is_some() && base_ingredient == repl_id;
        if is_base {
            priced.unit_price = 0;
            priced.is_swap = true;
            priced.is_base = true;
        } else if replacement.is_some() {
            // Charged over the DEFAULT option: the one carrying the recipe's
            // ingredient in the chosen option's family, its own group first.
            let over = base_ingredient.as_deref().and_then(|base| {
                let cands = &view
                    .item
                    .bases
                    .iter()
                    .find(|b| b.ingredient_id == base)?
                    .candidates;
                let fits = |c: &&crate::view::BaseCandidate| match &target.category_id {
                    None => c.kind == o.kind,
                    Some(cid) => c.swap_category_id.as_deref() == Some(cid.as_str()),
                };
                let own_group = |c: &&crate::view::BaseCandidate| {
                    c.group_id.is_some() && c.group_id == o.group_id
                };
                cands
                    .iter()
                    .filter(fits)
                    .find(own_group)
                    .or_else(|| cands.iter().find(fits))
                    .map(|c| Over {
                        id: c.option_id.clone(),
                        name: c.name.clone(),
                        price: c.price,
                    })
            });
            priced.unit_price = (o.price - over.as_ref().map_or(0, |b| b.price)).max(0);
            priced.is_swap = true;
            priced.over = over;
            if base_lines
                .iter()
                .any(|r| r.category.as_deref() == Some(slug.as_str()))
                && !swapped.contains(&slug)
            {
                swapped.push(slug.clone());
            }
        }
        priced.base_ingredient = base_ingredient;
        priced.replacement = replacement;
        priced.target = Some(target);
        options.push(priced);
    }

    let mut optionals = Vec::new();
    for id in &selection.optionals {
        let Some(f) = view.item.optionals.iter().find(|f| &f.id == id) else {
            notes.push(Note::OptionalNotFound { id: id.clone() });
            continue;
        };
        if let Some(fs) = &f.size_label {
            if size != Some(fs.as_str()) {
                notes.push(Note::OptionalSizeMismatch {
                    id: id.clone(),
                    size_label: fs.clone(),
                });
                continue;
            }
        }
        optionals.push(PricedOptional {
            id: f.id.clone(),
            price: f.price,
        });
    }

    Ok(PricedOptions {
        option_total: options.iter().map(|o| o.unit_price * o.quantity).sum(),
        optional_total: optionals.iter().map(|o| o.price).sum(),
        options,
        optionals,
        notes,
    })
}

/// What `option_id` alone is charged on a line of `size_label` — the price a
/// sheet shows beside it. `None` for an option not in the view.
pub fn option_charge(view: &CatalogView, size_label: Option<&str>, option_id: &str) -> Option<i64> {
    price_options(
        view,
        &Selection {
            size_label: size_label.map(str::to_string),
            options: vec![Pick {
                id: option_id.to_string(),
                quantity: 1,
            }],
            optionals: Vec::new(),
        },
    )
    .ok()
    .and_then(|p| p.options.first().map(|o| o.unit_price))
}

/// Whether `option_id` is the recipe's own choice on a line of `size_label`
/// (a swap that costs nothing because it changes nothing) — the option a
/// sheet opens with already chosen.
pub fn is_recipe_choice(view: &CatalogView, size_label: Option<&str>, option_id: &str) -> bool {
    price_options(
        view,
        &Selection {
            size_label: size_label.map(str::to_string),
            options: vec![Pick {
                id: option_id.to_string(),
                quantity: 1,
            }],
            optionals: Vec::new(),
        },
    )
    .ok()
    .and_then(|p| p.options.first().map(|o| o.is_base))
    .unwrap_or(false)
}

#[cfg(test)]
mod swap_family_tests {
    use super::{collapse_families, swap_target};

    // Moved from MadarRust `orders/component_resolve.rs` (`swap_family_tests`).
    fn collapse_swap_families(types: &[Option<String>]) -> Vec<bool> {
        let families: Vec<Option<String>> = types
            .iter()
            .map(|t| swap_target(t.as_deref(), None, None, None).map(|s| s.family))
            .collect();
        collapse_families(&families)
    }

    fn v(xs: &[&str]) -> Vec<Option<String>> {
        xs.iter().map(|s| Some(s.to_string())).collect()
    }

    #[test]
    fn one_of_each_family_keeps_everything() {
        assert_eq!(
            collapse_swap_families(&v(&["milk_type", "coffee_type", "extra", "extra"])),
            vec![true; 4]
        );
    }

    #[test]
    fn the_last_milk_and_the_last_coffee_win() {
        assert_eq!(
            collapse_swap_families(&v(&["milk_type", "extra", "milk_type"])),
            vec![false, true, true]
        );
        assert_eq!(
            collapse_swap_families(&v(&["coffee_type", "coffee_type"])),
            vec![false, true]
        );
    }

    #[test]
    fn an_explicit_milk_group_is_the_milk_family() {
        let t = swap_target(Some("extra"), Some("swaps"), Some("c1"), Some("milk")).unwrap();
        assert_eq!((t.slug.as_str(), t.family.as_str()), ("milk", "milk_type"));
        let t = swap_target(Some("extra"), Some("swaps"), Some("c2"), Some("tea")).unwrap();
        assert_eq!(t.family, "category:c2");
        // An explicit group without a category falls back to the type.
        assert!(swap_target(Some("extra"), Some("swaps"), None, Some("tea")).is_none());
    }
}
