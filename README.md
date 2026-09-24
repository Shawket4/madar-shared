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
madar-time  = { git = "https://github.com/Shawket4/madar-shared", tag = "v0.1.0" }
madar-sync  = { git = "https://github.com/Shawket4/madar-shared", tag = "v0.1.0" }
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

## Local links

**The usual way (owner decision, 2026-09-24): opt in with `dev/link.sh`**, and
link only while you are changing a shared crate:

```sh
dev/link.sh on        # every sibling consumer: ../MadarRust, ../madar/rust-core, ../wt-*
dev/link.sh status
dev/link.sh off       # unlink; Cargo.lock goes back to the pinned tag lines
dev/link.sh on ../MadarRust      # or just one checkout
```

`on` writes a marked `.cargo/config.toml` with the `[patch]` into each consumer
(git-ignored through that clone's `info/exclude`, never committed) and
re-resolves; `off` removes it and re-resolves so `Cargo.lock` is clean again.
The same script works on any machine where madar-shared sits beside the
consumer checkouts (e.g. `~/Desktop/Madar` on the Mac). **Unlink before you
commit** a consumer: a linked `Cargo.lock` has local paths, and CI's
`--locked` refuses it.

The details below are what the script does under the hood.

To build a consumer against THIS checkout rather than the tag it pins, cargo's
`[patch]` swaps the git source for local paths. The patch is in
`dev/cargo-patch.toml` (absolute paths for this machine's
`~/ClaudeProjects` layout). Use it per command:

```sh
cd ~/ClaudeProjects/MadarRust          # or madar/rust-core
cargo --config ../madar-shared/dev/cargo-patch.toml test
```

or, for a checkout you always want linked, copy it to that checkout's
`.cargo/config.toml` (untracked).

**Three things to know**

1. **The tag must exist.** cargo still resolves `tag = "vX.Y.Z"` against
   GitHub before it applies a patch, so a tag that is not pushed yet fails with
   "failed to find tag" even with the patch. Push the tag first, or (for a
   release in progress) point cargo at the local checkout for one command
   without touching any global config:
   ```sh
   CARGO_NET_GIT_FETCH_WITH_CLI=true GIT_CONFIG_COUNT=1 \
   GIT_CONFIG_KEY_0=url.file://$HOME/ClaudeProjects/madar-shared.insteadOf \
   GIT_CONFIG_VALUE_0=https://github.com/Shawket4/madar-shared cargo update -p madar-authz
   ```
   (the local repo needs the tag locally; the resulting `Cargo.lock` is exactly
   what the pushed tag gives, as long as the tag points at the same commit).
2. **Never commit a lock built with the patch.** With the patch active cargo
   drops the `source = "git+https://github.com/Shawket4/madar-shared?tag=…"`
   lines from the consumer's `Cargo.lock`. Before committing, build once
   without the patch (or `cargo update -p madar-authz` without it) and check
   `grep -c 'madar-shared?tag=' Cargo.lock` is one per madar-shared crate.
3. **Not in a shared parent directory.** A `[patch]` in
   `~/ClaudeProjects/.cargo/config.toml` applies to every cargo project below
   it, and cargo writes `[[patch.unused]]` into the lockfile of each project
   that does not use madar-shared (and of one that uses only some of the
   crates). Keep it per command or per checkout.
