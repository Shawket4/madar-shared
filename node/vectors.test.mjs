// The madar-shared vector files, run through the WebAssembly build: the same
// cases the crates pass (`cargo test`), now through the browser packages'
// boundary (serde-wasm-bindgen, epoch-ms instants, refusals returned).
//
// Build first (scripts/build-wasm.sh), then: node --test node/
// Each package runs every file its exports cover; `full` covers all
// twenty-five files the web dashboard boots with, `public` the customer pages'
// ten, `menu` the menu import's one.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { after, describe, test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const vectors = (path) => JSON.parse(readFileSync(join(root, "crates", path), "utf8"));

async function load(pkg) {
  const dir = join(root, "dist", pkg);
  const name = pkg === "menu" ? "madar_menu" : "madar_web";
  const m = await import(join(dir, `${name}.js`));
  m.initSync({ module: readFileSync(join(dir, `${name}_bg.wasm`)) });
  return m;
}

const runs = [];
const skipped = [];
const split = (list, keep) => [list.filter(keep), list.filter((c) => !keep(c))];

/** Every case through `got`, compared with `want`; all mismatches reported at once. */
function cases(pkg, file, part, list, got, want) {
  test(`${file} ${part} (${list.length} cases)`, () => {
    const bad = [];
    for (const c of list) {
      try {
        assert.deepStrictEqual(got(c), want(c));
      } catch (e) {
        bad.push(`${c.name ?? JSON.stringify(c).slice(0, 80)}: ${e.message.split("\n").slice(0, 6).join(" ")}`);
      }
    }
    runs.push({ pkg, file, part, cases: list.length, failed: bad.length });
    assert.deepStrictEqual(bad, []);
  });
}

const catalog = vectors("madar-catalog/vectors/catalog_vectors.json");
const combo = vectors("madar-catalog/vectors/combo_vectors.json");
const bill = vectors("madar-money/vectors/bill_vectors.json");
const phone = vectors("madar-ids/vectors/phone_vectors.json");
const scan = vectors("madar-ids/vectors/scan_vectors.json");
const businessDates = vectors("madar-time/vectors/business_date_vectors.json");
const cart = vectors("madar-money/vectors/cart_vectors.json");
const comboMenu = vectors("madar-catalog/vectors/combo_menu_vectors.json");
const loyaltyCard = vectors("madar-loyalty/vectors/loyalty_card_vectors.json");
const menuOrder = vectors("madar-catalog/vectors/menu_order_vectors.json");
const plan = vectors("madar-plan/vectors/plan_vectors.json");
// `public` bundles scripts/tz-filter.txt's zones (Cairo, MENA, the US, UTC); a
// case in another zone (Europe/London) must throw there. `full` bundles every zone.
const bundled = new RegExp(`^${readFileSync(join(root, "scripts/tz-filter.txt"), "utf8").trim()}$`);

