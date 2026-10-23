//! # madar-web
//!
//! The madar-shared rules in the browser: `wasm-bindgen` bindings over the
//! crates, so the web dashboard and the customer pages call the same Rust as
//! the backend and the POS core. Every export is one call into a crate; this
//! crate holds no rule of its own (SHARED_RULES_PLAN.md, Step 3).
//!
//! Three builds (scripts/build-wasm.sh):
//! - `public`: the customer pages (ordering, menu, loyalty) and the plan's
//!   entitlements.
//! - `full`: the web dashboard; everything in `public` and more.
//! - `menu`: the self-serve menu import (madar-menu: the Foodics reader, the
//!   bulk menu's checks, the replace sentence), loaded by the dashboard when
//!   its import dialog opens; nothing else.
//! - `card`: the Rewards card designer (madar-card: templates, validation,
//!   layout, the strip's SVG, tracks, audiences, messages), loaded by the
//!   dashboard's Rewards pages; nothing else.
//!
//! The boundary:
//! - Structs and enums cross as plain JS objects (serde-wasm-bindgen), typed
//!   by the declarations `tsify` generates from the crates' own types.
//!   Answers use the API's JSON shape: a missing value is `null`.
//! - A refusal the crate models (`PriceError`, `ComboRefusal`, …) is
//!   RETURNED, as the crate serialises it (`{ error: … }`, `{ refusal: … }`):
//!   the result type is a union. Input that is not what the type says throws.
//! - Whole numbers (piastres, counts) are JS numbers and must be whole.
//! - Instants are epoch milliseconds, never text. Calendar dates are
//!   `YYYY-MM-DD`.
//! - Time zones are IANA names: `full` bundles every zone, `public` Cairo,
//!   MENA and the US (scripts/tz-filter.txt). A zone not bundled throws.

#![cfg(any(feature = "public", feature = "menu", feature = "card"))]

use serde::Serialize;
use tsify::Ts;
use wasm_bindgen::prelude::*;

/// Every answer goes out through one serializer: `None` as `null` (the
/// API's JSON shape) and maps (`#[serde(flatten)]`) as plain objects.
fn out<T: Serialize + ?Sized>(v: &T) -> Result<JsValue, JsError> {
    v.serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|e| JsError::new(&e.to_string()))
}

/// A crate's `Result`, its refusal returned rather than thrown.
#[cfg(any(feature = "public", feature = "menu"))]
fn either<T: Serialize, E: Serialize>(r: Result<T, E>) -> Result<JsValue, JsError> {
    match r {
        Ok(v) => out(&v),
        Err(e) => out(&e),
    }
}

#[cfg(any(feature = "public", feature = "card"))]
/// A JS number that must be a whole number (piastres, counts): anything else
/// throws instead of being truncated.
fn int<T: TryFrom<i64>>(x: f64) -> Result<T, JsError> {
    const MAX_SAFE: f64 = 9_007_199_254_740_991.0;
    if x.fract() == 0.0 && x.abs() <= MAX_SAFE {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "x is whole and within 2^53, checked above: the cast is exact"
        )]
        let x = x as i64;
        if let Ok(v) = T::try_from(x) {
            return Ok(v);
        }
    }
    Err(JsError::new("expected a whole number in range"))
}

#[cfg(feature = "public")]
/// Each element of a JS array of typed objects.
fn each<T>(v: Vec<Ts<T>>) -> Result<Vec<T>, JsError>
where
    T: tsify::Tsify + serde::de::DeserializeOwned,
    T::JsType: Clone,
{
    v.iter().map(|t| Ok(t.to_rust()?)).collect()
}

#[cfg(feature = "public")]
fn zone(name: &str) -> Result<chrono_tz::Tz, JsError> {
    name.parse()
        .map_err(|_| JsError::new("unknown time zone, or one this build leaves out"))
}

#[cfg(feature = "public")]
fn ymd(d: chrono::NaiveDate) -> String {
    use chrono::Datelike;
    format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day())
}

#[cfg(feature = "public")]
fn instant(ms: f64) -> Result<chrono::DateTime<chrono::Utc>, JsError> {
    chrono::DateTime::from_timestamp_millis(int(ms)?)
        .ok_or_else(|| JsError::new("epoch milliseconds out of range"))
}

#[cfg(feature = "public")]
mod public {
    use madar_catalog::combo::{ComboView, MenuChoice, PickIn};
    use madar_catalog::{CatalogView, ItemView, Selection};
    use madar_money::line::{CartLineShape, PickShape};
    use tsify::Tsify;

    use super::*;

    /// The branch-local business date of an instant (epoch ms).
    #[wasm_bindgen]
    pub fn business_date(tz: &str, at_ms: f64) -> Result<String, JsError> {
        Ok(ymd(madar_time::business_date_of(
            zone(tz)?,
            instant(at_ms)?,
        )))
    }

    /// One unit of the item at `size_label` before any option (madar-catalog
    /// `unit_price`).
    #[wasm_bindgen(unchecked_return_type = "number | PriceError")]
    pub fn unit_price(item: Ts<ItemView>, size_label: Option<String>) -> Result<JsValue, JsError> {
        either(madar_catalog::unit_price(
            &item.to_rust()?,
            size_label.as_deref(),
        ))
    }

    /// A line's options and optional fields, priced without its size.
    #[wasm_bindgen(unchecked_return_type = "PricedOptions | PriceError")]
    pub fn price_options(
        view: Ts<CatalogView>,
        selection: Ts<Selection>,
    ) -> Result<JsValue, JsError> {
        either(madar_catalog::price_options(
            &view.to_rust()?,
            &selection.to_rust()?,
        ))
    }

