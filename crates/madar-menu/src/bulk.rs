//! A whole menu as one value, and the checks a server runs before writing it.
//!
//! Categories and modifier groups carry a `key`, a string the sender makes up;
//! items name their category and their groups by it, and the server swaps the
//! keys for ids. Money is whole piastres. Names are compared trimmed, with
//! runs of spaces folded and case ignored ([`name_key`]), so "Latte" and
//! " latte" are one name.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// The [`BulkMenu::version`] this crate reads. A sender on another version is
/// refused, not guessed at.
pub const VERSION: u32 = 1;

/// The label of the hidden size the server gives an item with no sizes.
const HIDDEN_SIZE: &str = "one_size";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BulkMenu {
    pub version: u32,
    #[serde(default)]
    pub categories: Vec<BulkCategory>,
    #[serde(default)]
    pub groups: Vec<BulkGroup>,
    #[serde(default)]
    pub items: Vec<BulkItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BulkCategory {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub name_ar: Option<String>,
}

/// Which kind of choice a group is, the `legacy_addon_type` the POS reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum BulkFamily {
    MilkType,
    CoffeeType,
    Extra,
}

impl BulkFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MilkType => "milk_type",
            Self::CoffeeType => "coffee_type",
            Self::Extra => "extra",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BulkGroup {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub name_ar: Option<String>,
    /// The group's own limits; an item's link may differ ([`BulkLink`]).
    pub min: u32,
    /// `None`: as many as there are options.
    #[serde(default)]
    pub max: Option<u32>,
    pub family: BulkFamily,
    pub options: Vec<BulkOption>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BulkOption {
    pub name: String,
    #[serde(default)]
    pub name_ar: Option<String>,
    pub price: i64,
    pub is_active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BulkItem {
    pub name: String,
    #[serde(default)]
    pub name_ar: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub description_ar: Option<String>,
    /// A [`BulkCategory::key`].
    pub category: String,
    /// The item's price; with no `sizes` it is the one price it sells at.
    pub price: i64,
    pub is_active: bool,
    /// Each size's full price (not a difference over `price`).
    #[serde(default)]
    pub sizes: Vec<BulkSize>,
    /// The item's groups, in the order the till shows them.
    #[serde(default)]
    pub groups: Vec<BulkLink>,
    /// Free-form extras the item alone offers.
    #[serde(default)]
    pub optionals: Vec<BulkOptional>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BulkSize {
    pub label: String,
    pub price: i64,
    pub is_active: bool,
}

/// An item's use of a group, with the limits that apply on this item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BulkLink {
    /// A [`BulkGroup::key`].
    pub group: String,
    pub min: u32,
    #[serde(default)]
    pub max: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BulkOptional {
    pub name: String,
    #[serde(default)]
    pub name_ar: Option<String>,
    pub price: i64,
}

/// The most a bulk menu may hold. Generous for a café or a restaurant; they
/// stop a free account from filling the database.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Limits {
    pub categories: usize,
    pub items: usize,
    pub groups: usize,
    pub options_per_group: usize,
    pub sizes_per_item: usize,
    pub groups_per_item: usize,
    pub optionals_per_item: usize,
    pub name_chars: usize,
    pub description_chars: usize,
    /// Piastres (100,000 EGP).
    pub max_price: i64,
}

pub const LIMITS: Limits = Limits {
    categories: 200,
    items: 2000,
    groups: 300,
    options_per_group: 50,
    sizes_per_item: 20,
    groups_per_item: 30,
    optionals_per_item: 50,
    name_chars: 120,
    description_chars: 1000,
    max_price: 10_000_000,
};

/// What a refusal is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum BulkThing {
    Category,
    Item,
    Group,
    Option,
    Size,
    Link,
    Optional,
}

/// Why a bulk menu cannot be written. The first problem found; `within` names
/// the item or group the thing belongs to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(missing_as_null))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum BulkRefusal {
    UnsupportedVersion {
        got: u32,
        supported: u32,
    },
    Empty,
    TooMany {
        what: BulkThing,
        max: usize,
        got: usize,
        within: Option<String>,
    },
    /// A name, label or key that is empty once trimmed. `index` is 0-based.
    Blank {
        what: BulkThing,
        index: usize,
        within: Option<String>,
    },
    TooLong {
        what: BulkThing,
        name: String,
        max: usize,
    },
    /// A control character (a NUL, a bell, …) in a name or description.
    BadText {
        what: BulkThing,
        name: String,
    },
    Duplicate {
        what: BulkThing,
        name: String,
        within: Option<String>,
    },
    UnknownCategory {
        item: String,
        category: String,
    },
    UnknownGroup {
        item: String,
        group: String,
    },
    /// Limits that no pick can meet: more required than there are options,
    /// or a maximum under the minimum.
    BadLimits {
        group: String,
        item: Option<String>,
        min: u32,
        max: Option<u32>,
        options: usize,
    },
    BadPrice {
        what: BulkThing,
        name: String,
        price: i64,
    },
    /// The label the server keeps for an item with no sizes.
    ReservedLabel {
        item: String,
        label: String,
    },
}

