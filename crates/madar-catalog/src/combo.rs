//! Combos (C1–C15): a menu item of `kind = 'combo'` whose price P covers one
//! pick per slot unit; a sold combo is a header line (no money) plus one part
//! line per pick.
//!
//! - [`quote`]: the parts and what they come to. P is split over the parts
//!   by [`madar_money::alloc::split`], weighted by each pick's price at the
//!   size the combo includes; a part's line total is its share plus its
//!   surcharge (the choice's, plus a bigger size's); its add-ons are extra at
//!   their normal prices (C10).
//! - [`validate`]: the slot counts and the choices, before any price.
//! - [`available`]: whether the combo is on sale on a channel at a branch now.
//! - [`resolve_sell`]: the org-wide channel switches with a branch's
//!   overrides (owner, §11: no per-combo channel toggles; the same switches
//!   gate the deals).
//!
//! The server (orders, tickets, the public intakes, the economics panel) and
//! the POS core run this one copy. Pinned by `vectors/combo_vectors.json`
//! (hand-computed).

use serde::{Deserialize, Serialize};

use madar_money::alloc;

use crate::price::{price_options, unit_price, PriceError, PricedOptions, Selection};
use crate::sale_window::{self, LocalNow, Window};
use crate::view::CatalogView;

/// A sale channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Pos,
    Qr,
    Online,
    Delivery,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pos => "pos",
            Self::Qr => "qr",
            Self::Online => "online",
            Self::Delivery => "delivery",
        }
    }
}

/// Which channels sell combos and deals: the org's switches, or the result
/// of [`resolve_sell`] for a branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sell {
    #[serde(default = "yes")]
    pub pos: bool,
    #[serde(default = "yes")]
    pub qr: bool,
    #[serde(default = "yes")]
    pub online: bool,
    #[serde(default = "yes")]
    pub delivery: bool,
}

impl Default for Sell {
    /// Every channel on (C3).
    fn default() -> Self {
        Self {
            pos: true,
            qr: true,
            online: true,
            delivery: true,
        }
    }
}

impl Sell {
    pub fn get(&self, channel: Channel) -> bool {
        match channel {
            Channel::Pos => self.pos,
            Channel::Qr => self.qr,
            Channel::Online => self.online,
            Channel::Delivery => self.delivery,
        }
    }
}

/// A branch's overrides of the org's switches; `None` inherits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SellOverride {
    #[serde(default)]
    pub pos: Option<bool>,
    #[serde(default)]
    pub qr: Option<bool>,
    #[serde(default)]
    pub online: Option<bool>,
    #[serde(default)]
    pub delivery: Option<bool>,
}

/// The switches at a branch: its override where it sets one, else the org's.
pub fn resolve_sell(org: &Sell, branch: Option<&SellOverride>) -> Sell {
    let Some(b) = branch else {
        return *org;
    };
    Sell {
        pos: b.pos.unwrap_or(org.pos),
        qr: b.qr.unwrap_or(org.qr),
        online: b.online.unwrap_or(org.online),
        delivery: b.delivery.unwrap_or(org.delivery),
    }
}

fn yes() -> bool {
    true
}

fn one() -> i64 {
    1
}

/// A combo as a branch sells it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComboView {
    pub id: String,
    /// P: the combo's `one_size` price, branch-effective.
    pub price: i64,
    #[serde(default = "yes")]
    pub is_active: bool,
    pub slots: Vec<SlotView>,
    /// No window = always on sale.
    #[serde(default)]
    pub windows: Vec<Window>,
}

/// One slot: what a customer picks `min..=max` of.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotView {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sort: i64,
    pub min: i64,
    pub max: i64,
    #[serde(default)]
    pub default_item_id: Option<String>,
    #[serde(default)]
    pub default_size_label: Option<String>,
    pub choices: Vec<ChoiceView>,
}

/// What a slot admits: one item, or every `kind = 'item'` item of a category.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChoiceView {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub menu_item_id: Option<String>,
    #[serde(default)]
    pub category_id: Option<String>,
    /// Per pick unit (C9's per-choice surcharge).
    #[serde(default)]
    pub surcharge: i64,
    /// The size P covers; `None` = the item's cheapest active size.
    #[serde(default)]
    pub included_size_label: Option<String>,
    /// The owner's price for a bigger size; a size with no row costs its
    /// usual difference over the included size.
    #[serde(default)]
    pub size_surcharges: Vec<SizeSurcharge>,
    #[serde(default)]
    pub sort: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SizeSurcharge {
    pub size_label: String,
    pub surcharge: i64,
}