    /// A menu-item line: its size price, then its options.
    #[wasm_bindgen(unchecked_return_type = "PricedLine | PriceError")]
    pub fn price_line(view: Ts<CatalogView>, selection: Ts<Selection>) -> Result<JsValue, JsError> {
        either(madar_catalog::price_line(
            &view.to_rust()?,
            &selection.to_rust()?,
        ))
    }

    /// What `option_id` alone is charged on a line of `size_label`; `null`
    /// for an option not in the view.
    #[wasm_bindgen(unchecked_return_type = "number | null")]
    pub fn option_charge(
        view: Ts<CatalogView>,
        size_label: Option<String>,
        option_id: &str,
    ) -> Result<JsValue, JsError> {
        out(&madar_catalog::option_charge(
            &view.to_rust()?,
            size_label.as_deref(),
            option_id,
        ))
    }

    /// A combo line of `n` units (madar-catalog `combo::quote`).
    #[wasm_bindgen(unchecked_return_type = "ComboQuote | ComboRefusal")]
    pub fn combo_quote(
        combo: Ts<ComboView>,
        picks: Vec<Ts<PickIn>>,
        n: f64,
    ) -> Result<JsValue, JsError> {
        either(madar_catalog::combo::quote(
            &combo.to_rust()?,
            &each(picks)?,
            int(n)?,
        ))
    }

    /// What a stored discount rule (`discount_type`, `value` as a decimal
    /// string: `"0.10"` is 10 % off) takes off `subtotal`, clamped to
    /// `[0, subtotal]` (madar-money `bill::rule_of` + `discount_on`).
    #[wasm_bindgen]
    pub fn bill_discount(
        subtotal: f64,
        discount_type: Option<String>,
        value: &str,
    ) -> Result<f64, JsError> {
        use madar_money::bill::{discount_on, rule_of, BillDiscount};
        let value = value
            .parse::<rust_decimal::Decimal>()
            .map_err(|_| JsError::new("expected a decimal string"))?;
        let rule = rule_of(discount_type.as_deref(), value);
        Ok(discount_on(int(subtotal)?, BillDiscount::Rule(rule)) as f64)
    }

    // ── the storefront cart (madar-money `line`, madar-catalog `combo`) ──

    /// One unit of a cart line: an item's unit price and its extras, or a
    /// combo's price and what its picks add.
    #[wasm_bindgen]
    pub fn cart_line_unit(line: Ts<CartLineShape>) -> Result<f64, JsError> {
        Ok(madar_money::line::cart_line_unit(&line.to_rust()?) as f64)
    }

    /// What a cart line is charged: one unit × its quantity.
    #[wasm_bindgen]
    pub fn cart_line_total(line: Ts<CartLineShape>) -> Result<f64, JsError> {
        Ok(madar_money::line::cart_line_total(&line.to_rust()?) as f64)
    }

    /// A cart's items total, before any deal, discount, tax or fee (the
    /// cart quote's `items_total`).
    #[wasm_bindgen]
    pub fn cart_subtotal(lines: Vec<Ts<CartLineShape>>) -> Result<f64, JsError> {
        Ok(madar_money::line::cart_subtotal(&each(lines)?) as f64)
    }

    /// What a combo's picks add to ONE combo unit.
    #[wasm_bindgen]
    pub fn combo_extras(picks: Vec<Ts<PickShape>>) -> Result<f64, JsError> {
        Ok(madar_money::line::combo_extras(&each(picks)?) as f64)
    }

    /// What one unit of a pick adds to the combo's price, from the public
    /// menu's choice (its surcharge plus its size's extra); `null` is the
    /// included size.
    #[wasm_bindgen]
    pub fn combo_choice_extra(
        choice: Ts<MenuChoice>,
        size_label: Option<String>,
    ) -> Result<f64, JsError> {
        Ok(madar_catalog::combo::choice_extra(&choice.to_rust()?, size_label.as_deref()) as f64)
    }

    // ── the menu's order (madar-catalog `order`) ──

    /// The positions of `categories` in the order the POS shows them to a
    /// reader in `locale`: `display_order` (missing or `null` is 0), then the
    /// name they read, by code point, then `id`. Pass the API's categories as
    /// they are (other fields are ignored) and read them back with
    /// `order.map((i) => categories[i])`.
    #[wasm_bindgen(unchecked_return_type = "number[]")]
    pub fn category_order(
        categories: Vec<Ts<madar_catalog::order::Category>>,
        locale: &str,
    ) -> Result<JsValue, JsError> {
        out(&madar_catalog::order::category_order(
            &each(categories)?,
            locale,
        ))
    }

    // ── the loyalty card (madar-loyalty `card`) ──

    /// A member's card for a balance against the next reward's cost.
    #[derive(Serialize, Tsify)]
    pub struct LoyaltyCard {
        /// Rewards the balance has already earned.
        pub rewards_ready: i32,
        /// Progress towards the next reward, the earned ones set aside.
        pub progress_to_next: i32,
        /// What the next reward still needs: the full cost on an exact multiple.
        pub points_to_next_reward: i32,
        /// At least one reward is earned.
        pub can_redeem: bool,
    }

    /// The card for `balance` against `next_reward_cost`.
    #[wasm_bindgen(unchecked_return_type = "LoyaltyCard")]
    pub fn loyalty_card(balance: f64, next_reward_cost: f64) -> Result<JsValue, JsError> {
        let c = madar_loyalty::card::card(int(balance)?, int(next_reward_cost)?);
        out(&LoyaltyCard {
            rewards_ready: c.rewards_ready,
            progress_to_next: c.progress_to_next,
            points_to_next_reward: c.points_to_next_reward,
            can_redeem: c.can_redeem,
        })
    }