/** The files the customer pages compute; both packages run them. */
function publicFiles(pkg, w) {
  const view = (key) => ({ item: catalog.items.find((i) => i.key === key).view, options: catalog.options });
  cases(pkg, "catalog_vectors", "price_line", catalog.cases,
    (c) => w.price_line(view(c.item), c.selection),
    (c) => c.expected.line ?? c.expected.error);
  const lines = catalog.cases.filter((c) => c.expected.line);
  cases(pkg, "catalog_vectors", "unit_price", lines,
    (c) => w.unit_price(view(c.item).item, c.selection.size_label),
    (c) => c.expected.line.unit_price);
  cases(pkg, "catalog_vectors", "price_options", lines,
    (c) => w.price_options(view(c.item), c.selection),
    (c) => { const { unit_price, ...rest } = c.expected.line; return rest; });
  // The figure beside one option: the charge of a line picking it alone.
  const single = lines.filter((c) => c.selection.options.length === 1 && (c.selection.options[0].quantity ?? 1) === 1);
  cases(pkg, "catalog_vectors", "option_charge", single,
    (c) => w.option_charge(view(c.item), c.selection.size_label, c.selection.options[0].id),
    (c) => c.expected.line.options[0].unit_price);

  cases(pkg, "combo_vectors", "combo_quote", combo.cases,
    (c) => w.combo_quote(combo.combos[c.combo],
      c.picks.map((p) => ({ ...p, view: combo.items[p.item], item: undefined })), c.n),
    (c) => c.expected.quote ?? c.expected.refusal);

  // The storefront cart: an item line, a combo line, the subtotal.
  cases(pkg, "cart_vectors", "cart_line_unit", cart.lines, (c) => w.cart_line_unit(c.line), (c) => c.unit);
  cases(pkg, "cart_vectors", "cart_line_total", cart.lines, (c) => w.cart_line_total(c.line), (c) => c.total);
  cases(pkg, "cart_vectors", "cart_subtotal", cart.carts, (c) => w.cart_subtotal(c.lines), (c) => c.subtotal);
  cases(pkg, "cart_vectors", "combo_extras", cart.combo_extras, (c) => w.combo_extras(c.picks), (c) => c.expected);
  // A quoted combo's line, estimated from its parts, is what the quote charges.
  const quoted = combo.cases.filter((c) => c.expected.quote);
  const comboLine = (q) => ({ combo: { quantity: q.quantity, price: q.price,
    picks: q.parts.map((p) => ({ quantity: p.pick_quantity, surcharge_unit: p.surcharge_unit, extras_unit: p.extras_unit })) } });
  cases(pkg, "combo_vectors", "cart_line_total (a quoted combo)", quoted,
    (c) => [w.cart_line_unit(comboLine(c.expected.quote)), w.cart_line_total(comboLine(c.expected.quote))],
    (c) => [c.expected.quote.unit_total, c.expected.quote.quantity * c.expected.quote.unit_total]);
  cases(pkg, "combo_menu_vectors", "combo_choice_extra", comboMenu,
    (c) => w.combo_choice_extra(c.choice, c.size_label), (c) => c.expected);

  cases(pkg, "menu_order_vectors", "category_order", menuOrder.categories,
    (c) => w.category_order(c.categories, c.locale).map((i) => c.categories[i].id), (c) => c.expected);

  cases(pkg, "loyalty_card_vectors", "loyalty_card", loyaltyCard.cards,
    (c) => w.loyalty_card(c.balance, c.next_reward_cost), (c) => c.expected);
  cases(pkg, "loyalty_card_vectors", "loyalty_stamps", loyaltyCard.stamps,
    (c) => w.loyalty_stamps(c.earned, c.cost), (c) => c.expected);

  // The plan's entitlements; an unknown plan is Paid.
  cases(pkg, "plan_vectors", "entitlements", plan.plans, (c) => w.entitlements(c.plan), (c) => c.expected);
  const paid = plan.plans.find((c) => c.plan === "paid").expected;
  const free = plan.plans.find((c) => c.plan === "free").expected;
  cases(pkg, "plan_vectors", "entitlements (parse)", plan.parse.map(([raw, want]) => ({ raw, want })),
    (c) => w.entitlements(c.raw), (c) => (c.want === "free" ? free : paid));

  const rules = [...bill.bills, ...bill.open_bills].filter((b) => b.discount.kind !== "stated");
  cases(pkg, "bill_vectors", "bill_discount", rules,
    (b) => w.bill_discount(b.subtotal, b.discount.kind, b.discount.value),
    (b) => b.discount_amount);

  cases(pkg, "phone_vectors", "phone_canonical",
    [...phone.valid.map(([raw, want]) => ({ raw, want })), ...phone.invalid.map((raw) => ({ raw, want: null }))],
    (c) => w.phone_canonical(c.raw), (c) => c.want);

  cases(pkg, "scan_vectors", "scan_classify", scan.map(([raw, kind, value]) => ({ raw, kind, value })),
    (c) => w.scan_classify(c.raw), (c) => ({ kind: c.kind, value: c.value }));

  const [dates, elsewhere] = pkg === "public" ? split(businessDates, (c) => bundled.test(c.tz)) : [businessDates, []];
  cases(pkg, "business_date_vectors", "business_date", dates,
    (c) => w.business_date(c.tz, Date.parse(c.at)), (c) => c.business_date);
  if (pkg === "public") {
    test(`a zone the build leaves out throws (${elsewhere.length} cases)`, () => {
      assert.ok(elsewhere.length > 0);
      for (const c of elsewhere) assert.throws(() => w.business_date(c.tz, Date.parse(c.at)), /time zone/);
      skipped.push(`public: ${elsewhere.length} business_date cases outside the bundled zones (${[...new Set(elsewhere.map((c) => c.tz))]})`);
    });
  }

  test("input that is not what the type says throws", () => {
    assert.throws(() => w.unit_price({ sizes: "none" }, null), Error);
    assert.throws(() => w.bill_discount(100.5, "fixed", "10"), /whole number/);
    assert.throws(() => w.bill_discount(100, "fixed", "ten"), /decimal/);
    assert.throws(() => w.cart_line_total({ item: { quantity: 1.5, unit_price: 100, addons: [], optionals: [] } }), Error);
    assert.throws(() => w.cart_subtotal([{ kind: "item", quantity: 1, unit_price: 100, addons: [], optionals: [] }]), Error);
    assert.throws(() => w.loyalty_card(2.5, 5), /whole number/);
    assert.throws(() => w.category_order([{ id: "a", name: "A", display_order: 1.5 }], "en"), Error);
    assert.throws(() => w.category_order([{ name: "no id" }], "en"), Error);
  });
}