/// One pick, with its item's view (branch-priced) and its category.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PickIn {
    pub slot_id: String,
    pub view: CatalogView,
    /// The picked item's category (a category choice admits it by this).
    #[serde(default)]
    pub category_id: Option<String>,
    /// Its size, add-ons and optional fields.
    #[serde(default)]
    pub selection: Selection,
    /// Units per combo unit.
    #[serde(default = "one")]
    pub quantity: i64,
}

/// Why a combo line cannot be priced. [`ComboRefusal::code`] is the API's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum ComboRefusal {
    /// A pick names a slot the combo does not have.
    UnknownSlot {
        slot_id: String,
    },
    TooFew {
        slot_id: String,
        min: i64,
        got: i64,
    },
    TooMany {
        slot_id: String,
        max: i64,
        got: i64,
    },
    /// No choice of the slot admits the pick.
    NotAllowed {
        slot_id: String,
        menu_item_id: String,
    },
    /// A pick of fewer than one unit.
    BadQuantity {
        slot_id: String,
    },
    /// The pick's own line cannot be priced.
    Price {
        menu_item_id: String,
        error: PriceError,
    },
}

impl ComboRefusal {
    /// The coded refusal (contract §2.7).
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownSlot { .. } | Self::NotAllowed { .. } => "COMBO_CHOICE_NOT_ALLOWED",
            Self::TooFew { .. } | Self::BadQuantity { .. } => "COMBO_SLOT_TOO_FEW",
            Self::TooMany { .. } => "COMBO_SLOT_TOO_MANY",
            Self::Price { .. } => "COMBO_ITEM_UNAVAILABLE",
        }
    }
}

impl core::fmt::Display for ComboRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownSlot { slot_id } => write!(f, "no slot {slot_id} in this combo"),
            Self::TooFew { slot_id, min, got } => {
                write!(f, "slot {slot_id} needs at least {min}, got {got}")
            }
            Self::TooMany { slot_id, max, got } => {
                write!(f, "slot {slot_id} takes at most {max}, got {got}")
            }
            Self::NotAllowed {
                slot_id,
                menu_item_id,
            } => write!(f, "item {menu_item_id} is not a choice of slot {slot_id}"),
            Self::BadQuantity { slot_id } => write!(f, "a pick in slot {slot_id} is below 1"),
            Self::Price {
                menu_item_id,
                error,
            } => write!(f, "item {menu_item_id}: {error}"),
        }
    }
}

/// The choice of `slot` that admits the item: its own item choice first,
/// else the category choice for `category_id`.
pub fn choice_for<'a>(
    slot: &'a SlotView,
    menu_item_id: &str,
    category_id: Option<&str>,
) -> Option<&'a ChoiceView> {
    slot.choices
        .iter()
        .find(|c| c.menu_item_id.as_deref() == Some(menu_item_id))
        .or_else(|| {
            let cat = category_id?;
            slot.choices
                .iter()
                .find(|c| c.menu_item_id.is_none() && c.category_id.as_deref() == Some(cat))
        })
}

/// C1's fixed bundle: every slot has exactly one choice, an item choice, and
/// `min == max`.
pub fn is_fixed(combo: &ComboView) -> bool {
    !combo.slots.is_empty()
        && combo
            .slots
            .iter()
            .all(|s| s.min == s.max && s.choices.len() == 1 && s.choices[0].menu_item_id.is_some())
}

/// The size P covers for a pick of `view` through `choice`: the choice's
/// included size, else the active priced size with the lowest effective price
/// (its branch price, else its price), the first in view order on a tie.
pub fn included_size(view: &CatalogView, choice: &ChoiceView) -> Result<String, PriceError> {
    if let Some(label) = &choice.included_size_label {
        return Ok(label.clone());
    }
    let mut best: Option<(i64, &str)> = None;
    for s in view.item.sizes.iter().filter(|s| s.is_active) {
        let Some(p) = s.branch_price.or(s.price) else {
            continue;
        };
        if best.is_none_or(|(b, _)| p < b) {
            best = Some((p, s.label.as_str()));
        }
    }
    best.map(|(_, l)| l.to_string())
        .ok_or(PriceError::NoPricedSize)
}

fn slot_of<'a>(combo: &'a ComboView, slot_id: &str) -> Option<(usize, &'a SlotView)> {
    combo
        .slots
        .iter()
        .enumerate()
        .find(|(_, s)| s.id == slot_id)
}