    /// How many steps of a `cost`-step stamp row `earned` fills; `null` when
    /// the card has no row (a cost of 0 or less, or above 12).
    #[wasm_bindgen(unchecked_return_type = "number | null")]
    pub fn loyalty_stamps(earned: f64, cost: f64) -> Result<JsValue, JsError> {
        out(&madar_loyalty::card::stamps(int(earned)?, int(cost)?))
    }

    // ── the plan (madar-plan) ──

    /// What a plan switches on: its features, limits (`null` is unlimited) and
    /// whether its tills hold a realtime stream. `"free"` is Free; any other
    /// value is Paid, so an old server never locks a paying shop out.
    #[wasm_bindgen(unchecked_return_type = "Entitlements")]
    pub fn entitlements(plan: &str) -> Result<JsValue, JsError> {
        out(&madar_plan::entitlements(madar_plan::Plan::parse(plan)))
    }

    /// The canonical phone (E.164 digits, no `+`), or `null` when `raw` is
    /// not a phone number.
    #[wasm_bindgen(unchecked_return_type = "string | null")]
    pub fn phone_canonical(raw: &str) -> Result<JsValue, JsError> {
        out(&madar_ids::phone::canonical(raw))
    }

    /// What a scanned or typed string is: `{kind: "token" | "phone" | "partial",
    /// value}`. Feed it every keystroke of a wedge scanner and act once `kind`
    /// is not `partial`.
    #[wasm_bindgen(
        unchecked_return_type = "{ kind: 'token' | 'phone' | 'partial'; value: string }"
    )]
    pub fn scan_classify(raw: &str) -> Result<JsValue, JsError> {
        out(&madar_ids::scan::classify(raw))
    }
}

#[cfg(feature = "full")]
mod full {
    use chrono::NaiveDate;
    use madar_catalog::combo::SlotView;
    use madar_inventory::replenish::Input as ReplenishInput;
    use madar_inventory::transfer::{Action, Side, TransferStatus};
    use madar_money::cost::CostLine;
    use rust_decimal::prelude::FromPrimitive;
    use rust_decimal::Decimal;
    use serde::Deserialize;
    use tsify::Tsify;

    use super::*;

    /// `YYYY-MM-DD`, read by hand: chrono's text parser would add to the
    /// download for one format.
    fn date(s: &str) -> Result<NaiveDate, JsError> {
        let mut p = s.splitn(3, '-');
        let mut num = || p.next().and_then(|x| x.parse::<u32>().ok());
        let (y, m, d) = (num(), num(), num());
        y.zip(m)
            .zip(d)
            .and_then(|((y, m), d)| NaiveDate::from_ymd_opt(i32::try_from(y).ok()?, m, d))
            .ok_or_else(|| JsError::new("expected a YYYY-MM-DD date"))
    }

    // ── time (madar-time) ────────────────────────────────────────────────

    /// A branch-local calendar day as `[start, end)` in epoch ms (DST days
    /// are 23 or 25 hours).
    #[wasm_bindgen(unchecked_return_type = "[number, number]")]
    pub fn day_bounds(tz: &str, date: &str) -> Result<JsValue, JsError> {
        let (a, b) = madar_time::day_bounds(zone(tz)?, self::date(date)?);
        out(&(a.timestamp_millis(), b.timestamp_millis()))
    }

    /// The Saturday the week holding `date` starts on.
    #[wasm_bindgen]
    pub fn week_start(date: &str) -> Result<String, JsError> {
        Ok(ymd(madar_time::week_start(self::date(date)?)))
    }

    /// An instant (epoch ms) on a zone's wall clock.
    #[derive(Serialize, Tsify)]
    pub struct LocalParts {
        pub date: String,
        pub hour: u32,
        pub minute: u32,
    }

    /// An instant (epoch ms) read on `tz`'s wall clock.
    #[wasm_bindgen(unchecked_return_type = "LocalParts")]
    pub fn local_parts(tz: &str, at_ms: f64) -> Result<JsValue, JsError> {
        let p = madar_time::local_parts(zone(tz)?, instant(at_ms)?);
        out(&LocalParts {
            date: ymd(p.date),
            hour: p.hour,
            minute: p.minute,
        })
    }

    /// `date` at `hour`:`minute` on `tz`'s wall clock, in epoch ms: a time
    /// that happens twice is the earliest, one in a DST gap moves forward by
    /// the gap, an hour or minute past its range rolls over.
    #[wasm_bindgen]
    pub fn local_instant(tz: &str, date: &str, hour: f64, minute: f64) -> Result<f64, JsError> {
        let at = madar_time::local_instant(zone(tz)?, self::date(date)?, int(hour)?, int(minute)?);
        Ok(at.timestamp_millis() as f64)
    }

    // ── Dawam pay (madar-dawam) ──────────────────────────────────────────

    /// The pay window `[start, end]` (inclusive dates) holding `day`;
    /// `start_day` is clamped to 1–28.
    #[wasm_bindgen(unchecked_return_type = "[string, string]")]
    pub fn pay_period(day: &str, start_day: f64) -> Result<JsValue, JsError> {
        let (a, b) = madar_dawam::pay::period_window(date(day)?, int(start_day)?);
        out(&(ymd(a), ymd(b)))
    }

    /// A decimal the dashboard holds as a JS number, read as the decimal it
    /// prints as (`0.35` is 0.35); a non-finite number throws.
    fn decimal(x: f64) -> Result<Decimal, JsError> {
        Decimal::from_f64(x).ok_or_else(|| JsError::new("expected a finite number"))
    }