function fullFiles(pkg, w) {
  // Every zone, the ones outside public's filter (Europe/London) included.
  const dayBounds = vectors("madar-time/vectors/day_bounds_vectors.json");
  assert.ok(dayBounds.some((c) => !bundled.test(c.tz)) && businessDates.some((c) => !bundled.test(c.tz)));
  cases(pkg, "day_bounds_vectors", "day_bounds", dayBounds,
    (c) => w.day_bounds(c.tz, c.date), (c) => [Date.parse(c.start), Date.parse(c.end)]);
  cases(pkg, "week_vectors", "week_start", vectors("madar-time/vectors/week_vectors.json"),
    (c) => w.week_start(c.date), (c) => c.week_start);

  cases(pkg, "dawam_vectors", "pay_period", vectors("madar-dawam/vectors/dawam_vectors.json").periods,
    (c) => w.pay_period(c.day, c.start_day), (c) => [c.start, c.end]);

  const wall = vectors("madar-time/vectors/wallclock_vectors.json");
  cases(pkg, "wallclock_vectors", "local_parts", wall.parts,
    (c) => w.local_parts(c.tz, Date.parse(c.at)), (c) => ({ date: c.date, hour: c.hour, minute: c.minute }));
  cases(pkg, "wallclock_vectors", "local_instant", wall.instants,
    (c) => w.local_instant(c.tz, c.date, c.hour, c.minute), (c) => Date.parse(c.at));

  // Decimals (working days, a rung's value) are JS numbers on the dashboard.
  const salary = vectors("madar-dawam/vectors/salary_vectors.json");
  cases(pkg, "salary_vectors", "rates", salary.rates,
    (c) => w.rates({ [c.typed]: c.value }, Number(c.working_days), c.day_minutes),
    (c) => ({ monthly: c.monthly, daily: c.daily, hourly: c.hourly }));
  cases(pkg, "salary_vectors", "first_pay", salary.first_pay,
    (c) => w.first_pay(c.monthly, c.hire_date, c.start_day),
    (c) => ({ from: c.from, to: c.to, days: c.days, period_days: c.period_days, piastres: c.piastres }));
  skipped.push(`full: ${salary.prorated.length} salary_vectors prorated cases (prorated_base is not exported: the web does not compute it)`);

  const ladder = vectors("madar-dawam/vectors/ladder_vectors.json");
  const rungs = ladder.ladder.map((t) => ({ ...t, value: Number(t.value) }));
  cases(pkg, "ladder_vectors", "select_late_tier", ladder.select,
    (c) => w.select_late_tier(rungs, c.late_minutes), (c) => c.tier);
  cases(pkg, "ladder_vectors", "late_deduction_piastres", ladder.deductions,
    (c) => w.late_deduction_piastres({ from_minutes: 0, to_minutes: null, kind: c.kind, value: Number(c.value) },
      c.salary, Number(c.working_days), c.day_minutes),
    (c) => c.piastres);
  cases(pkg, "ladder_vectors", "absence_deduction_piastres", ladder.absences,
    (c) => w.absence_deduction_piastres(c.salary, Number(c.working_days), Number(c.days_absent), Number(c.deduction_days)),
    (c) => c.piastres);

  const units = vectors("madar-units/vectors/unit_vectors.json");
  cases(pkg, "unit_vectors", "convert_with_density", units,
    (c) => w.convert_with_density(c.qty, c.from, c.to, c.density),
    (c) => (c.error == null ? c.result : { error: c.error }));
  cases(pkg, "unit_vectors", "convert (same_without_density)", units,
    (c) => {
      const ok = (r) => (typeof r === "number" ? r : null);
      return ok(w.convert(c.qty, c.from, c.to)) === ok(w.convert_with_density(c.qty, c.from, c.to, c.density));
    },
    (c) => c.same_without_density);
  const recipeQty = vectors("madar-units/vectors/recipe_qty_vectors.json");
  cases(pkg, "recipe_qty_vectors", "recipe_base_qty", recipeQty.recipe_base_qty,
    (c) => w.recipe_base_qty(c.qty, c.unit, c.base_unit, c.density, c.yield_pct),
    (c) => (c.error == null ? c.expected : { error: c.error }));
  cases(pkg, "recipe_qty_vectors", "usable_qty", recipeQty.usable_qty,
    (c) => w.usable_qty(c.stored, c.yield_pct), (c) => c.expected);
  cases(pkg, "recipe_qty_vectors", "round_trips", recipeQty.round_trips,
    (c) => {
      const stored = w.recipe_base_qty(c.qty, c.unit, c.base_unit, c.density, c.yield_pct);
      const usable = w.usable_qty(stored, c.yield_pct);
      return [stored, usable, usable === c.typed_in_base];
    },
    (c) => [c.stored, c.usable, c.round_trips]);
  cases(pkg, "scale_vectors", "scale_qty", vectors("madar-units/vectors/scale_vectors.json").cases,
    (c) => w.scale_qty(c.qty, c.factor), (c) => c.expected);

  const inventory = vectors("madar-inventory/vectors/inventory_vectors.json");
  cases(pkg, "inventory_vectors", "transfer_step", inventory.steps,
    (c) => w.transfer_step(c.status, c.action),
    (c) => (c.side == null ? null : { side: c.side, cap: c.cap, next: c.next }));
  cases(pkg, "inventory_vectors", "check_receive_line", inventory.receives,
    (c) => w.check_receive_line(c.qty_sent, c.qty_received, c.note), (c) => c.ok ?? c.refused);
  cases(pkg, "inventory_vectors", "replenish_suggest", inventory.replenish,
    (c) => w.replenish_suggest(c.input), (c) => c.out);

  const purchase = vectors("madar-inventory/vectors/purchase_vectors.json");
  cases(pkg, "purchase_vectors", "quantity_dec", purchase.quantity,
    (c) => w.quantity_dec(c.q), (c) => Number(c.quantity_dec));
  cases(pkg, "purchase_vectors", "quantity_milli", purchase.quantity,
    (c) => w.quantity_milli(c.q), (c) => c.milli);
  cases(pkg, "purchase_vectors", "delivery_cost", purchase.delivery_cost,
    (c) => w.delivery_cost(c.quantity_received, c.line_cost, c.unit_cost, c.ordered_line_cost, Number(c.quantity_ordered)),
    (c) => (c.error == null ? Number(c.expected) : { error: c.error }));
  cases(pkg, "purchase_vectors", "estimate_line_total", purchase.estimate_line_total,
    (c) => w.estimate_line_total(c.cost_per_stock_unit, c.qty, c.purchase_unit, c.stock_unit), (c) => c.expected);
  cases(pkg, "purchase_vectors", "unit_cost_from_total", purchase.unit_cost_from_total,
    (c) => w.unit_cost_from_total(c.line, c.qty), (c) => c.expected);
  cases(pkg, "count_vectors", "is_variance_flagged", vectors("madar-inventory/vectors/count_vectors.json").cases,
    (c) => w.is_variance_flagged(c.book, c.counted, c.pct), (c) => c.expected);

  const cost = vectors("madar-money/vectors/cost_vectors.json");
  cases(pkg, "cost_vectors", "line_cost", cost.line_cost,
    (c) => w.line_cost(c.qty, c.cost_per_unit), (c) => c.expected);
  cases(pkg, "cost_vectors", "recipe_cost", cost.recipe_cost, (c) => w.recipe_cost(c.lines), (c) => c.expected);
  cases(pkg, "cost_vectors", "margin", cost.margin, (c) => w.margin(c.price, c.cost), (c) => c.expected);
  // A piastre figure beyond 2^53 is not a JS number: the web cannot hold it.
  const [bands, hugeBands] = split(cost.food_cost_band, (c) => Number.isSafeInteger(c.cost) && Number.isSafeInteger(c.price));
  if (hugeBands.length) skipped.push(`full: ${hugeBands.length} food_cost_band case(s) beyond JS's safe integers (${hugeBands.map((c) => c.name)})`);
  cases(pkg, "cost_vectors", "food_cost_band", bands,
    (c) => w.food_cost_band(c.cost, c.price), (c) => c.expected);
  cases(pkg, "pos_metrics_vectors", "average_ticket", vectors("madar-money/vectors/pos_metrics_vectors.json").expected,
    (c) => w.average_ticket(c.net_sales, c.order_count), (c) => c.average_ticket);

  cases(pkg, "reconcile_vectors", "till_plan_lines", vectors("madar-till/vectors/reconcile_vectors.json"),
    (c) => w.till_plan_lines(c.totals, c.closing_cash_declared, c.closing_cash_system, c.cash_note, c.inputs, false),
    (c) => c.expected.Ok ?? c.expected.Err);

  // No vector file of their own: each answers as the crate's rule does on one case.
  test("the remaining exports answer", () => {
    assert.deepStrictEqual(w.unit_spec("KG"), ["mass", 1000]);
    assert.equal(w.unit_spec("cups"), null);
    assert.deepStrictEqual(w.units_of("ml"), ["ml", "l"]);
    const lunch = combo.combos.lunch;
    const pick = combo.cases[0].picks[0];
    const admits = w.combo_choice_for(lunch.slots.find((s) => s.id === pick.slot_id), combo.items[pick.item].item.id, null);
    assert.equal(admits.menu_item_id, combo.items[pick.item].item.id);
    assert.equal(w.combo_choice_for(lunch.slots[0], "no-such-item", null), null);
    assert.throws(() => w.business_date("Mars/Olympus", 0), /time zone/);
    assert.throws(() => w.week_start("2026-02-30"), /YYYY-MM-DD/);
    assert.throws(() => w.business_date("Africa/Cairo", 1.5), /whole number/);
    assert.throws(() => w.delivery_cost(1, 10.5, null, 0, 1), /whole number/);
    assert.throws(() => w.rates({ monthly: 100 }, NaN, 480), /finite/);
    assert.throws(() => w.late_deduction_piastres({ from_minutes: 0, to_minutes: null, kind: "hours", value: 1 }, 100, 26, 480), Error);
    assert.throws(() => w.local_instant("Africa/Cairo", "2026-01-01", -1, 0), /whole number/);
  });
}

