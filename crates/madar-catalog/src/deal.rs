//! Deal rules (C8, C17): mix & match and multi-buy over a cart.
//!
//! Two kinds cover the brief:
//! - `n_for_price`: any N units from the pool for a price X ("any 2 bites for
//!   90");
//! - `buy_get`: buy B, get G at p% off, 100 = free ("buy 2 get 1"); with a
//!   reward pool the G come from it ("buy 2 coffees get a cookie").
//!
//! A deal covers the item price at its size only: add-ons always pay. Only
//! plain lines take part (never a combo header or part, a reward or a staff
//! drink) — the caller passes those lines alone, each with the units still
//! free.
//!
//! - The till: [`suggest`] after every cart change; the teller applies one
//!   ([`price_application`] on its units); its units are consumed and
//!   `suggest` runs again. Never auto-applied on the till.
//! - QR and online checkout: [`auto_apply`] — the best suggestion, again and
//!   again, until none is left (the customer sees what was applied).
//! - The server: [`price_application`] on the units a till names (live: a
//!   refusal is `409 DEAL_NOT_ELIGIBLE`; replay: the server's verdict).
//!
//! A chunk's discount is spread over its units in proportion to their prices
//! ([`madar_money::alloc::split`]), so item reports stay comparable (§10 Q5).
//! Pinned by `vectors/deal_vectors.json` (hand-computed).

use serde::{Deserialize, Serialize};

use madar_money::alloc;

use crate::sale_window::{self, LocalNow, Window};

fn yes() -> bool {
    true
}

/// A pool entry: an item, or every item of a category; optionally one size.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolEntry {
    #[serde(default)]
    pub menu_item_id: Option<String>,
    #[serde(default)]
    pub category_id: Option<String>,
    /// `None` = any size.
    #[serde(default)]
    pub size_label: Option<String>,
}

/// A deal rule as the `deal_rule` feed row and `DealRule` carry it (extra
/// fields ignored).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DealView {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// `n_for_price` | `buy_get`.
    pub kind: String,
    /// N (`n_for_price`) or the "buy" count B.
    pub qty: i64,
    /// `n_for_price`: the price of N.
    #[serde(default)]
    pub price: Option<i64>,
    /// `buy_get`: G.
    #[serde(default)]
    pub get_qty: Option<i64>,
    /// `buy_get`: the percentage off the G (100 = free).
    #[serde(default)]
    pub get_percent: Option<i64>,
    #[serde(default)]
    pub max_per_order: Option<i64>,
    #[serde(default)]
    pub sort: i64,
    /// Branch-resolved by the caller (the feed row already is).
    #[serde(default = "yes")]
    pub is_active: bool,
    #[serde(default)]
    pub pool: Vec<PoolEntry>,
    /// `buy_get` only; empty = the G come from the pool too.
    #[serde(default)]
    pub reward_pool: Vec<PoolEntry>,
    #[serde(default)]
    pub windows: Vec<Window>,
}

/// One eligible cart line.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DealLine {
    /// The line's index in the order's `items[]` (the till: its cart line).
    pub line_index: usize,
    pub menu_item_id: String,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub size_label: Option<String>,
    /// One unit at its size, no add-ons.
    pub unit_price: i64,
    /// The units still free (not in another applied deal).
    pub quantity: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineUnits {
    pub line_index: usize,
    pub units: i64,
}

/// An application already on the order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsedDeal {
    pub deal_id: String,
    pub times: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DealContext {
    #[serde(default)]
    pub branch_id: Option<String>,
    /// The branch's local wall clock.
    pub now: LocalNow,
    /// Applications already on the order (they count toward `max_per_order`).
    #[serde(default)]
    pub used: Vec<UsedDeal>,
}

/// What the till offers: the deal, how many times, the units, the saving.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    pub deal_id: String,
    pub times: i64,
    /// By line_index.
    pub units: Vec<LineUnits>,
    /// = [`price_application`]'s discount for these units.
    pub saving: i64,
}