    /// The one rate the salary calculator was typed with, in piastres.
    #[derive(Deserialize, Tsify)]
    #[serde(rename_all = "snake_case")]
    pub enum TypedRate {
        Monthly(i64),
        Daily(i64),
        Hourly(i64),
    }

    /// A salary as a month, a day and an hour, in piastres.
    #[derive(Serialize, Tsify)]
    pub struct SalaryRates {
        pub monthly: i64,
        pub daily: i64,
        pub hourly: i64,
    }

    /// The three rates from whichever one was typed (madar-dawam
    /// `salary::rates`); `working_days` may be a fraction.
    #[wasm_bindgen(unchecked_return_type = "SalaryRates")]
    pub fn rates(
        typed: Ts<TypedRate>,
        working_days: f64,
        day_minutes: f64,
    ) -> Result<JsValue, JsError> {
        use madar_dawam::salary::Typed;
        let typed = match typed.to_rust()? {
            TypedRate::Monthly(p) => Typed::Monthly(p),
            TypedRate::Daily(p) => Typed::Daily(p),
            TypedRate::Hourly(p) => Typed::Hourly(p),
        };
        let r = madar_dawam::salary::rates(typed, decimal(working_days)?, int(day_minutes)?);
        out(&SalaryRates {
            monthly: r.monthly,
            daily: r.daily,
            hourly: r.hourly,
        })
    }

    /// What someone hired on a day earns in their first pay period.
    #[derive(Serialize, Tsify)]
    pub struct FirstPay {
        /// The hire date.
        pub from: String,
        /// The period's last day.
        pub to: String,
        pub days: i64,
        pub period_days: i64,
        pub piastres: i64,
    }

    /// The first pay of someone hired on `hire_date` at `monthly`, periods
    /// opening on `start_day` (madar-dawam `salary::first_pay`).
    #[wasm_bindgen(unchecked_return_type = "FirstPay")]
    pub fn first_pay(monthly: f64, hire_date: &str, start_day: f64) -> Result<JsValue, JsError> {
        let f = madar_dawam::salary::first_pay(int(monthly)?, date(hire_date)?, int(start_day)?);
        out(&FirstPay {
            from: ymd(f.from),
            to: ymd(f.to),
            days: f.days,
            period_days: f.period_days,
            piastres: f.piastres,
        })
    }

    /// One rung of the late ladder as the Rules page holds it: inclusive at
    /// both ends, `to_minutes` `null` for the open top rung.
    #[derive(Deserialize, Tsify)]
    #[tsify(missing_as_null)]
    pub struct LateTier {
        pub from_minutes: i32,
        pub to_minutes: Option<i32>,
        #[tsify(type = "\"minutes\" | \"piastres\" | \"day_fraction\"")]
        pub kind: madar_dawam::ladder::LateDeductionKind,
        pub value: f64,
    }

    fn tier(t: LateTier) -> Result<madar_dawam::ladder::LateTier, JsError> {
        Ok(madar_dawam::ladder::LateTier {
            from_minutes: t.from_minutes,
            to_minutes: t.to_minutes,
            kind: t.kind,
            value: decimal(t.value)?,
        })
    }

    fn pay_rates(
        salary: f64,
        working_days: f64,
        day_minutes: f64,
    ) -> Result<madar_dawam::salary::PayRates, JsError> {
        Ok(madar_dawam::salary::PayRates::from_base(
            int(salary)?,
            decimal(working_days)?,
            int(day_minutes)?,
        ))
    }

    /// The index of the FIRST rung `late_minutes` falls on, or `null` (on
    /// time, or past a ladder that stops).
    #[wasm_bindgen(unchecked_return_type = "number | null")]
    pub fn select_late_tier(
        tiers: Vec<Ts<LateTier>>,
        late_minutes: f64,
    ) -> Result<JsValue, JsError> {
        let tiers = each(tiers)?
            .into_iter()
            .map(tier)
            .collect::<Result<Vec<_>, _>>()?;
        let hit = madar_dawam::ladder::select_late_tier(&tiers, int(late_minutes)?);
        out(&hit.and_then(|t| tiers.iter().position(|x| core::ptr::eq(x, t))))
    }

    /// What a rung costs in piastres for a monthly `salary`, `working_days` a
    /// month and the day's `day_minutes` (madar-dawam
    /// `ladder::late_deduction_piastres`).
    #[wasm_bindgen]
    pub fn late_deduction_piastres(
        tier: Ts<LateTier>,
        salary: f64,
        working_days: f64,
        day_minutes: f64,
    ) -> Result<f64, JsError> {
        let rates = pay_rates(salary, working_days, day_minutes)?;
        Ok(
            madar_dawam::ladder::late_deduction_piastres(&self::tier(tier.to_rust()?)?, &rates)
                as f64,
        )
    }

    /// What `days_absent` absent days cost at `deduction_days` docked each,
    /// for a monthly `salary` over `working_days` (madar-dawam
    /// `ladder::absence_deduction_piastres`; the day's minutes do not enter).
    #[wasm_bindgen]
    pub fn absence_deduction_piastres(
        salary: f64,
        working_days: f64,
        days_absent: f64,
        deduction_days: f64,
    ) -> Result<f64, JsError> {
        Ok(madar_dawam::ladder::absence_deduction_piastres(
            &pay_rates(salary, working_days, 0.0)?,
            decimal(days_absent)?,
            decimal(deduction_days)?,
        ) as f64)
    }

    // ── units (madar-units) ──────────────────────────────────────────────

    /// A unit's `[family, factor to the family's canonical unit]`, or `null`.
    #[wasm_bindgen(unchecked_return_type = "[string, number] | null")]
    pub fn unit_spec(unit: &str) -> Result<JsValue, JsError> {
        out(&madar_units::unit_spec(unit))
    }

