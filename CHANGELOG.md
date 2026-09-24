# Changelog

Every tag both consumers pin. A release that changes a result on either side
says so here; everything else is a move.

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