/// The picks against the slots: each pick names a slot of the combo, at
/// least one unit, and an item a choice of that slot admits (checked in input
/// order); then each slot's count (Σ pick quantity) is within `min..=max`
/// (checked in the combo's slot order).
pub fn validate(combo: &ComboView, picks: &[PickIn]) -> Result<(), ComboRefusal> {
    for p in picks {
        let Some((_, slot)) = slot_of(combo, &p.slot_id) else {
            return Err(ComboRefusal::UnknownSlot {
                slot_id: p.slot_id.clone(),
            });
        };
        if p.quantity < 1 {
            return Err(ComboRefusal::BadQuantity {
                slot_id: slot.id.clone(),
            });
        }
        if choice_for(slot, &p.view.item.id, p.category_id.as_deref()).is_none() {
            return Err(ComboRefusal::NotAllowed {
                slot_id: slot.id.clone(),
                menu_item_id: p.view.item.id.clone(),
            });
        }
    }
    for slot in &combo.slots {
        let got: i64 = picks
            .iter()
            .filter(|p| p.slot_id == slot.id)
            .map(|p| p.quantity)
            .sum();
        if got < slot.min {
            return Err(ComboRefusal::TooFew {
                slot_id: slot.id.clone(),
                min: slot.min,
                got,
            });
        }
        if got > slot.max {
            return Err(ComboRefusal::TooMany {
                slot_id: slot.id.clone(),
                max: slot.max,
                got,
            });
        }
    }
    Ok(())
}

/// A priced combo line.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComboQuote {
    /// P, per combo unit.
    pub price: i64,
    /// n: combo units on the line.
    pub quantity: i64,
    /// One combo unit with its surcharges and add-ons.
    pub unit_total: i64,
    /// One combo unit's picks à la carte at their chosen sizes, with add-ons.
    pub list_unit: i64,
    /// `list_unit − unit_total` (negative when the combo costs more).
    pub saving_unit: i64,
    /// In slot order (slot sort, slot position, then input order).
    pub parts: Vec<PartQuote>,
}

impl ComboQuote {
    /// The whole line: n × unit_total (= Σ part line_total + Σ addons_total).
    pub fn line_total(&self) -> i64 {
        self.quantity * self.unit_total
    }
}

/// One part line of a combo line. The `unit`/per-pick figures are for ONE
/// combo unit; the rest are for the whole line (n combo units).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartQuote {
    /// Index into the input picks.
    pub pick_index: usize,
    pub slot_id: String,
    pub menu_item_id: String,
    /// The pick's size, else its included size.
    pub size_label: String,
    pub included_size_label: String,
    /// Units per combo unit.
    pub pick_quantity: i64,
    /// The part line's quantity: pick quantity × n.
    pub quantity: i64,
    /// The item's normal price at the chosen size (so `unit_price × quantity
    /// − line_total` is what the combo saved on this line).
    pub unit_price: i64,
    /// This part's share of ONE P.
    pub share_unit: i64,
    /// Per pick unit: the choice's surcharge + the size's extra.
    pub surcharge_unit: i64,
    /// n × share_unit.
    pub combo_share: i64,
    /// n × pick quantity × surcharge_unit.
    pub combo_surcharge: i64,
    /// combo_share + combo_surcharge.
    pub line_total: i64,
    /// The pick's add-ons and optional fields, priced per unit of the part.
    pub options: PricedOptions,
    /// option_total + optional_total, per unit of the part.
    pub extras_unit: i64,
    /// extras_unit × quantity.
    pub addons_total: i64,
}