    /// The units a quantity stocked in `base` may be typed in.
    #[wasm_bindgen]
    pub fn units_of(base: &str) -> Vec<String> {
        madar_units::units_of(base)
    }

    /// The server's 400 message for a quantity it cannot convert.
    #[derive(Serialize, Tsify)]
    pub struct UnitRefusal {
        pub error: String,
    }

    fn quantity(r: Result<f64, madar_units::UnitError>) -> Result<JsValue, JsError> {
        either(r.map_err(|e| UnitRefusal {
            error: e.to_string(),
        }))
    }

    /// `qty` in `from_unit` as `to_unit`, 3 dp; across families a refusal.
    #[wasm_bindgen(unchecked_return_type = "number | UnitRefusal")]
    pub fn convert(qty: f64, from_unit: &str, to_unit: &str) -> Result<JsValue, JsError> {
        quantity(madar_units::convert(qty, from_unit, to_unit))
    }

    /// [`convert`], with mass↔volume bridged by a positive density (g/ml).
    #[wasm_bindgen(unchecked_return_type = "number | UnitRefusal")]
    pub fn convert_with_density(
        qty: f64,
        from_unit: &str,
        to_unit: &str,
        density_g_per_ml: Option<f64>,
    ) -> Result<JsValue, JsError> {
        quantity(madar_units::convert_with_density(
            qty,
            from_unit,
            to_unit,
            density_g_per_ml,
        ))
    }

    /// What a recipe line stores: converted to the base unit, grossed up by
    /// the yield loss, 3 dp.
    #[wasm_bindgen(unchecked_return_type = "number | UnitRefusal")]
    pub fn recipe_base_qty(
        qty: f64,
        unit: &str,
        base_unit: &str,
        density_g_per_ml: Option<f64>,
        yield_pct: Option<f64>,
    ) -> Result<JsValue, JsError> {
        quantity(madar_units::recipe_base_qty(
            qty,
            unit,
            base_unit,
            density_g_per_ml,
            yield_pct,
        ))
    }

    /// The usable amount a stored recipe quantity stands for, 3 dp.
    #[wasm_bindgen]
    pub fn usable_qty(stored: f64, yield_pct: Option<f64>) -> f64 {
        madar_units::usable_qty(stored, yield_pct)
    }

    /// A recipe quantity copied to another size × `factor`, 3 dp, half away
    /// from zero.
    #[wasm_bindgen]
    pub fn scale_qty(qty: f64, factor: f64) -> f64 {
        madar_units::scale_qty(qty, factor)
    }

    // ── money (madar-money) ──────────────────────────────────────────────

    /// Net sales over orders, rounded half up; 0 with no orders.
    #[wasm_bindgen]
    pub fn average_ticket(net_sales: f64, order_count: f64) -> Result<f64, JsError> {
        Ok(madar_money::metrics::average_ticket(int(net_sales)?, int(order_count)?) as f64)
    }

    /// One recipe line's cost in whole piastres.
    #[wasm_bindgen]
    pub fn line_cost(qty: f64, cost_per_unit: f64) -> f64 {
        madar_money::cost::line_cost(qty, cost_per_unit) as f64
    }

    /// A recipe's cost: the known lines' exact sum, rounded once.
    #[wasm_bindgen(unchecked_return_type = "RecipeCost")]
    pub fn recipe_cost(lines: Vec<Ts<CostLine>>) -> Result<JsValue, JsError> {
        out(&madar_money::cost::recipe_cost(&each(lines)?))
    }

    /// `(price − cost) / price`; `null` unless `price > 0`.
    #[wasm_bindgen(unchecked_return_type = "number | null")]
    pub fn margin(price: f64, cost: f64) -> Result<JsValue, JsError> {
        out(&madar_money::cost::margin(int(price)?, int(cost)?))
    }

    /// The food-cost band of `cost` against `price`; `null` unless `price > 0`.
    #[wasm_bindgen(unchecked_return_type = "Band | null")]
    pub fn food_cost_band(cost: f64, price: f64) -> Result<JsValue, JsError> {
        out(&madar_money::cost::food_cost_band(int(cost)?, int(price)?))
    }

    // ── till close (madar-till) ──────────────────────────────────────────

    /// What the system says one payment method took on a till.
    #[derive(Deserialize, Tsify)]
    #[tsify(missing_as_null)]
    pub struct TillTotal {
        pub method: String,
        pub payment_method_id: Option<String>,
        pub is_cash: bool,
        pub system_total: i64,
        pub order_count: i64,
    }

    /// What the teller said about one method: `checked` | `disagreed`.
    #[derive(Deserialize, Tsify)]
    #[tsify(missing_as_null)]
    pub struct TillInput {
        pub method: String,
        pub status: String,
        pub declared_amount: Option<i32>,
        pub note: Option<String>,
    }

    /// A reconciliation line to store.
    #[derive(Serialize, Tsify)]
    #[tsify(missing_as_null)]
    pub struct TillLine {
        pub method: String,
        pub payment_method_id: Option<String>,
        pub is_cash: bool,
        pub system_total: i32,
        pub order_count: i32,
        pub status: String,
        pub declared_amount: Option<i32>,
        pub note: Option<String>,
    }

    /// Why a live close's inputs were refused: `code` is the API's
    /// (`RECONCILIATION_AMOUNT_REQUIRED`, `RECONCILIATION_NOTE_REQUIRED`), or
    /// `null` for a status that is neither `checked` nor `disagreed`.
    #[derive(Serialize, Tsify)]
    #[tsify(missing_as_null)]
    pub struct TillRefusal {
        pub code: Option<String>,
        pub method: String,
    }