impl BulkRefusal {
    /// The API's error code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedVersion { .. } => "BULK_MENU_UNSUPPORTED_VERSION",
            _ => "BULK_MENU_INVALID",
        }
    }
}

impl std::fmt::Display for BulkRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedVersion { got, supported } => {
                write!(
                    f,
                    "bulk menu version {got} is not supported (this server reads {supported})"
                )
            }
            Self::Empty => write!(f, "the menu has no items"),
            Self::TooMany { what, max, got, .. } => {
                write!(f, "too many {what:?}: {got} (at most {max})")
            }
            Self::Blank { what, index, .. } => write!(f, "{what:?} #{} has no name", index + 1),
            Self::TooLong { what, name, max } => {
                write!(f, "{what:?} \"{name}\" is longer than {max} characters")
            }
            Self::BadText { what, name } => {
                write!(f, "{what:?} \"{name}\" holds a control character")
            }
            Self::Duplicate { what, name, .. } => write!(f, "{what:?} \"{name}\" appears twice"),
            Self::UnknownCategory { item, category } => {
                write!(
                    f,
                    "item \"{item}\" names category \"{category}\", which is not in the menu"
                )
            }
            Self::UnknownGroup { item, group } => {
                write!(
                    f,
                    "item \"{item}\" names group \"{group}\", which is not in the menu"
                )
            }
            Self::BadLimits {
                group,
                min,
                max,
                options,
                ..
            } => {
                write!(f, "group \"{group}\": min {min}, max {max:?} cannot be met with {options} options")
            }
            Self::BadPrice { what, name, price } => {
                write!(f, "{what:?} \"{name}\" has price {price}")
            }
            Self::ReservedLabel { item, label } => write!(
                f,
                "item \"{item}\" uses the reserved size label \"{label}\""
            ),
        }
    }
}

impl std::error::Error for BulkRefusal {}

/// What a valid menu holds, for the preview and the server's answer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BulkSummary {
    pub categories: usize,
    pub items: usize,
    pub sizes: usize,
    pub groups: usize,
    pub options: usize,
    pub links: usize,
    pub optionals: usize,
}