/// A combo line of `n` units (n below 1 counts as 1): [`validate`], then
/// price every pick and split P (contract §4).
pub fn quote(combo: &ComboView, picks: &[PickIn], n: i64) -> Result<ComboQuote, ComboRefusal> {
    validate(combo, picks)?;
    let n = n.max(1);

    // Slot order: sort, then position in the combo, then input order.
    let mut order: Vec<(i64, usize, usize)> = picks
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let (pos, slot) = slot_of(combo, &p.slot_id).expect("validated");
            (slot.sort, pos, i)
        })
        .collect();
    order.sort();

    struct Priced {
        index: usize,
        slot_id: String,
        included: String,
        label: String,
        base: i64,
        chosen: i64,
        surcharge_unit: i64,
        options: PricedOptions,
        extras_unit: i64,
        quantity: i64,
    }
    let mut priced = Vec::with_capacity(order.len());
    for &(_, pos, i) in &order {
        let p = &picks[i];
        let slot = &combo.slots[pos];
        let choice =
            choice_for(slot, &p.view.item.id, p.category_id.as_deref()).expect("validated");
        let fail = |error: PriceError| ComboRefusal::Price {
            menu_item_id: p.view.item.id.clone(),
            error,
        };
        let included = included_size(&p.view, choice).map_err(fail)?;
        let base = unit_price(&p.view.item, Some(&included)).map_err(fail)?;
        let label = p
            .selection
            .size_label
            .clone()
            .unwrap_or_else(|| included.clone());
        let chosen = unit_price(&p.view.item, Some(&label)).map_err(fail)?;
        let size_extra = if label == included {
            0
        } else if let Some(s) = choice
            .size_surcharges
            .iter()
            .find(|s| s.size_label == label)
        {
            s.surcharge
        } else {
            (chosen - base).max(0)
        };
        let selection = Selection {
            size_label: Some(label.clone()),
            ..p.selection.clone()
        };
        let options = price_options(&p.view, &selection).map_err(fail)?;
        let extras_unit = options.option_total + options.optional_total;
        priced.push(Priced {
            index: i,
            slot_id: slot.id.clone(),
            included,
            label,
            base,
            chosen,
            surcharge_unit: choice.surcharge + size_extra,
            options,
            extras_unit,
            quantity: p.quantity,
        });
    }

    let weights: Vec<i64> = priced.iter().map(|p| p.base * p.quantity).collect();
    let shares = alloc::split(combo.price, &weights);

    let mut unit_total = combo.price;
    let mut list_unit = 0;
    let mut parts = Vec::with_capacity(priced.len());
    for (p, share_unit) in priced.into_iter().zip(shares) {
        unit_total += p.quantity * (p.surcharge_unit + p.extras_unit);
        list_unit += p.quantity * (p.chosen + p.extras_unit);
        let quantity = p.quantity * n;
        let combo_share = n * share_unit;
        let combo_surcharge = quantity * p.surcharge_unit;
        parts.push(PartQuote {
            pick_index: p.index,
            slot_id: p.slot_id,
            menu_item_id: picks[p.index].view.item.id.clone(),
            size_label: p.label,
            included_size_label: p.included,
            pick_quantity: p.quantity,
            quantity,
            unit_price: p.chosen,
            share_unit,
            surcharge_unit: p.surcharge_unit,
            combo_share,
            combo_surcharge,
            line_total: combo_share + combo_surcharge,
            options: p.options,
            extras_unit: p.extras_unit,
            addons_total: p.extras_unit * quantity,
        });
    }
    Ok(ComboQuote {
        price: combo.price,
        quantity: n,
        unit_total,
        list_unit,
        saving_unit: list_unit - unit_total,
        parts,
    })
}

/// Why a combo is not on sale.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum Unavailable {
    Inactive,
    /// Switched off at the branch (or the delivery sub-channel) through its
    /// `one_size` price override.
    Branch,
    /// The channel's switch is off at this branch.
    Channel,
    /// No window covers now.
    Window,
    /// A required slot has no choice left to sell.
    SlotEmpty {
        slot_id: String,
    },
}

impl Unavailable {
    /// `inactive|branch|channel|window|slot_empty` (the `reason` var of
    /// `COMBO_UNAVAILABLE`).
    pub fn token(&self) -> &'static str {
        match self {
            Self::Inactive => "inactive",
            Self::Branch => "branch",
            Self::Channel => "channel",
            Self::Window => "window",
            Self::SlotEmpty { .. } => "slot_empty",
        }
    }
}

/// Where and when a combo is asked for.
#[derive(Clone, Copy, Debug)]
pub struct Availability<'a> {
    pub channel: Channel,
    /// The channel switches, already resolved for the branch.
    pub sell: &'a Sell,
    /// The combo item is not switched off at the branch / sub-channel.
    pub branch_enabled: bool,
    pub branch_id: Option<&'a str>,
    pub now: &'a LocalNow,
}