    /// The close's reconciliation lines (madar-till `reconcile::plan_lines`):
    /// the cash line first, then each non-cash method used, then inputs
    /// naming a method not used. `replay` never refuses.
    #[wasm_bindgen(unchecked_return_type = "TillLine[] | TillRefusal")]
    pub fn till_plan_lines(
        totals: Vec<Ts<TillTotal>>,
        closing_cash_declared: f64,
        closing_cash_system: f64,
        cash_note: Option<String>,
        inputs: Vec<Ts<TillInput>>,
        replay: bool,
    ) -> Result<JsValue, JsError> {
        use madar_till::reconcile::{plan_lines, Input, MethodTotal, PlanError};
        let totals: Vec<MethodTotal<String>> = each(totals)?
            .into_iter()
            .map(|t| MethodTotal {
                method: t.method,
                payment_method_id: t.payment_method_id,
                is_cash: t.is_cash,
                system_total: t.system_total,
                order_count: t.order_count,
            })
            .collect();
        let inputs = each(inputs)?;
        let inputs: Vec<Input<'_>> = inputs
            .iter()
            .map(|i| Input {
                method: &i.method,
                status: &i.status,
                declared_amount: i.declared_amount,
                note: i.note.as_deref(),
            })
            .collect();
        let planned = plan_lines(
            &totals,
            int(closing_cash_declared)?,
            int(closing_cash_system)?,
            cash_note.as_deref(),
            &inputs,
            replay,
        );
        either(
            planned
                .map(|lines| {
                    lines
                        .into_iter()
                        .map(|l| TillLine {
                            method: l.method,
                            payment_method_id: l.payment_method_id,
                            is_cash: l.is_cash,
                            system_total: l.system_total,
                            order_count: l.order_count,
                            status: l.status.to_string(),
                            declared_amount: l.declared_amount,
                            note: l.note,
                        })
                        .collect::<Vec<_>>()
                })
                .map_err(|e| TillRefusal {
                    code: e.code().map(str::to_string),
                    method: match e {
                        PlanError::AmountRequired { method }
                        | PlanError::NoteRequired { method }
                        | PlanError::InvalidStatus { method, .. } => method,
                    },
                }),
        )
    }

    // ── inventory (madar-inventory) ──────────────────────────────────────

    /// An allowed transfer action: the side that takes it, the capability
    /// it needs (its key) and the status it leads to.
    #[derive(Serialize, Tsify)]
    pub struct TransferStep {
        pub side: Side,
        pub cap: &'static str,
        pub next: TransferStatus,
    }

    /// What `action` needs and leads to in `status`; `null` when it is not
    /// open there.
    #[wasm_bindgen(unchecked_return_type = "TransferStep | null")]
    pub fn transfer_step(
        status: Ts<TransferStatus>,
        action: Ts<Action>,
    ) -> Result<JsValue, JsError> {
        out(
            &madar_inventory::transfer::step(status.to_rust()?, action.to_rust()?).map(|s| {
                TransferStep {
                    side: s.side,
                    cap: s.cap.key(),
                    next: s.next,
                }
            }),
        )
    }

    /// One received line, judged (a note of only whitespace is no note).
    #[wasm_bindgen(unchecked_return_type = "LineCheck | ReceiveRefusal")]
    pub fn check_receive_line(
        qty_sent: f64,
        qty_received: f64,
        note: Option<String>,
    ) -> Result<JsValue, JsError> {
        either(madar_inventory::transfer::check_receive_line(
            qty_sent,
            qty_received,
            note.as_deref(),
        ))
    }

    /// How much a warehouse should send a branch.
    #[wasm_bindgen(unchecked_return_type = "ReplenishSuggestion")]
    pub fn replenish_suggest(input: Ts<ReplenishInput>) -> Result<JsValue, JsError> {
        out(&madar_inventory::replenish::suggest(&input.to_rust()?))
    }

    /// A quantity as `numeric(12,3)` holds it: 3 dp, half away from zero.
    #[wasm_bindgen]
    pub fn quantity_dec(q: f64) -> f64 {
        use rust_decimal::prelude::ToPrimitive;
        madar_inventory::purchase::quantity_dec(q)
            .to_f64()
            .unwrap_or(0.0)
    }

    /// A quantity in whole thousandths, as `numeric(12,3)` stores it (half
    /// away from zero; non-finite is 0).
    #[wasm_bindgen]
    pub fn quantity_milli(q: f64) -> f64 {
        madar_inventory::purchase::milli(q) as f64
    }

    /// The server's 400 message for a purchase cost it refuses.
    #[derive(Serialize, Tsify)]
    pub struct PurchaseRefusal {
        pub error: String,
    }

    /// Piastres one delivery cost, not rounded: the invoice total if given,
    /// else the per-unit price × the quantity, else the ordered line total
    /// pro rata to the quantity received (the receive dialog's hint).
    /// `quantity_ordered` is the stored column.
    #[wasm_bindgen(unchecked_return_type = "number | PurchaseRefusal")]
    pub fn delivery_cost(
        quantity_received: f64,
        line_cost: Option<f64>,
        unit_cost: Option<f64>,
        ordered_line_cost: f64,
        quantity_ordered: f64,
    ) -> Result<JsValue, JsError> {
        use madar_inventory::purchase::quantity_dec;
        use rust_decimal::prelude::ToPrimitive;
        either(
            madar_inventory::purchase::delivery_cost(
                quantity_received,
                line_cost.map(int).transpose()?,
                unit_cost.map(int).transpose()?,
                int(ordered_line_cost)?,
                quantity_dec(quantity_ordered),
            )
            .map(|d| d.to_f64().unwrap_or(0.0))
            .map_err(|e| PurchaseRefusal {
                error: e.to_string(),
            }),
        )
    }