/// A priced application.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Application {
    pub deal_id: String,
    pub times: i64,
    pub discount: i64,
    /// By line_index.
    pub lines: Vec<AppliedLine>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedLine {
    pub line_index: usize,
    pub units: i64,
    /// This application's cut of the line (its `deal_minor`).
    pub discount: i64,
}

/// Why an application is refused (`DEAL_NOT_ELIGIBLE` with this `reason`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum NotEligible {
    Inactive,
    Window,
    /// A malformed rule.
    Invalid,
    NotInPool {
        line_index: usize,
    },
    /// The units do not make whole chunks (or a reward pool has too few).
    Count,
    MaxPerOrder,
    NoSaving,
    /// More units than the line has free (a missing line has none).
    Overlap {
        line_index: usize,
    },
}

impl NotEligible {
    pub fn token(&self) -> &'static str {
        match self {
            Self::Inactive => "inactive",
            Self::Window => "window",
            Self::Invalid => "invalid",
            Self::NotInPool { .. } => "not_in_pool",
            Self::Count => "count",
            Self::MaxPerOrder => "max_per_order",
            Self::NoSaving => "no_saving",
            Self::Overlap { .. } => "overlap",
        }
    }
}

impl core::fmt::Display for NotEligible {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.token())
    }
}

#[derive(Clone, Copy, Debug)]
enum Rule {
    NForPrice { n: i64, price: i64 },
    BuyGet { buy: i64, get: i64, pct: i64 },
}

fn rule(deal: &DealView) -> Result<Rule, NotEligible> {
    match deal.kind.as_str() {
        "n_for_price" => match deal.price {
            Some(price) if price >= 0 && deal.qty >= 2 => {
                Ok(Rule::NForPrice { n: deal.qty, price })
            }
            _ => Err(NotEligible::Invalid),
        },
        "buy_get" => match (deal.get_qty, deal.get_percent) {
            (Some(get), Some(pct)) if deal.qty >= 1 && get >= 1 && (1..=100).contains(&pct) => {
                Ok(Rule::BuyGet {
                    buy: deal.qty,
                    get,
                    pct,
                })
            }
            _ => Err(NotEligible::Invalid),
        },
        _ => Err(NotEligible::Invalid),
    }
}

fn entry_matches(e: &PoolEntry, line: &DealLine) -> bool {
    let hit = match (&e.menu_item_id, &e.category_id) {
        (Some(id), _) => *id == line.menu_item_id,
        (None, Some(cat)) => line.category_id.as_deref() == Some(cat.as_str()),
        (None, None) => false,
    };
    hit && e
        .size_label
        .as_ref()
        .is_none_or(|s| line.size_label.as_deref() == Some(s.as_str()))
}

fn in_pool(pool: &[PoolEntry], line: &DealLine) -> bool {
    pool.iter().any(|e| entry_matches(e, line))
}

fn used_times(deal: &DealView, ctx: &DealContext) -> i64 {
    ctx.used
        .iter()
        .filter(|u| u.deal_id == deal.id)
        .map(|u| u.times)
        .sum()
}

/// How many more times the deal may apply on this order.
fn cap(deal: &DealView, ctx: &DealContext) -> i64 {
    deal.max_per_order
        .map_or(i64::MAX, |m| m - used_times(deal, ctx))
}

/// One unit of a line.
#[derive(Clone, Copy, Debug)]
struct Unit {
    /// Position in the `lines` slice.
    pos: usize,
    line_index: usize,
    value: i64,
}

fn desc(units: &mut [Unit]) {
    units.sort_by(|a, b| b.value.cmp(&a.value).then(a.line_index.cmp(&b.line_index)));
}

fn expand(lines: &[DealLine], keep: impl Fn(&DealLine) -> bool) -> Vec<Unit> {
    let mut out = Vec::new();
    for (pos, l) in lines.iter().enumerate() {
        if !keep(l) {
            continue;
        }
        for _ in 0..l.quantity.max(0) {
            out.push(Unit {
                pos,
                line_index: l.line_index,
                value: l.unit_price,
            });
        }
    }
    out
}

