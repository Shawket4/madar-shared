# Changelog

Every tag both consumers pin. A release that changes a result on either side
says so here; everything else is a move.

## v0.4.0 — every deferred item: the server calls the crates (2026-09-25)

New crate `madar-loyalty`; new pieces in `madar-money`, `madar-catalog`,
`madar-dawam`. The backend's pinned copies are gone: it calls the crates.

- **madar-money `bill`**: `price_bill_on` (the assembly over a subtotal a till
  STATED, the server's reading when it priced no reward or staff comp
  itself), `price_subtotal` (the discount and the engine over a subtotal
  already held), `rule_of` (a stored discount rule: a percentage is a
  fraction, fixed is minor units). `price_bill` and `price_open_bill` call the
  same steps; nothing they return changed.
- **madar-loyalty** (new): `plan(lines, programme, asks, mode)` — which of the
  asked rewards a sale takes. `Mode::Server` is the backend's `plan_strict`
  (the first refusal, in its order); `Mode::Till` is the till's reward board
  (trim, name the first trim). `replay_lines` is the backend's replay of a
  refused plan. Pinned by `loyalty_plan_vectors.json` (27 cases, each with
  the till's plan, the server's verdict and the replay's lines) and two
  invariants: a trimmed plan is one the server takes; a plan the server
  takes is not trimmed.
- **madar-catalog `staff`**: `comp_input(item, line)` — the staff comp's input
  (catalogue sizes for a sized line, with the branch's price; the attached
  required groups that are not swap groups, options by the allow-list) — the
  backend's SQL builder, moved. `ItemView.groups` (`GroupView`) carries the
  item's attached groups; skipped when empty, so every existing view
  serialises as before. Pinned by `staff_input_vectors.json` (75 staff lines,
  written by the backend from its SQL builder while both ran).
- **madar-dawam**: the offline stamp derives its OpenAPI schema behind the
  new `utoipa` feature (the backend's schema `OfflineStamp`, now with a
  description).
- **madar-dawam** (Dawam, DW1/DW3/DW6): `shift::instants` / `wall_instant` —
  a shift's wall-clock times placed as the server's roster SQL places them
  (`(date + time) AT TIME ZONE zone`: a time that happens twice is the later
  one, a time in the spring gap is read at the offset before the jump, the
  end on the next date when it does not come after the start), pinned by
  `shift_vectors.json` (104 shifts on Cairo, Beirut, Berlin, Riyadh and UTC
  DST days, each checked against Postgres); `pay::percent_of_salary` — the
  server's `salary × percent ÷ 100`, half away from zero, pinned by
  `percent_vectors.json` (108 cases, each checked against Postgres's
  `round(numeric)`); `presence::LOW_BATTERY` (15).
- **Docs**: madar-till's fold and carryover picker are what the backend runs.

**The backend (a move; its results did not change):** the order path's bill
(create order, the ticket settle, the table bill's preview, the delivery
intake) is `price_bill_on` / `price_subtotal` / `rule_of`, a line's extras
`line::extras_per_unit`; the till's drawer, Z report and close lines are
`madar_till::report` over the rows it loads, the carryover
`carryover::last_close_declared` over its two candidates; a sale's rewards are
`madar_loyalty::plan` (strict); a staff line's comp input is
`madar_catalog::staff::comp_input` over the order's loaded catalogue, whose
feed `pricing` now carries the item's groups; `OfflineStamp` is madar-dawam's.
The Dawam salary percentages (two Rust copies), the roster week and the
low-battery line are madar-dawam's / madar-time's; the SQL twins
(`dawam_advance_cap`, an adjustment's value, the roster's shift placement) are
pinned to the crate by `tests/dawam_shared_rules_tests.rs`.
Pinned by the till vectors (the SQL's own answers), the bill vectors rung
through `POST /orders`, new order-path tests (service charge, reward, staff
drink, discount, split tender and change), the staff input vectors (the SQL
builder's answers) and the backend's whole suite.

**What changes on the server (DW1, deliberate):** the roster suggester
placed its slots with chrono's `.earliest()` while the roster itself (SQL)
placed the same shift as Postgres does; it runs `shift::instants` now. On
Cairo's spring-forward day (2026-04-24) a shift starting 00:30 is now
suggested at 2026-04-23T22:30Z (it had no slot at all); on the fall-back
night (2026-10-29) a shift starting 23:30 sits at 21:30Z (was 20:30Z, an hour
early) — where the roster puts it.

**What changes on a till built against v0.4.0** (the server's inputs, where
the till read its mirror its own way; a tablet in the field keeps its old
reading until it updates):

- A staff drink's comp is judged on the choice groups the server ships in the
  item's `pricing` (from a v0.4.0 server): a required group's DEFAULT option
  sets the allowance on every menu (the till's legacy path took the cheapest,
  so a required group with A 5.00 default picked and B 3.00 comped 3.00 and
  charged 2.00; now 5.00 and 0.00, as the server books it); a non-swap group
  holding a swap option, and an item-private optional in a group, are judged
  as the server judges them; sizes carry the branch's price beside the
  catalogue's. From an older server the till reads its mirror as before.
- The reward board, the table bill's preview and the rest are the same
  rules run from one copy: no figure changes.

**What changes in the staff app's core built against v0.4.0 (DW3,
deliberate):** a percent-of-base bonus or deduction the server sent no amount
for is priced with `pay::percent_of_salary` (the POS staff screen's
`adjustment_view`, the last f64 copy): 33.3 % of a 1500 salary is 500 (was
499), a negative percentage 0. The Dawam app itself already showed the
server's `value_piastres` only. Its shift placement (`dawam.rs`) now calls
`shift::instants` (the same Postgres rule it had copied: no figure changes),
its low-battery line is `presence::low_battery`, and its manager tabs are
decided by `madar_authz::Cap` values (DW6), not strings.

## v0.3.0 — C, catalogue pricing (2026-09-24)

New crate `madar-catalog`: how a sale line is priced from the catalogue.

- The rule is the SERVER's, moved from MadarRust `orders/handlers.rs`
  (`catalog_unit_price`) and `orders/component_resolve.rs` (`swap_target`,
  `collapse_families`, `merge_sized_option_lines`, the pricing half of the
  resolver) with its two swap-family tests. The server's results did not
  change: every case of `catalog_vectors.json` (54 lines over 7 items and
  20 options) was checked against the order path's answers taken before the
  move, stock deductions included.
- `view`: `CatalogView` — an item (sizes with their branch prices, the
  branch's item price, the recipe's lines per size and its default size, the
  swap bases' candidates, the optional fields) and the options (price, type,
  group, effect, swap category, the ingredient it replaces, its lines per
  size). The backend builds it from SQL; `feed` rebuilds it from the
  `pricing` the backend now ships on every `/menu-items?full=true` row and
  every add-on row.
- `price`: `price_line`, `price_options` (a bundle component), `unit_price`,
  `option_charge` / `is_recipe_choice` (what a sheet shows and preselects),
  `swap_target`, `collapse_families`, `merge_sized_lines`.

**What changes on a till built against v0.3.0** (the server's answer, where
the till's own swap families and size rule disagreed; a tablet in the field
keeps its old pricing until it updates, and the server flags — never
refuses — a sale priced the old way):

- An explicit `swaps` group (a tea group over a black-tea recipe): green is
  charged the 400 difference, not 700; the recipe's black is free, not 300.
- An option sharing the recipe's ingredient with the default (a "barista
  whole" at 300 beside whole milk at 0) is the recipe's own choice: 0, not 300.
- A swap is charged over the recipe's option in the chosen option's own group
  first, the active ones before an inactive one (not over
  `default_milk_addon_id`).
- A swap group picked twice keeps the LAST pick once, where it was picked
  (a till sent vanilla ×2 + caramel as additive: 650; now caramel ×1: 50).
- A line with no size is the branch's item price, else the lowest active
  size (was the lowest size); an inactive size or a size only the branch
  prices takes the server's fallback.
- An optional field offered on another size only is skipped.
- The recipe preview swaps what the rule swaps.

## v0.2.0 — B2, the second extraction (2026-09-24)

New crates `madar-till`, `madar-units`, `madar-ids`, `madar-dawam`; new
modules in `madar-money`, `madar-sync`, `madar-authz`.

- **madar-money v2**
  - `line`: a line's total, the bundle component surcharge (per component
    unit, M3 — the server's rule).
  - `bill`: `net_line` (staff comp, then the reward), `price_bill`,
    `refusal`, `price_open_bill` (a table bill's preview), `change_due`,
    `negative_leg` / `legs_cover` / `recorded_tender`.
  - `discount`: `ask_from` / `percent_bps_of` (moved from the backend),
    `bps_of_rate`, `figures`, `request_for` — the server's rounding
    (Decimal round-half-even).
  - `waste`: `value_of`, and the server's input rules (above 0, at most
    1 000 000, nothing once converted, whole pcs for a menu item).
- **madar-till**: the drawer / Z report fold, the carryover picker,
  close reconciliation. `till_report_vectors.json` moved in byte-identical;
  `till_edge_vectors.json` and `carryover_vectors.json` are new.
- **madar-units**: units, families, conversion, the density bridge.
- **madar-ids**: the canonical phone (`phone_vectors.json` moved in
  byte-identical), order-ref formats, the member card token.
- **madar-sync v2**: the `/sync/replay` envelopes (`replay::ReplayOp`, 29
  ops, the permanent aliases) and `replay_current.json`, the current
  release's envelopes.
- **madar-dawam**: the geofence, the pay period, the offline stamp's type
  and the anchor's format.
- **madar-authz**: `acts::void_facts` / `VoidAsk`; `pin::verify_offline_pin`
  and `TEST_PHC`. The registry and authz-gen's outputs are unchanged.

**What changes on a till built against v0.2.0** (the server's behaviour was
taken in every case; a tablet in the field keeps its embedded copy until it
updates):

- A discount's basis points and a fixed preset's value round half-to-even,
  as the server judges them (0.00025 → 2 bps, not 3; a preset of 12.5 → 12),
  and a settle's legacy 0-100 percentage is read as the server reads it.
- A table bill whose subtotal went below zero previews at 0.
- The drawer's close lines: a payment method named with a tab is a method; a
  cash method with no creation time is picked last as the fallback name; the
  lines shown for a queued close are the server's planner's (trimmed notes, a
  named-but-unused method gets its line).
- A waste above 1 000 000, or one that converts to nothing in its
  ingredient's unit, is refused before it is queued.
- A void's age is whole minutes of the exact difference (it used whole
  seconds first).
- The geofence distance uses the server's `atan2` form (sub-millimetre).
- A malformed `X-Dawam-Time` anchor no longer yields a server time.

## v0.1.0 — B1, the first extraction (2026-09-23)

`madar-authz` (with its spec and authz-gen), `madar-money` v1 (tax engine and
channel rule, refund split, staff pool / comp rule, reward cover, metrics),
`madar-time`, `madar-sync` (type lists, ledger classification, R-checksum,
kitchen ids). A pure move: no result changed.