    /// The order dialog's line estimate in piastres; `null` without a cost,
    /// a quantity above 0, or units of one family.
    #[wasm_bindgen(unchecked_return_type = "number | null")]
    pub fn estimate_line_total(
        cost_per_stock_unit: Option<f64>,
        qty: f64,
        purchase_unit: &str,
        stock_unit: &str,
    ) -> Result<JsValue, JsError> {
        out(&madar_inventory::purchase::estimate_line_total(
            cost_per_stock_unit,
            qty,
            purchase_unit,
            stock_unit,
        ))
    }

    /// The unit cost (8 dp) a typed line total implies; `null` unless
    /// `line ≥ 0` and the 3 dp quantity is above 0.
    #[wasm_bindgen(unchecked_return_type = "number | null")]
    pub fn unit_cost_from_total(line: f64, qty: f64) -> Result<JsValue, JsError> {
        out(&madar_inventory::purchase::unit_cost_from_total(
            int(line)?,
            qty,
        ))
    }

    /// Whether a counted row needs a reason: off by at least `pct` % of the
    /// book, or stock from zero.
    #[wasm_bindgen]
    pub fn is_variance_flagged(book: f64, counted: f64, pct: f64) -> bool {
        madar_inventory::count::is_variance_flagged(book, counted, pct)
    }

    // ── catalog, dashboard only ──────────────────────────────────────────

    /// The choice of `slot` that admits an item: its own item choice first,
    /// else the category choice; `null` when none does.
    #[wasm_bindgen(unchecked_return_type = "ChoiceView | null")]
    pub fn combo_choice_for(
        slot: Ts<SlotView>,
        menu_item_id: &str,
        category_id: Option<String>,
    ) -> Result<JsValue, JsError> {
        out(&madar_catalog::combo::choice_for(
            &slot.to_rust()?,
            menu_item_id,
            category_id.as_deref(),
        ))
    }
}

#[cfg(feature = "menu")]
mod menu {
    use super::*;

    /// A Foodics console export read into a bulk menu: each file's text, in
    /// any order. A file that cannot be read is returned, not thrown.
    #[wasm_bindgen(unchecked_return_type = "FoodicsParsed | FoodicsError")]
    pub fn foodics_parse(files: Vec<String>) -> Result<JsValue, JsError> {
        let refs: Vec<&str> = files.iter().map(String::as_str).collect();
        either(madar_menu::foodics::parse(&refs))
    }

    /// The sentence an owner types to replace the whole menu, in `lang`.
    #[wasm_bindgen]
    pub fn replace_sentence(org_name: &str, lang: &str) -> String {
        madar_menu::replace_sentence(org_name, lang)
    }

    /// Whether `typed` is that sentence, in either language.
    #[wasm_bindgen]
    pub fn confirms_replace(org_name: &str, typed: &str) -> bool {
        madar_menu::confirms_replace(org_name, typed)
    }

    /// The server's checks on a bulk menu, for the preview.
    #[wasm_bindgen(unchecked_return_type = "BulkSummary | BulkRefusal")]
    pub fn bulk_validate(menu: Ts<madar_menu::BulkMenu>) -> Result<JsValue, JsError> {
        either(madar_menu::validate(&menu.to_rust()?))
    }
}

#[cfg(feature = "card")]
mod card {
    use super::*;
    use madar_card::audience::{validate_audience, Audience};
    use madar_card::grid::{Canvas, Cell, Store, APPLE_PRIMARY_ZONE, COLS, FOOTER_CELL, ROWS};
    use madar_card::message::{validate_message, Limits, Message, LIMITS};
    use madar_card::render::{CardState, SvgOpts};
    use madar_card::tracks::{Mode, Track};
    use madar_card::{CardDesign, Colours, Ctx};

    fn tracks_of(v: Vec<Ts<Track>>) -> Result<Vec<Track>, JsError> {
        v.iter().map(|t| Ok(t.to_rust()?)).collect()
    }

    /// The fixed numbers the editor and the composer show.
    #[derive(Serialize, tsify::Tsify)]
    pub struct CardRules {
        pub cols: u8,
        pub rows: u8,
        pub apple_primary_zone: Cell,
        pub footer_cell: Cell,
        pub template_ids: Vec<String>,
        pub max_elements: usize,
        pub max_stamps: i32,
        pub text_max: usize,
        pub max_header: usize,
        pub max_secondary_and_auxiliary: usize,
        pub min_contrast: f64,
        pub max_tracks: usize,
        pub max_track_size: i32,
        pub track_name_max: usize,
        pub audience_max_rules: usize,
        pub audience_max_days: u32,
        pub message: Limits,
    }

    #[wasm_bindgen(unchecked_return_type = "CardRules")]
    pub fn card_rules() -> Result<JsValue, JsError> {
        use madar_card::validate as v;
        out(&CardRules {
            cols: COLS,
            rows: ROWS,
            apple_primary_zone: APPLE_PRIMARY_ZONE,
            footer_cell: FOOTER_CELL,
            template_ids: madar_card::templates::TEMPLATE_IDS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            max_elements: v::MAX_ELEMENTS,
            max_stamps: v::MAX_STAMPS,
            text_max: v::TEXT_MAX,
            max_header: v::MAX_HEADER,
            max_secondary_and_auxiliary: v::MAX_SECONDARY_AND_AUXILIARY,
            min_contrast: v::MIN_CONTRAST,
            max_tracks: madar_card::tracks::MAX_TRACKS,
            max_track_size: madar_card::tracks::MAX_TRACK_SIZE,
            track_name_max: madar_card::tracks::NAME_MAX,
            audience_max_rules: madar_card::audience::MAX_RULES,
            audience_max_days: madar_card::audience::MAX_DAYS,
            message: LIMITS,
        })
    }