fn pct_of(values: i64, pct: i64) -> i64 {
    alloc::round_half_away(i128::from(values.max(0)) * i128::from(pct), 100) as i64
}

/// The deal is active and a window covers now.
pub fn eligible_now(deal: &DealView, ctx: &DealContext) -> Result<(), NotEligible> {
    if !deal.is_active {
        return Err(NotEligible::Inactive);
    }
    if !sale_window::open(&deal.windows, ctx.branch_id.as_deref(), &ctx.now) {
        return Err(NotEligible::Window);
    }
    Ok(())
}

fn by_line(units: &[Unit]) -> Vec<LineUnits> {
    let mut out: Vec<LineUnits> = Vec::new();
    for u in units {
        match out.iter_mut().find(|l| l.line_index == u.line_index) {
            Some(l) => l.units += 1,
            None => out.push(LineUnits {
                line_index: u.line_index,
                units: 1,
            }),
        }
    }
    out.sort_by_key(|l| l.line_index);
    out
}

/// The best way to take `deal` over `lines` (contract §5): `None` when it
/// saves nothing, is off, or has no time left under `max_per_order`.
pub fn best(deal: &DealView, lines: &[DealLine], ctx: &DealContext) -> Option<Suggestion> {
    eligible_now(deal, ctx).ok()?;
    let r = rule(deal).ok()?;
    let cap = cap(deal, ctx);
    let mut chosen: Vec<Unit> = Vec::new();
    let mut times = 0i64;
    match r {
        Rule::NForPrice { n, price } => {
            let mut units = expand(lines, |l| in_pool(&deal.pool, l));
            desc(&mut units);
            let n = n as usize;
            let mut i = 0;
            while times < cap && i + n <= units.len() {
                let chunk = &units[i..i + n];
                if chunk.iter().map(|u| u.value).sum::<i64>() - price <= 0 {
                    break;
                }
                chosen.extend_from_slice(chunk);
                times += 1;
                i += n;
            }
        }
        Rule::BuyGet { buy, get, pct } if deal.reward_pool.is_empty() => {
            let mut units = expand(lines, |l| in_pool(&deal.pool, l));
            desc(&mut units);
            let size = (buy + get) as usize;
            let mut i = 0;
            while times < cap && i + size <= units.len() {
                let chunk = &units[i..i + size];
                let rewards: i64 = chunk[buy as usize..].iter().map(|u| u.value).sum();
                if pct_of(rewards, pct) <= 0 {
                    break;
                }
                chosen.extend_from_slice(chunk);
                times += 1;
                i += size;
            }
        }
        Rule::BuyGet { buy, get, pct } => {
            // Every unit once, with where it may go.
            let all = expand(lines, |_| true);
            let pool: Vec<bool> = all
                .iter()
                .map(|u| in_pool(&deal.pool, &lines[u.pos]))
                .collect();
            let reward: Vec<bool> = all
                .iter()
                .map(|u| in_pool(&deal.reward_pool, &lines[u.pos]))
                .collect();
            let mut order: Vec<usize> = (0..all.len()).collect();
            order.sort_by(|&a, &b| {
                all[b]
                    .value
                    .cmp(&all[a].value)
                    .then(all[a].line_index.cmp(&all[b].line_index))
            });
            let mut taken = vec![false; all.len()];
            while times < cap {
                let buys: Vec<usize> = order
                    .iter()
                    .copied()
                    .filter(|&k| pool[k] && !taken[k])
                    .take(buy as usize)
                    .collect();
                if buys.len() < buy as usize {
                    break;
                }
                let rewards: Vec<usize> = order
                    .iter()
                    .rev()
                    .copied()
                    .filter(|&k| reward[k] && !taken[k] && !buys.contains(&k))
                    .take(get as usize)
                    .collect();
                if rewards.len() < get as usize {
                    break;
                }
                if pct_of(rewards.iter().map(|&k| all[k].value).sum(), pct) <= 0 {
                    break;
                }
                for &k in buys.iter().chain(&rewards) {
                    taken[k] = true;
                    chosen.push(all[k]);
                }
                times += 1;
            }
        }
    }
    if times == 0 {
        return None;
    }
    let units = by_line(&chosen);
    let app = price_application(deal, lines, &units, ctx).ok()?;
    (app.discount > 0).then(|| Suggestion {
        deal_id: deal.id.clone(),
        times: app.times,
        units,
        saving: app.discount,
    })
}