/** The menu import (madar-menu): the Foodics reader, the server's checks, the
 * replace sentence; only its own package carries them. */
function menuFiles(pkg, w) {
  const menuImport = vectors("madar-menu/vectors/menu_import_vectors.json");
  cases(pkg, "menu_import_vectors", "foodics_parse", menuImport.foodics,
    (c) => w.foodics_parse(c.files), (c) => c.expected);
  cases(pkg, "menu_import_vectors", "bulk_validate", menuImport.validate,
    (c) => w.bulk_validate(c.menu), (c) => c.expected);
  cases(pkg, "menu_import_vectors", "replace_sentence", menuImport.sentences,
    (c) => w.replace_sentence(c.org_name, c.lang), (c) => c.expected);
  cases(pkg, "menu_import_vectors", "confirms_replace", menuImport.confirm,
    (c) => w.confirms_replace(c.org_name, c.typed), (c) => c.expected);
}

for (const pkg of ["public", "full"]) {
  const w = await load(pkg);
  describe(pkg, () => {
    publicFiles(pkg, w);
    if (pkg === "full") fullFiles(pkg, w);
    // Neither boot package carries the menu import's reader.
    test(`${pkg} leaves out the menu import`, () => {
      assert.equal(w.foodics_parse, undefined);
      assert.equal(w.bulk_validate, undefined);
      assert.equal(w.confirms_replace, undefined);
    });
  });
}

{
  const w = await load("menu");
  describe("menu", () => {
    menuFiles("menu", w);
    test("menu carries only the menu import", () => {
      assert.equal(w.price_line, undefined);
      assert.equal(w.business_date, undefined);
    });
  });
}

after(() => {
  const total = (pkg) => runs.filter((r) => r.pkg === pkg).reduce((n, r) => n + r.cases, 0);
  const failed = runs.reduce((n, r) => n + r.failed, 0);
  console.log(`vector cases through wasm: public ${total("public")}, full ${total("full")}, menu ${total("menu")}, failed ${failed}`);
  for (const s of skipped) console.log(`not run: ${s}`);
});
