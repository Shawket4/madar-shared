// The madar-shared vector files, run through the WebAssembly build: the same
// cases the crates pass (`cargo test`), now through the browser packages'
// boundary (serde-wasm-bindgen, epoch-ms instants, refusals returned).
//
// Build first (scripts/build-wasm.sh), then: node --test node/
// Each package runs every file its exports cover; `full` covers all seventeen
// files the web uses, `public` the customer pages' four.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { after, describe, test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const vectors = (path) => JSON.parse(readFileSync(join(root, "crates", path), "utf8"));

async function load(pkg) {
  const dir = join(root, "dist", pkg);
  const m = await import(join(dir, "madar_web.js"));
  m.initSync({ module: readFileSync(join(dir, "madar_web_bg.wasm")) });
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

  const rules = [...bill.bills, ...bill.open_bills].filter((b) => b.discount.kind !== "stated");
  cases(pkg, "bill_vectors", "bill_discount", rules,
    (b) => w.bill_discount(b.subtotal, b.discount.kind, b.discount.value),
    (b) => b.discount_amount);

  cases(pkg, "phone_vectors", "phone_canonical",
    [...phone.valid.map(([raw, want]) => ({ raw, want })), ...phone.invalid.map((raw) => ({ raw, want: null }))],
    (c) => w.phone_canonical(c.raw), (c) => c.want);

  test("input that is not what the type says throws", () => {
    assert.throws(() => w.unit_price({ sizes: "none" }, null), Error);
    assert.throws(() => w.bill_discount(100.5, "fixed", "10"), /whole number/);
    assert.throws(() => w.bill_discount(100, "fixed", "ten"), /decimal/);
  });
}

function fullFiles(pkg, w) {
  // The build bundles Cairo, MENA and the US (scripts/tz-filter.txt): a case in
  // another zone (Europe/London) must throw instead of answering.
  const bundled = new RegExp(`^${readFileSync(join(root, "scripts/tz-filter.txt"), "utf8").trim()}$`);
  const [businessDates, businessElsewhere] = split(vectors("madar-time/vectors/business_date_vectors.json"), (c) => bundled.test(c.tz));
  cases(pkg, "business_date_vectors", "business_date", businessDates,
    (c) => w.business_date(c.tz, Date.parse(c.at)), (c) => c.business_date);
  const [dayBounds, boundsElsewhere] = split(vectors("madar-time/vectors/day_bounds_vectors.json"), (c) => bundled.test(c.tz));
  cases(pkg, "day_bounds_vectors", "day_bounds", dayBounds,
    (c) => w.day_bounds(c.tz, c.date), (c) => [Date.parse(c.start), Date.parse(c.end)]);
  test(`a zone the build leaves out throws (${businessElsewhere.length + boundsElsewhere.length} cases)`, () => {
    assert.ok(businessElsewhere.length + boundsElsewhere.length > 0);
    for (const c of businessElsewhere) assert.throws(() => w.business_date(c.tz, Date.parse(c.at)), /time zone/);
    for (const c of boundsElsewhere) assert.throws(() => w.day_bounds(c.tz, c.date), /time zone/);
    skipped.push(`${businessElsewhere.length + boundsElsewhere.length} time cases outside the bundled zones (${[...new Set([...businessElsewhere, ...boundsElsewhere].map((c) => c.tz))]})`);
  });
  cases(pkg, "week_vectors", "week_start", vectors("madar-time/vectors/week_vectors.json"),
    (c) => w.week_start(c.date), (c) => c.week_start);

  cases(pkg, "dawam_vectors", "pay_period", vectors("madar-dawam/vectors/dawam_vectors.json").periods,
    (c) => w.pay_period(c.day, c.start_day), (c) => [c.start, c.end]);

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
  if (hugeBands.length) skipped.push(`${hugeBands.length} food_cost_band case(s) beyond JS's safe integers (${hugeBands.map((c) => c.name)})`);
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
  });
}

for (const pkg of ["public", "full"]) {
  const w = await load(pkg);
  describe(pkg, () => {
    publicFiles(pkg, w);
    if (pkg === "full") fullFiles(pkg, w);
  });
}

after(() => {
  const total = (pkg) => runs.filter((r) => r.pkg === pkg).reduce((n, r) => n + r.cases, 0);
  const failed = runs.reduce((n, r) => n + r.failed, 0);
  console.log(`vector cases through wasm: public ${total("public")}, full ${total("full")}, failed ${failed}`);
  for (const s of skipped) console.log(`not run (full): ${s}`);
});