/// Every deal that saves something now, best first: saving desc, then the
/// deal's sort, then its id. The till shows the top few; nothing is applied.
pub fn suggest(deals: &[DealView], lines: &[DealLine], ctx: &DealContext) -> Vec<Suggestion> {
    let mut out: Vec<(i64, &str, Suggestion)> = deals
        .iter()
        .filter(|d| eligible_now(d, ctx).is_ok())
        .filter_map(|d| best(d, lines, ctx).map(|s| (d.sort, d.id.as_str(), s)))
        .collect();
    out.sort_by(|a, b| {
        b.2.saving
            .cmp(&a.2.saving)
            .then(a.0.cmp(&b.0))
            .then(a.1.cmp(b.1))
    });
    out.into_iter().map(|(_, _, s)| s).collect()
}

/// Price `deal` over EXACTLY these units (no re-optimising): the till's
/// apply and the server's check. The units are re-chunked from the top;
/// each chunk's discount is split over its units by price, and summed per
/// line.
pub fn price_application(
    deal: &DealView,
    lines: &[DealLine],
    units: &[LineUnits],
    ctx: &DealContext,
) -> Result<Application, NotEligible> {
    let r = rule(deal)?;
    eligible_now(deal, ctx)?;

    // The units exist and are free.
    let mut claimed: Vec<(usize, usize, i64)> = Vec::new(); // (line_index, pos, units)
    for u in units {
        let Some(pos) = lines.iter().position(|l| l.line_index == u.line_index) else {
            return Err(NotEligible::Overlap {
                line_index: u.line_index,
            });
        };
        if u.units < 1 {
            return Err(NotEligible::Count);
        }
        let total = match claimed.iter_mut().find(|c| c.0 == u.line_index) {
            Some(c) => {
                c.2 += u.units;
                c.2
            }
            None => {
                claimed.push((u.line_index, pos, u.units));
                u.units
            }
        };
        if total > lines[pos].quantity {
            return Err(NotEligible::Overlap {
                line_index: u.line_index,
            });
        }
    }
    claimed.sort_by_key(|c| c.0);
    let mut picked: Vec<Unit> = Vec::new();
    for &(line_index, pos, n) in &claimed {
        for _ in 0..n {
            picked.push(Unit {
                pos,
                line_index,
                value: lines[pos].unit_price,
            });
        }
    }
    let total_units = picked.len() as i64;
    let cap = cap(deal, ctx);

    // Chunks: (units in split order, the chunk's discount).
    let mut chunks: Vec<(Vec<Unit>, i64)> = Vec::new();
    let in_given_order = |pred: &dyn Fn(&DealLine) -> bool| -> Result<(), NotEligible> {
        for u in units {
            let l = lines
                .iter()
                .find(|l| l.line_index == u.line_index)
                .expect("checked");
            if !pred(l) {
                return Err(NotEligible::NotInPool {
                    line_index: u.line_index,
                });
            }
        }
        Ok(())
    };
    let times = match r {
        Rule::NForPrice { n, price } => {
            in_given_order(&|l| in_pool(&deal.pool, l))?;
            if total_units == 0 || total_units % n != 0 {
                return Err(NotEligible::Count);
            }
            let times = total_units / n;
            if times > cap {
                return Err(NotEligible::MaxPerOrder);
            }
            desc(&mut picked);
            for chunk in picked.chunks(n as usize) {
                let sum: i64 = chunk.iter().map(|u| u.value).sum();
                chunks.push((chunk.to_vec(), (sum - price).max(0)));
            }
            times
        }
        Rule::BuyGet { buy, get, pct } if deal.reward_pool.is_empty() => {
            in_given_order(&|l| in_pool(&deal.pool, l))?;
            let size = buy + get;
            if total_units == 0 || total_units % size != 0 {
                return Err(NotEligible::Count);
            }
            let times = total_units / size;
            if times > cap {
                return Err(NotEligible::MaxPerOrder);
            }
            desc(&mut picked);
            for chunk in picked.chunks(size as usize) {
                let rewards: i64 = chunk[buy as usize..].iter().map(|u| u.value).sum();
                chunks.push((chunk.to_vec(), pct_of(rewards, pct)));
            }
            times
        }
        Rule::BuyGet { buy, get, pct } => {
            in_given_order(&|l| in_pool(&deal.pool, l) || in_pool(&deal.reward_pool, l))?;
            let size = buy + get;
            if total_units == 0 || total_units % size != 0 {
                return Err(NotEligible::Count);
            }
            let times = total_units / size;
            // The rewards: the times×G cheapest reward units (a unit outside
            // the buy pool first, then the later line).
            let mut idx: Vec<usize> = (0..picked.len())
                .filter(|&k| in_pool(&deal.reward_pool, &lines[picked[k].pos]))
                .collect();
            let want = (times * get) as usize;
            if idx.len() < want {
                return Err(NotEligible::Count);
            }
            idx.sort_by(|&a, &b| {
                let (ua, ub) = (&picked[a], &picked[b]);
                ua.value
                    .cmp(&ub.value)
                    .then(
                        in_pool(&deal.pool, &lines[ua.pos])
                            .cmp(&in_pool(&deal.pool, &lines[ub.pos])),
                    )
                    .then(ub.line_index.cmp(&ua.line_index))
                    .then(b.cmp(&a))
            });
            idx.truncate(want);
            let mut rewards: Vec<Unit> = idx.iter().map(|&k| picked[k]).collect();
            let mut buys: Vec<Unit> = (0..picked.len())
                .filter(|k| !idx.contains(k))
                .map(|k| picked[k])
                .collect();
            if let Some(u) = buys.iter().find(|u| !in_pool(&deal.pool, &lines[u.pos])) {
                return Err(NotEligible::NotInPool {
                    line_index: u.line_index,
                });
            }
            if times > cap {
                return Err(NotEligible::MaxPerOrder);
            }
            desc(&mut buys);
            desc(&mut rewards);
            for (b, g) in buys.chunks(buy as usize).zip(rewards.chunks(get as usize)) {
                let disc = pct_of(g.iter().map(|u| u.value).sum(), pct);
                let mut chunk = b.to_vec();
                chunk.extend_from_slice(g);
                chunks.push((chunk, disc));
            }
            times
        }
    };

    let mut out: Vec<AppliedLine> = claimed
        .iter()
        .map(|&(line_index, _, units)| AppliedLine {
            line_index,
            units,
            discount: 0,
        })
        .collect();
    let mut discount = 0;
    for (chunk, disc) in &chunks {
        let weights: Vec<i64> = chunk.iter().map(|u| u.value).collect();
        for (u, cut) in chunk.iter().zip(alloc::split(*disc, &weights)) {
            if let Some(l) = out.iter_mut().find(|l| l.line_index == u.line_index) {
                l.discount += cut;
            }
        }
        discount += disc;
    }
    if discount <= 0 {
        return Err(NotEligible::NoSaving);
    }
    Ok(Application {
        deal_id: deal.id.clone(),
        times,
        discount,
        lines: out,
    })
}