/// Is `combo` on sale `at`? The first reason, in this order: inactive, the
/// branch, the channel, the windows, then a required slot (min ≥ 1) whose
/// every choice `choice_available` rejects. The caller expands a category
/// choice against its own menu.
pub fn available(
    combo: &ComboView,
    at: &Availability,
    choice_available: impl Fn(&ChoiceView) -> bool,
) -> Result<(), Unavailable> {
    if !combo.is_active {
        return Err(Unavailable::Inactive);
    }
    if !at.branch_enabled {
        return Err(Unavailable::Branch);
    }
    if !at.sell.get(at.channel) {
        return Err(Unavailable::Channel);
    }
    if !sale_window::open(&combo.windows, at.branch_id, at.now) {
        return Err(Unavailable::Window);
    }
    for slot in &combo.slots {
        if slot.min >= 1 && !slot.choices.iter().any(&choice_available) {
            return Err(Unavailable::SlotEmpty {
                slot_id: slot.id.clone(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Deserialize)]
    struct Vectors {
        items: BTreeMap<String, CatalogView>,
        combos: BTreeMap<String, ComboView>,
        cases: Vec<Case>,
        availability: Vec<AvailCase>,
        sell: Vec<SellCase>,
        is_fixed: Vec<FixedCase>,
    }

    #[derive(Deserialize)]
    struct Case {
        name: String,
        combo: String,
        picks: Vec<PickCase>,
        n: i64,
        expected: Expected,
        #[serde(default)]
        is_fixed: Option<bool>,
    }

    #[derive(Deserialize)]
    struct PickCase {
        slot_id: String,
        item: String,
        #[serde(default)]
        category_id: Option<String>,
        selection: Selection,
        quantity: i64,
    }

    #[derive(Debug, PartialEq, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Expected {
        Quote(ComboQuote),
        Refusal(ComboRefusal),
    }

    #[derive(Deserialize)]
    struct AvailCase {
        name: String,
        combo: String,
        channel: Channel,
        sell: Sell,
        branch_enabled: bool,
        branch_id: Option<String>,
        now: LocalNow,
        unavailable_items: Vec<String>,
        empty_categories: Vec<String>,
        expected: Option<Unavailable>,
    }

    #[derive(Deserialize)]
    struct SellCase {
        name: String,
        org: Sell,
        branch: Option<SellOverride>,
        expected: Sell,
    }

    #[derive(Deserialize)]
    struct FixedCase {
        combo: String,
        is_fixed: bool,
    }

    fn load() -> Vectors {
        serde_json::from_str(crate::vectors::COMBO).unwrap()
    }

    #[test]
    fn combo_quote_vectors() {
        let v = load();
        assert!(v.cases.len() >= 20);
        for c in &v.cases {
            let combo = &v.combos[&c.combo];
            let picks: Vec<PickIn> = c
                .picks
                .iter()
                .map(|p| PickIn {
                    slot_id: p.slot_id.clone(),
                    view: v.items[&p.item].clone(),
                    category_id: p.category_id.clone(),
                    selection: p.selection.clone(),
                    quantity: p.quantity,
                })
                .collect();
            let got = match quote(combo, &picks, c.n) {
                Ok(q) => Expected::Quote(q),
                Err(r) => Expected::Refusal(r),
            };
            assert_eq!(got, c.expected, "{}", c.name);
            if let Expected::Quote(q) = &got {
                // The money identities.
                let shares: i64 = q.parts.iter().map(|p| p.share_unit).sum();
                assert_eq!(shares, q.price, "{}: Σ shares = P", c.name);
                let whole: i64 = q.parts.iter().map(|p| p.line_total + p.addons_total).sum();
                assert_eq!(
                    whole,
                    q.line_total(),
                    "{}: Σ lines = n × unit_total",
                    c.name
                );
                for p in &q.parts {
                    assert_eq!(
                        p.line_total,
                        p.combo_share + p.combo_surcharge,
                        "{}",
                        c.name
                    );
                }
            }
            if let Some(f) = c.is_fixed {
                assert_eq!(is_fixed(combo), f, "{}: is_fixed", c.name);
            }
        }
    }

    #[test]
    fn combo_availability_vectors() {
        let v = load();
        for c in &v.availability {
            let at = Availability {
                channel: c.channel,
                sell: &c.sell,
                branch_enabled: c.branch_enabled,
                branch_id: c.branch_id.as_deref(),
                now: &c.now,
            };
            let got = available(&v.combos[&c.combo], &at, |ch| {
                ch.menu_item_id
                    .as_ref()
                    .is_none_or(|i| !c.unavailable_items.contains(i))
                    && ch
                        .category_id
                        .as_ref()
                        .is_none_or(|k| !c.empty_categories.contains(k))
            });
            assert_eq!(got.err(), c.expected, "{}", c.name);
        }
    }

    #[test]
    fn combo_sell_and_fixed_vectors() {
        let v = load();
        for c in &v.sell {
            assert_eq!(
                resolve_sell(&c.org, c.branch.as_ref()),
                c.expected,
                "{}",
                c.name
            );
        }
        for c in &v.is_fixed {
            assert_eq!(is_fixed(&v.combos[&c.combo]), c.is_fixed, "{}", c.combo);
        }
    }

    #[test]
    fn codes() {
        let r = ComboRefusal::TooMany {
            slot_id: "s".into(),
            max: 1,
            got: 2,
        };
        assert_eq!(r.code(), "COMBO_SLOT_TOO_MANY");
        assert_eq!(Unavailable::Window.token(), "window");
        assert_eq!(Channel::Qr.as_str(), "qr");
    }
}
