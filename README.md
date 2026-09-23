# madar-shared

Pure, I/O-free Rust logic that the Madar backend
([MadarRust](https://github.com/Shawket4/MadarRust)) and the Rust cores
([madar/rust-core](https://github.com/Shawket4/madar): madar-core behind the
POS, the staff app and the dashboard app) must both run, and must never
disagree on.

Before this repo existed each rule lived twice, kept in step by a vendored copy
or by test-vector JSON files copied between the repos by hand. Now there is one
copy, its vectors are its own tests, and both consumers pin the same tag.

## What lives here

| Crate | What it holds |
|---|---|
| `madar-authz` | The permission decision library: the capability registry (generated from `authz/spec/capabilities.toml`), `resolve`, `decide`, the anti-escalation guard, the signed-snapshot binding. |
| `madar-money` | Money rules already identical on both sides: the tax engine and the sale-channel rule, the refund tax/service split, the staff pool decision, the staff-comp rule, the loyalty reward cover, POS-metrics `average_ticket` and its constants. |
| `madar-time` | Business-day rules: week start, business date of an instant, the `YYMMDD` stamp, local day bounds (with the DST-gap rule). |
| `madar-sync` | `/sync/pull` type lists and ledger classification, the R-checksum, the kitchen UUIDv5 ids. |

Plus `authz/gen` (`authz-gen`), the generator for the permission registry.

**Rules for every crate**

- No I/O: no sqlx, no reqwest, no tokio, no clock, no randomness. Callers own
  persistence and time and pass them in.
- A move into this repo never changes a result. Old tablets in the field keep
  their embedded copy of a rule, so the vectors a rule is pinned by stay
  byte-identical across the move.
- Vector files live in the crate that owns the rule (`crates/*/vectors/`) and
  are that crate's tests. A consumer test that needs them (a SQL function, the
  core's bill assembly) reads the same bytes through the crate's `vectors`
  module — never a copy.

## Consuming it

```toml
madar-authz = { git = "https://github.com/Shawket4/madar-shared", tag = "v0.1.0" }
madar-money = { git = "https://github.com/Shawket4/madar-shared", tag = "v0.1.0" }
```

All crates share one version and are released together under one tag.

## Releasing

1. Land the change on `main` here; CI (`.github/workflows/ci.yml`) must be green.
2. Bump `version` in the root `Cargo.toml` (`[workspace.package]`) and tag
   `vX.Y.Z` on that commit; push the tag.
3. Bump the tag in **both** consumers, one PR each:
   - MadarRust: `Cargo.toml`, then `cargo update -p madar-authz` (one crate
     updates every crate from the same repo).
   - madar: `rust-core/crates/madar-core/Cargo.toml`, then
     `cargo update -p madar-authz` in `rust-core/`.

A rule change that changes results is a deliberate, versioned event: the tag
bump is where both sides adopt it together.

## The permission registry

`authz/spec/capabilities.toml` is the single source of truth. `authz-gen`
writes:

- `crates/madar-authz/src/generated.rs` (Rust: backend + cores),
- `<dashboard>/src/generated/capabilities.ts` with `--dashboard <MadarDashboard checkout>`,
- `<pos>/packages/app_core/lib/src/generated/capabilities.dart` with `--pos <madar checkout>`.

```sh
cargo run -p authz-gen                                                # Rust only
cargo run -p authz-gen -- --dashboard ../MadarDashboard --pos ../madar
cargo run -p authz-gen -- --check [--dashboard ... --pos ...]         # fail on drift
```

A spec change is: edit the spec, run the generator with both sibling
checkouts, commit here, release a tag, bump the tag in both consumers and
commit the regenerated `.ts`/`.dart` with the bump.

## Regenerating vectors

Each generator writes into its crate's `vectors/` directory:

```sh
MADAR_REGENERATE_TAX_VECTORS=1    cargo test -p madar-money tax::vectors
MADAR_REGENERATE_REWARD_VECTORS=1 cargo test -p madar-money loyalty::vectors
MADAR_REGENERATE_REFUND_VECTORS=1 cargo test -p madar-money refund_split_vectors
MADAR_REGENERATE_NEGATIVE_VECTORS=1 cargo test -p madar-money negative_part_vectors
MADAR_REGENERATE_TIME_VECTORS=1   cargo test -p madar-time day_bound_vectors
```

`pos_metrics_vectors.json` is produced by the backend's SQL scenario
(`MadarRust/tests/reports_pos_metrics_tests.rs`, `MADAR_WRITE_POS_METRICS_VECTORS=1`),
which writes it into a sibling `madar-shared` checkout.

## Developing against a local checkout

See [Local links](#local-links) below.