/// QR and online checkout: the best suggestion, applied, its units consumed,
/// again until nothing saves. The server runs this on the order it is given;
/// the browser shows the same.
pub fn auto_apply(deals: &[DealView], lines: &[DealLine], ctx: &DealContext) -> Vec<Application> {
    let mut lines = lines.to_vec();
    let mut ctx = ctx.clone();
    let mut out = Vec::new();
    // Each round consumes at least one unit; the bound is a backstop.
    for _ in 0..10_000 {
        let Some(top) = suggest(deals, &lines, &ctx).into_iter().next() else {
            break;
        };
        let Some(deal) = deals.iter().find(|d| d.id == top.deal_id) else {
            break;
        };
        let Ok(app) = price_application(deal, &lines, &top.units, &ctx) else {
            break;
        };
        for u in &top.units {
            if let Some(l) = lines.iter_mut().find(|l| l.line_index == u.line_index) {
                l.quantity -= u.units;
            }
        }
        match ctx.used.iter_mut().find(|u| u.deal_id == deal.id) {
            Some(u) => u.times += app.times,
            None => ctx.used.push(UsedDeal {
                deal_id: deal.id.clone(),
                times: app.times,
            }),
        }
        out.push(app);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Deserialize)]
    struct Vectors {
        deals: BTreeMap<String, DealView>,
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Case {
        op: String,
        name: String,
        #[serde(default)]
        deal: Option<String>,
        #[serde(default)]
        deals: Vec<String>,
        lines: Vec<DealLine>,
        #[serde(default)]
        units: Vec<LineUnits>,
        ctx: DealContext,
        expected: serde_json::Value,
    }

    #[derive(Debug, PartialEq, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Priced {
        Application(Application),
        Refusal(NotEligible),
    }

    #[test]
    fn deal_vectors() {
        let v: Vectors = serde_json::from_str(crate::vectors::DEAL).unwrap();
        assert!(v.cases.len() >= 30);
        let one = |c: &Case| &v.deals[c.deal.as_ref().expect("deal")];
        let many =
            |c: &Case| -> Vec<DealView> { c.deals.iter().map(|k| v.deals[k].clone()).collect() };
        for c in &v.cases {
            let name = format!("{}/{}", c.op, c.name);
            match c.op.as_str() {
                "best" => {
                    let want: Option<Suggestion> =
                        serde_json::from_value(c.expected.clone()).unwrap();
                    assert_eq!(best(one(c), &c.lines, &c.ctx), want, "{name}");
                }
                "price" => {
                    let want: Priced = serde_json::from_value(c.expected.clone()).unwrap();
                    let got = match price_application(one(c), &c.lines, &c.units, &c.ctx) {
                        Ok(a) => Priced::Application(a),
                        Err(e) => Priced::Refusal(e),
                    };
                    assert_eq!(got, want, "{name}");
                    if let Priced::Application(a) = &got {
                        let sum: i64 = a.lines.iter().map(|l| l.discount).sum();
                        assert_eq!(sum, a.discount, "{name}: Σ line cuts");
                    }
                }
                "suggest" => {
                    let want: Vec<Suggestion> = serde_json::from_value(c.expected.clone()).unwrap();
                    assert_eq!(suggest(&many(c), &c.lines, &c.ctx), want, "{name}");
                }
                "auto_apply" => {
                    let want: Vec<Application> =
                        serde_json::from_value(c.expected.clone()).unwrap();
                    assert_eq!(auto_apply(&many(c), &c.lines, &c.ctx), want, "{name}");
                }
                other => panic!("{name}: unknown op {other}"),
            }
        }
    }

    #[test]
    fn a_feed_row_deserializes() {
        let row = serde_json::json!({
            "id": "d1", "name": "Any 2 bites for 90", "name_translations": {"ar": "x"},
            "kind": "n_for_price", "qty": 2, "price": 9000, "get_qty": null, "get_percent": null,
            "max_per_order": null, "sort": 0, "is_active": true,
            "pool": [{"menu_item_id": null, "category_id": "c", "size_label": null}],
            "reward_pool": [], "windows": [], "branch_overrides": []
        });
        let d: DealView = serde_json::from_value(row).unwrap();
        assert_eq!(d.qty, 2);
        assert_eq!(d.pool[0].category_id.as_deref(), Some("c"));
        assert_eq!(NotEligible::Count.token(), "count");
    }
}