/// How two names are compared: trimmed, runs of whitespace folded, case
/// ignored.
pub fn name_key(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The checks a server runs before writing `menu`; the first problem refuses
/// it.
pub fn validate(menu: &BulkMenu) -> Result<BulkSummary, BulkRefusal> {
    if menu.version != VERSION {
        return Err(BulkRefusal::UnsupportedVersion {
            got: menu.version,
            supported: VERSION,
        });
    }
    if menu.items.is_empty() {
        return Err(BulkRefusal::Empty);
    }
    count(
        BulkThing::Category,
        menu.categories.len(),
        LIMITS.categories,
        None,
    )?;
    count(BulkThing::Item, menu.items.len(), LIMITS.items, None)?;
    count(BulkThing::Group, menu.groups.len(), LIMITS.groups, None)?;

    let mut s = BulkSummary {
        categories: menu.categories.len(),
        items: menu.items.len(),
        ..BulkSummary::default()
    };

    let mut category_keys = HashSet::new();
    let mut names = Names::default();
    for (i, c) in menu.categories.iter().enumerate() {
        name(BulkThing::Category, &c.key, i, None)?;
        name(BulkThing::Category, &c.name, i, None)?;
        optional_name(BulkThing::Category, c.name_ar.as_deref())?;
        names.add(BulkThing::Category, &c.name, None)?;
        if !category_keys.insert(c.key.trim()) {
            return Err(dup(BulkThing::Category, &c.key, None));
        }
    }

    let mut groups = std::collections::HashMap::new();
    let mut group_names = Names::default();
    for (i, g) in menu.groups.iter().enumerate() {
        name(BulkThing::Group, &g.key, i, None)?;
        name(BulkThing::Group, &g.name, i, None)?;
        optional_name(BulkThing::Group, g.name_ar.as_deref())?;
        group_names.add(BulkThing::Group, &g.name, None)?;
        count(
            BulkThing::Option,
            g.options.len(),
            LIMITS.options_per_group,
            Some(&g.name),
        )?;
        let mut options = Names::default();
        for (j, o) in g.options.iter().enumerate() {
            name(BulkThing::Option, &o.name, j, Some(&g.name))?;
            optional_name(BulkThing::Option, o.name_ar.as_deref())?;
            options.add(BulkThing::Option, &o.name, Some(&g.name))?;
            price(BulkThing::Option, &o.name, o.price)?;
        }
        let active = g.options.iter().filter(|o| o.is_active).count();
        limits(&g.name, None, g.min, g.max, active)?;
        if groups.insert(g.key.trim(), (g, active)).is_some() {
            return Err(dup(BulkThing::Group, &g.key, None));
        }
        s.options += g.options.len();
    }
    s.groups = groups.len();

    let mut items = Names::default();
    for (i, it) in menu.items.iter().enumerate() {
        name(BulkThing::Item, &it.name, i, None)?;
        optional_name(BulkThing::Item, it.name_ar.as_deref())?;
        description(it.description.as_deref())?;
        description(it.description_ar.as_deref())?;
        items.add(BulkThing::Item, &it.name, None)?;
        price(BulkThing::Item, &it.name, it.price)?;
        if !category_keys.contains(it.category.trim()) {
            return Err(BulkRefusal::UnknownCategory {
                item: it.name.clone(),
                category: it.category.clone(),
            });
        }

        count(
            BulkThing::Size,
            it.sizes.len(),
            LIMITS.sizes_per_item,
            Some(&it.name),
        )?;
        let mut labels = Names::default();
        for (j, z) in it.sizes.iter().enumerate() {
            name(BulkThing::Size, &z.label, j, Some(&it.name))?;
            if name_key(&z.label) == HIDDEN_SIZE {
                return Err(BulkRefusal::ReservedLabel {
                    item: it.name.clone(),
                    label: z.label.clone(),
                });
            }
            labels.add(BulkThing::Size, &z.label, Some(&it.name))?;
            price(BulkThing::Size, &z.label, z.price)?;
        }

        count(
            BulkThing::Link,
            it.groups.len(),
            LIMITS.groups_per_item,
            Some(&it.name),
        )?;
        let mut linked = HashSet::new();
        for l in &it.groups {
            let Some((g, active)) = groups.get(l.group.trim()) else {
                return Err(BulkRefusal::UnknownGroup {
                    item: it.name.clone(),
                    group: l.group.clone(),
                });
            };
            if !linked.insert(l.group.trim()) {
                return Err(dup(BulkThing::Link, &g.name, Some(&it.name)));
            }
            limits(&g.name, Some(&it.name), l.min, l.max, *active)?;
        }

        count(
            BulkThing::Optional,
            it.optionals.len(),
            LIMITS.optionals_per_item,
            Some(&it.name),
        )?;
        let mut optionals = Names::default();
        for (j, o) in it.optionals.iter().enumerate() {
            name(BulkThing::Optional, &o.name, j, Some(&it.name))?;
            optional_name(BulkThing::Optional, o.name_ar.as_deref())?;
            optionals.add(BulkThing::Optional, &o.name, Some(&it.name))?;
            price(BulkThing::Optional, &o.name, o.price)?;
        }

        s.sizes += it.sizes.len();
        s.links += it.groups.len();
        s.optionals += it.optionals.len();
    }
    Ok(s)
}

#[derive(Default)]
struct Names(HashSet<String>);

impl Names {
    fn add(&mut self, what: BulkThing, n: &str, within: Option<&str>) -> Result<(), BulkRefusal> {
        if self.0.insert(name_key(n)) {
            Ok(())
        } else {
            Err(dup(what, n, within))
        }
    }
}

fn dup(what: BulkThing, n: &str, within: Option<&str>) -> BulkRefusal {
    BulkRefusal::Duplicate {
        what,
        name: n.trim().to_string(),
        within: within.map(str::to_string),
    }
}

fn count(what: BulkThing, got: usize, max: usize, within: Option<&str>) -> Result<(), BulkRefusal> {
    if got > max {
        return Err(BulkRefusal::TooMany {
            what,
            max,
            got,
            within: within.map(str::to_string),
        });
    }
    Ok(())
}

fn text(what: BulkThing, s: &str, max: usize, allow_breaks: bool) -> Result<(), BulkRefusal> {
    let short: String = s.chars().take(40).collect();
    if s.chars().count() > max {
        return Err(BulkRefusal::TooLong {
            what,
            name: short,
            max,
        });
    }
    if s.chars()
        .any(|c| c.is_control() && !(allow_breaks && matches!(c, '\n' | '\r' | '\t')))
    {
        return Err(BulkRefusal::BadText { what, name: short });
    }
    Ok(())
}

fn name(what: BulkThing, s: &str, index: usize, within: Option<&str>) -> Result<(), BulkRefusal> {
    if s.trim().is_empty() {
        return Err(BulkRefusal::Blank {
            what,
            index,
            within: within.map(str::to_string),
        });
    }
    text(what, s, LIMITS.name_chars, false)
}

fn optional_name(what: BulkThing, s: Option<&str>) -> Result<(), BulkRefusal> {
    s.map_or(Ok(()), |s| text(what, s, LIMITS.name_chars, false))
}

fn description(s: Option<&str>) -> Result<(), BulkRefusal> {
    s.map_or(Ok(()), |s| {
        text(BulkThing::Item, s, LIMITS.description_chars, true)
    })
}

fn price(what: BulkThing, n: &str, p: i64) -> Result<(), BulkRefusal> {
    if !(0..=LIMITS.max_price).contains(&p) {
        return Err(BulkRefusal::BadPrice {
            what,
            name: n.trim().to_string(),
            price: p,
        });
    }
    Ok(())
}

fn limits(
    group: &str,
    item: Option<&str>,
    min: u32,
    max: Option<u32>,
    options: usize,
) -> Result<(), BulkRefusal> {
    let unmeetable = usize::try_from(min).map_or(true, |m| m > options);
    if unmeetable || max.is_some_and(|m| m == 0 || m < min) {
        return Err(BulkRefusal::BadLimits {
            group: group.to_string(),
            item: item.map(str::to_string),
            min,
            max,
            options,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn option(name: &str, price: i64) -> BulkOption {
        BulkOption {
            name: name.into(),
            name_ar: None,
            price,
            is_active: true,
        }
    }

    fn menu() -> BulkMenu {
        BulkMenu {
            version: VERSION,
            categories: vec![BulkCategory {
                key: "c1".into(),
                name: "Coffee".into(),
                name_ar: Some("قهوة".into()),
            }],
            groups: vec![BulkGroup {
                key: "milk".into(),
                name: "Milk".into(),
                name_ar: None,
                min: 1,
                max: Some(1),
                family: BulkFamily::MilkType,
                options: vec![option("Full Cream", 0), option("Oat", 4500)],
            }],
            items: vec![BulkItem {
                name: "Latte".into(),
                name_ar: Some("لاتيه".into()),
                description: None,
                description_ar: None,
                category: "c1".into(),
                price: 9000,
                is_active: true,
                sizes: vec![
                    BulkSize {
                        label: "Small".into(),
                        price: 9000,
                        is_active: true,
                    },
                    BulkSize {
                        label: "Large".into(),
                        price: 11000,
                        is_active: true,
                    },
                ],
                groups: vec![BulkLink {
                    group: "milk".into(),
                    min: 1,
                    max: Some(1),
                }],
                optionals: vec![],
            }],
        }
    }

    #[test]
    fn a_whole_menu_passes_and_is_counted() {
        let s = validate(&menu()).unwrap();
        assert_eq!(
            s,
            BulkSummary {
                categories: 1,
                items: 1,
                sizes: 2,
                groups: 1,
                options: 2,
                links: 1,
                optionals: 0
            }
        );
    }

    #[test]
    fn another_version_is_refused() {
        let mut m = menu();
        m.version = 2;
        assert_eq!(
            validate(&m),
            Err(BulkRefusal::UnsupportedVersion {
                got: 2,
                supported: 1
            })
        );
    }

    #[test]
    fn names_differing_only_in_case_and_spaces_are_one_name() {
        let mut m = menu();
        let mut twin = m.items[0].clone();
        twin.name = "  LATTE ".into();
        m.items.push(twin);
        assert!(matches!(
            validate(&m),
            Err(BulkRefusal::Duplicate {
                what: BulkThing::Item,
                ..
            })
        ));
    }

    #[test]
    fn unknown_refs_are_refused() {
        let mut m = menu();
        m.items[0].category = "nope".into();
        assert!(matches!(
            validate(&m),
            Err(BulkRefusal::UnknownCategory { .. })
        ));
        let mut m = menu();
        m.items[0].groups[0].group = "nope".into();
        assert!(matches!(
            validate(&m),
            Err(BulkRefusal::UnknownGroup { .. })
        ));
    }

    #[test]
    fn limits_that_cannot_be_met_are_refused() {
        let mut m = menu();
        m.items[0].groups[0].min = 3;
        m.items[0].groups[0].max = None;
        assert!(matches!(
            validate(&m),
            Err(BulkRefusal::BadLimits {
                item: Some(_),
                options: 2,
                ..
            })
        ));
        let mut m = menu();
        m.groups[0].max = Some(0);
        m.groups[0].min = 0;
        assert!(matches!(
            validate(&m),
            Err(BulkRefusal::BadLimits { item: None, .. })
        ));
        // An inactive option cannot be picked, so it does not count.
        let mut m = menu();
        m.groups[0].options[1].is_active = false;
        m.groups[0].min = 2;
        m.groups[0].max = None;
        assert!(matches!(
            validate(&m),
            Err(BulkRefusal::BadLimits { options: 1, .. })
        ));
    }

    #[test]
    fn hostile_text_and_prices_are_refused() {
        let mut m = menu();
        m.items[0].name = "Lat\u{0}te".into();
        assert!(matches!(validate(&m), Err(BulkRefusal::BadText { .. })));
        let mut m = menu();
        m.items[0].name = "x".repeat(121);
        assert!(matches!(validate(&m), Err(BulkRefusal::TooLong { .. })));
        let mut m = menu();
        m.items[0].description = Some("two\nlines".into());
        assert!(validate(&m).is_ok());
        let mut m = menu();
        m.groups[0].options[0].price = -1;
        assert!(matches!(
            validate(&m),
            Err(BulkRefusal::BadPrice {
                what: BulkThing::Option,
                ..
            })
        ));
        let mut m = menu();
        m.items[0].sizes[0].price = LIMITS.max_price + 1;
        assert!(matches!(
            validate(&m),
            Err(BulkRefusal::BadPrice {
                what: BulkThing::Size,
                ..
            })
        ));
    }

    #[test]
    fn caps_hold() {
        let mut m = menu();
        let item = m.items[0].clone();
        m.items = (0..=LIMITS.items)
            .map(|i| BulkItem {
                name: format!("Item {i}"),
                ..item.clone()
            })
            .collect();
        assert!(matches!(
            validate(&m),
            Err(BulkRefusal::TooMany {
                what: BulkThing::Item,
                got: 2001,
                ..
            })
        ));
        let mut m = menu();
        m.items[0].sizes = (0..=LIMITS.sizes_per_item)
            .map(|i| BulkSize {
                label: format!("S{i}"),
                price: 0,
                is_active: true,
            })
            .collect();
        assert!(matches!(
            validate(&m),
            Err(BulkRefusal::TooMany {
                what: BulkThing::Size,
                ..
            })
        ));
    }

    #[test]
    fn the_hidden_size_label_and_empty_menus_are_refused() {
        let mut m = menu();
        m.items[0].sizes[0].label = "One_Size".into();
        assert!(matches!(
            validate(&m),
            Err(BulkRefusal::ReservedLabel { .. })
        ));
        let mut m = menu();
        m.items.clear();
        assert_eq!(validate(&m), Err(BulkRefusal::Empty));
    }

    #[test]
    fn the_wire_shape_round_trips() {
        let m = menu();
        let json = serde_json::to_value(&m).unwrap();
        assert_eq!(json["groups"][0]["family"], "milk_type");
        let back: BulkMenu = serde_json::from_value(json).unwrap();
        assert_eq!(back, m);
        let r = serde_json::to_value(BulkRefusal::Empty).unwrap();
        assert_eq!(r, serde_json::json!({ "refusal": "empty" }));
    }
}
