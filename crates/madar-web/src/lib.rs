//! # madar-web
//!
//! The madar-shared rules in the browser: `wasm-bindgen` bindings over the
//! crates, so the web dashboard and the customer pages call the same Rust as
//! the backend and the POS core. Every export is one call into a crate; this
//! crate holds no rule of its own (SHARED_RULES_PLAN.md, Step 3).
//!
//! Two builds (scripts/build-wasm.sh):
//! - `public`: the customer pages (ordering, menu, loyalty).
//! - `full`: the web dashboard; everything in `public` and more.
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
//! - Time zones are IANA names; the build bundles Cairo, MENA and the US
//!   (scripts/tz-filter.txt). Any other zone throws.

#![cfg(feature = "public")]

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
fn either<T: Serialize, E: Serialize>(r: Result<T, E>) -> Result<JsValue, JsError> {
    match r {
        Ok(v) => out(&v),
        Err(e) => out(&e),
    }
}

/// A JS number that must be a whole number (piastres, counts): anything else
/// throws instead of being truncated.
fn int<T: TryFrom<i64>>(x: f64) -> Result<T, JsError> {
    const MAX_SAFE: f64 = 9_007_199_254_740_991.0;
    if x.fract() == 0.0 && x.abs() <= MAX_SAFE {
        if let Ok(v) = T::try_from(x as i64) {
            return Ok(v);
        }
    }
    Err(JsError::new("expected a whole number in range"))
}

/// Each element of a JS array of typed objects.
fn each<T>(v: Vec<Ts<T>>) -> Result<Vec<T>, JsError>
where
    T: tsify::Tsify + serde::de::DeserializeOwned,
    T::JsType: Clone,
{
    v.iter().map(|t| Ok(t.to_rust()?)).collect()
}

mod public {
    use madar_catalog::combo::{ComboView, PickIn};
    use madar_catalog::{CatalogView, ItemView, Selection};

    use super::*;

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

    /// The canonical phone (E.164 digits, no `+`), or `null` when `raw` is
    /// not a phone number.
    #[wasm_bindgen(unchecked_return_type = "string | null")]
    pub fn phone_canonical(raw: &str) -> Result<JsValue, JsError> {
        out(&madar_ids::phone::canonical(raw))
    }
}

#[cfg(feature = "full")]
mod full {
    use chrono::{DateTime, Datelike, NaiveDate, Utc};
    use chrono_tz::Tz;
    use madar_catalog::combo::SlotView;
    use madar_inventory::replenish::Input as ReplenishInput;
    use madar_inventory::transfer::{Action, Side, TransferStatus};
    use madar_money::cost::CostLine;
    use serde::Deserialize;
    use tsify::Tsify;

    use super::*;

    fn zone(name: &str) -> Result<Tz, JsError> {
        name.parse()
            .map_err(|_| JsError::new("unknown time zone, or one this build leaves out"))
    }

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

    fn ymd(d: NaiveDate) -> String {
        format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day())
    }

    fn instant(ms: f64) -> Result<DateTime<Utc>, JsError> {
        DateTime::from_timestamp_millis(int(ms)?)
            .ok_or_else(|| JsError::new("epoch milliseconds out of range"))
    }

    // ── time (madar-time) ────────────────────────────────────────────────

    /// The branch-local business date of an instant (epoch ms).
    #[wasm_bindgen]
    pub fn business_date(tz: &str, at_ms: f64) -> Result<String, JsError> {
        Ok(ymd(madar_time::business_date_of(
            zone(tz)?,
            instant(at_ms)?,
        )))
    }

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

    // ── Dawam pay (madar-dawam) ──────────────────────────────────────────

    /// The pay window `[start, end]` (inclusive dates) holding `day`;
    /// `start_day` is clamped to 1–28.
    #[wasm_bindgen(unchecked_return_type = "[string, string]")]
    pub fn pay_period(day: &str, start_day: f64) -> Result<JsValue, JsError> {
        let (a, b) = madar_dawam::pay::period_window(date(day)?, int(start_day)?);
        out(&(ymd(a), ymd(b)))
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