    /// Every template this shop can use, built for it.
    #[wasm_bindgen(unchecked_return_type = "CardDesign[]")]
    pub fn card_templates(
        colours: Ts<Colours>,
        ctx: Ts<Ctx>,
        tracks: Vec<Ts<Track>>,
    ) -> Result<JsValue, JsError> {
        out(&madar_card::templates::templates(
            &colours.to_rust()?,
            &ctx.to_rust()?,
            &tracks_of(tracks)?,
        ))
    }

    /// The design a shop starts from (today's look, or a row per track).
    #[wasm_bindgen(unchecked_return_type = "CardDesign")]
    pub fn card_default(
        colours: Ts<Colours>,
        ctx: Ts<Ctx>,
        tracks: Vec<Ts<Track>>,
    ) -> Result<JsValue, JsError> {
        out(&madar_card::templates::default_design(
            &colours.to_rust()?,
            &ctx.to_rust()?,
            &tracks_of(tracks)?,
        ))
    }

    /// The server's checks on a design, for this shop.
    #[wasm_bindgen(unchecked_return_type = "Problem[]")]
    pub fn card_validate(design: Ts<CardDesign>, ctx: Ts<Ctx>) -> Result<JsValue, JsError> {
        out(&madar_card::validate(&design.to_rust()?, &ctx.to_rust()?))
    }

    /// The first free spot of `w`x`h` cells, or `null`.
    #[wasm_bindgen(unchecked_return_type = "Cell | null")]
    pub fn card_first_free(design: Ts<CardDesign>, w: f64, h: f64) -> Result<JsValue, JsError> {
        out(&madar_card::validate::first_free(
            &design.to_rust()?,
            int(w)?,
            int(h)?,
        ))
    }

    /// Where each element lands on a wallet's image.
    #[wasm_bindgen(unchecked_return_type = "Placement[]")]
    pub fn card_layout(
        design: Ts<CardDesign>,
        store: Ts<Store>,
        scale: f64,
        arabic: bool,
        primary_field: bool,
    ) -> Result<JsValue, JsError> {
        let canvas = Canvas::of(store.to_rust()?, int(scale)?);
        out(&madar_card::render::layout(
            &design.to_rust()?,
            canvas,
            arabic,
            primary_field,
        ))
    }

    /// The strip (Apple, at `scale`) or the hero (Google) as SVG.
    #[wasm_bindgen]
    pub fn card_svg(
        design: Ts<CardDesign>,
        state: Ts<CardState>,
        store: Ts<Store>,
        scale: f64,
        opts: Ts<SvgOpts>,
    ) -> Result<String, JsError> {
        let canvas = Canvas::of(store.to_rust()?, int(scale)?);
        Ok(madar_card::render::svg(
            &design.to_rust()?,
            &state.to_rust()?,
            canvas,
            &opts.to_rust()?,
        ))
    }

    /// The problems with a set of tracks.
    #[wasm_bindgen(unchecked_return_type = "Problem[]")]
    pub fn tracks_validate(tracks: Vec<Ts<Track>>, mode: Ts<Mode>) -> Result<JsValue, JsError> {
        out(&madar_card::tracks::validate_tracks(
            &tracks_of(tracks)?,
            mode.to_rust()?,
        ))
    }

    #[derive(Serialize, tsify::Tsify)]
    pub struct TrackCard {
        pub rewards_ready: i32,
        pub progress_to_next: i32,
        pub points_to_next_reward: i32,
        pub can_redeem: bool,
    }

    /// One track's card for its balance.
    #[wasm_bindgen(unchecked_return_type = "TrackCard")]
    pub fn track_card(balance: f64, size: f64) -> Result<JsValue, JsError> {
        let c = madar_card::tracks::track_card(int(balance)?, int(size)?);
        out(&TrackCard {
            rewards_ready: c.rewards_ready,
            progress_to_next: c.progress_to_next,
            points_to_next_reward: c.points_to_next_reward,
            can_redeem: c.can_redeem,
        })
    }

    #[derive(Serialize, tsify::Tsify)]
    pub struct Stepper {
        pub progress: String,
        pub earned: Option<String>,
    }

    /// A track's figures as the wallet fields print them.
    #[wasm_bindgen(unchecked_return_type = "Stepper")]
    pub fn card_stepper(balance: f64, size: f64) -> Result<JsValue, JsError> {
        let (b, s) = (int(balance)?, int(size)?);
        out(&Stepper {
            progress: madar_card::text::progress_line(b, s),
            earned: madar_card::text::earned_line(b, s),
        })
    }

    /// The problems with an audience for a shop of `track_count` tracks.
    #[wasm_bindgen(unchecked_return_type = "Problem[]")]
    pub fn audience_validate(audience: Ts<Audience>, track_count: f64) -> Result<JsValue, JsError> {
        out(&validate_audience(&audience.to_rust()?, int(track_count)?))
    }

    /// The problems with a message.
    #[wasm_bindgen(unchecked_return_type = "Problem[]")]
    pub fn message_validate(message: Ts<Message>) -> Result<JsValue, JsError> {
        out(&validate_message(&message.to_rust()?))
    }

    /// The line a member reading Arabic (or not) gets.
    #[wasm_bindgen]
    pub fn message_line_for(message: Ts<Message>, arabic: bool) -> Result<String, JsError> {
        Ok(message.to_rust()?.line_for(arabic).to_string())
    }
}
